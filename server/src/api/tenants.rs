//! Tenant consumption (chargeback): rollups joined with pod/namespace labels,
//! grouped by the compiled tenant expression.

use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::rules::load_rules;
use super::{ApiError, AppState, RangeQuery};
use crate::tenancy::{compile_rules, TenantRule};

#[derive(Row, Deserialize, Serialize, Debug)]
pub struct TenantStat {
    pub tenant: String,
    pub namespaces: u64,
    pub pods: u64,
    pub cpu_core_hours: f64,
    pub cpu_request_core_hours: f64,
    pub mem_gib_hours: f64,
    pub mem_request_gib_hours: f64,
}

pub async fn tenants_query(
    ch: &clickhouse::Client,
    query: &RangeQuery,
    rules: &[TenantRule],
) -> anyhow::Result<Vec<TenantStat>> {
    let (from, to) = query.pod_range();
    let (table, _) = query.pod_source();
    let where_clause = query.pod_where();
    let tenant_expr = compile_rules(rules, "ns_labels", "pod_labels")?;
    let sql = format!(
        "SELECT {tenant_expr} AS tenant, \
                uniqExact(namespace) AS namespaces, \
                uniqExact(u.pod_uid) AS pods, \
                sum(cpu_core_secs)/3600 AS cpu_core_hours, \
                sum(cpu_request_core_secs)/3600 AS cpu_request_core_hours, \
                sum(mem_ws_byte_secs)/3600/1073741824 AS mem_gib_hours, \
                sum(mem_request_byte_secs)/3600/1073741824 AS mem_request_gib_hours \
         FROM {table} AS u \
         LEFT JOIN (SELECT pod_uid, labels AS pod_labels FROM pod_meta FINAL) AS pm \
              ON u.pod_uid = pm.pod_uid \
         LEFT JOIN (SELECT name, labels AS ns_labels FROM namespaces FINAL) AS ns \
              ON u.namespace = ns.name \
         WHERE {where_clause} \
         GROUP BY tenant \
         ORDER BY cpu_core_hours DESC"
    );
    Ok(ch.query(&sql).bind(from).bind(to).fetch_all().await?)
}

// Not #[serde(flatten)]: flatten breaks number parsing in query strings
// (serde_urlencoded buffers flattened values as strings).
#[derive(Deserialize)]
pub struct TenantQuery {
    pub from: Option<u32>,
    pub to: Option<u32>,
    /// Billing rates; costs are returned when provided.
    pub cost_core_hour: Option<f64>,
    pub cost_gib_hour: Option<f64>,
}

impl TenantQuery {
    fn range(&self) -> RangeQuery {
        RangeQuery {
            from: self.from,
            to: self.to,
            namespace: None,
            sort_by: None,
            order: None,
            limit: None,
            offset: None,
        }
    }
}

#[derive(Serialize)]
pub struct TenantRow {
    #[serde(flatten)]
    stat: TenantStat,
    /// Billed on requests (what was reserved), the usual chargeback basis.
    cost_requested: f64,
    /// Billed on actual usage, for comparison.
    cost_used: f64,
}

fn with_costs(stats: Vec<TenantStat>, core_rate: f64, gib_rate: f64) -> Vec<TenantRow> {
    stats
        .into_iter()
        .map(|s| TenantRow {
            cost_requested: s.cpu_request_core_hours * core_rate
                + s.mem_request_gib_hours * gib_rate,
            cost_used: s.cpu_core_hours * core_rate + s.mem_gib_hours * gib_rate,
            stat: s,
        })
        .collect()
}

pub async fn handler(
    State(state): State<AppState>,
    Query(query): Query<TenantQuery>,
) -> Result<Json<Vec<TenantRow>>, ApiError> {
    let rules = load_rules(&state.ch).await?;
    let stats = tenants_query(&state.ch, &query.range(), &rules).await?;
    Ok(Json(with_costs(
        stats,
        query.cost_core_hour.unwrap_or(0.0),
        query.cost_gib_hour.unwrap_or(0.0),
    )))
}

pub async fn export_csv(
    State(state): State<AppState>,
    Query(query): Query<TenantQuery>,
) -> Result<Response, ApiError> {
    let rules = load_rules(&state.ch).await?;
    let stats = tenants_query(&state.ch, &query.range(), &rules).await?;
    let rows = with_costs(
        stats,
        query.cost_core_hour.unwrap_or(0.0),
        query.cost_gib_hour.unwrap_or(0.0),
    );
    let (from, to) = query.range().pod_range();

    let mut csv = String::from(
        "tenant,namespaces,pods,cpu_core_hours,cpu_request_core_hours,\
         mem_gib_hours,mem_request_gib_hours,cost_requested,cost_used\n",
    );
    for r in &rows {
        // Tenant names come from labels; quote them CSV-safely.
        let tenant = format!("\"{}\"", r.stat.tenant.replace('"', "\"\""));
        csv.push_str(&format!(
            "{tenant},{},{},{:.4},{:.4},{:.4},{:.4},{:.2},{:.2}\n",
            r.stat.namespaces,
            r.stat.pods,
            r.stat.cpu_core_hours,
            r.stat.cpu_request_core_hours,
            r.stat.mem_gib_hours,
            r.stat.mem_request_gib_hours,
            r.cost_requested,
            r.cost_used,
        ));
    }
    let filename = format!("ekokube-tenants-{from}-{to}.csv");
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        csv,
    )
        .into_response())
}
