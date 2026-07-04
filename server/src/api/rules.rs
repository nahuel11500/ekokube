//! Tenant rule set CRUD (a single ordered document) + live preview.

use axum::extract::{Query, State};
use axum::Json;
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::tenants::{tenants_query, TenantStat};
use super::{ApiError, AppState, RangeQuery};
use crate::tenancy::TenantRule;

#[derive(Row, Deserialize)]
struct ConfigRow {
    rules_json: String,
}

pub async fn load_rules(ch: &clickhouse::Client) -> anyhow::Result<Vec<TenantRule>> {
    let rows: Vec<ConfigRow> = ch
        .query("SELECT rules_json FROM tenant_config FINAL WHERE key = 'default'")
        .fetch_all()
        .await?;
    match rows.first() {
        Some(row) => Ok(serde_json::from_str(&row.rules_json)?),
        None => Ok(Vec::new()),
    }
}

pub async fn get_rules(State(state): State<AppState>) -> Result<Json<Vec<TenantRule>>, ApiError> {
    Ok(Json(load_rules(&state.ch).await?))
}

#[derive(Row, Serialize)]
struct InsertConfigRow {
    key: String,
    rules_json: String,
    updated_at: u32,
}

pub async fn put_rules(
    State(state): State<AppState>,
    Json(rules): Json<Vec<TenantRule>>,
) -> Result<Json<Vec<TenantRule>>, ApiError> {
    // Compile up front so an invalid rule set is rejected, not stored.
    crate::tenancy::compile_rules(&rules, "ns_labels", "pod_labels")?;
    let mut insert = state.ch.insert("tenant_config")?;
    insert
        .write(&InsertConfigRow {
            key: "default".into(),
            rules_json: serde_json::to_string(&rules)?,
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as u32)
                .unwrap_or(0),
        })
        .await?;
    insert.end().await?;
    Ok(Json(rules))
}

/// Evaluates a candidate rule set (request body) without saving it.
pub async fn preview_rules(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
    Json(rules): Json<Vec<TenantRule>>,
) -> Result<Json<Vec<TenantStat>>, ApiError> {
    let stats = tenants_query(&state.ch, &query, &rules).await?;
    Ok(Json(stats))
}
