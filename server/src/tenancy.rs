//! Tenant rules: an ordered rule set, edited in the UI, compiled into a
//! ClickHouse expression at query time. First matching rule wins; unmatched
//! pods land in the "unassigned" tenant. Because assignment happens at query
//! time, rule changes apply retroactively to all historical data.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const UNASSIGNED: &str = "unassigned";
const MAX_RULES: usize = 200;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MatcherType {
    /// namespace == match_value
    NamespaceExact,
    /// namespace matches the RE2 regex in match_value
    NamespaceRegex,
    /// namespace has label match_key (== match_value unless empty)
    NamespaceLabel,
    /// pod has label match_key (== match_value unless empty)
    PodLabel,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TenantSource {
    /// tenant is the literal tenant_name
    Static,
    /// tenant is the value of the matched label (label matchers only)
    LabelValue,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TenantRule {
    pub matcher_type: MatcherType,
    #[serde(default)]
    pub match_key: String,
    #[serde(default)]
    pub match_value: String,
    pub tenant_source: TenantSource,
    #[serde(default)]
    pub tenant_name: String,
}

/// Escapes a string for embedding in a single-quoted ClickHouse literal.
fn sql_quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

impl TenantRule {
    pub fn validate(&self) -> Result<()> {
        match self.matcher_type {
            MatcherType::NamespaceExact | MatcherType::NamespaceRegex => {
                if self.match_value.is_empty() {
                    bail!("namespace matcher needs a match_value");
                }
                if self.tenant_source == TenantSource::LabelValue {
                    bail!("label_value tenant source requires a label matcher");
                }
            }
            MatcherType::NamespaceLabel | MatcherType::PodLabel => {
                if self.match_key.is_empty() {
                    bail!("label matcher needs a match_key");
                }
            }
        }
        if self.tenant_source == TenantSource::Static && self.tenant_name.is_empty() {
            bail!("static tenant source needs a tenant_name");
        }
        Ok(())
    }

    /// (condition, value) pair for the multiIf chain. `ns_labels` / `pod_labels`
    /// are the SQL expressions holding the label maps.
    fn compile(&self, ns_labels: &str, pod_labels: &str) -> (String, String) {
        let condition = match self.matcher_type {
            MatcherType::NamespaceExact => {
                format!("namespace = {}", sql_quote(&self.match_value))
            }
            MatcherType::NamespaceRegex => {
                format!("match(namespace, {})", sql_quote(&self.match_value))
            }
            MatcherType::NamespaceLabel => {
                let key = sql_quote(&self.match_key);
                if self.match_value.is_empty() {
                    format!("mapContains({ns_labels}, {key})")
                } else {
                    format!("{ns_labels}[{key}] = {}", sql_quote(&self.match_value))
                }
            }
            MatcherType::PodLabel => {
                let key = sql_quote(&self.match_key);
                if self.match_value.is_empty() {
                    format!("mapContains({pod_labels}, {key})")
                } else {
                    format!("{pod_labels}[{key}] = {}", sql_quote(&self.match_value))
                }
            }
        };
        let value = match self.tenant_source {
            TenantSource::Static => sql_quote(&self.tenant_name),
            TenantSource::LabelValue => {
                let key = sql_quote(&self.match_key);
                match self.matcher_type {
                    MatcherType::NamespaceLabel => format!("{ns_labels}[{key}]"),
                    _ => format!("{pod_labels}[{key}]"),
                }
            }
        };
        (condition, value)
    }
}

/// Compiles the ordered rule set into a single ClickHouse expression yielding
/// the tenant name.
pub fn compile_rules(rules: &[TenantRule], ns_labels: &str, pod_labels: &str) -> Result<String> {
    if rules.len() > MAX_RULES {
        bail!("too many rules (max {MAX_RULES})");
    }
    for rule in rules {
        rule.validate()?;
    }
    if rules.is_empty() {
        return Ok(sql_quote(UNASSIGNED));
    }
    let mut parts = Vec::with_capacity(rules.len() * 2 + 1);
    for rule in rules {
        let (condition, value) = rule.compile(ns_labels, pod_labels);
        parts.push(condition);
        parts.push(value);
    }
    parts.push(sql_quote(UNASSIGNED));
    Ok(format!("multiIf({})", parts.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(
        matcher_type: MatcherType,
        key: &str,
        value: &str,
        source: TenantSource,
        name: &str,
    ) -> TenantRule {
        TenantRule {
            matcher_type,
            match_key: key.into(),
            match_value: value.into(),
            tenant_source: source,
            tenant_name: name.into(),
        }
    }

    #[test]
    fn empty_rules_yield_unassigned() {
        assert_eq!(
            compile_rules(&[], "ns.labels", "pm.labels").unwrap(),
            "'unassigned'"
        );
    }

    #[test]
    fn compiles_ordered_multiif() {
        let rules = vec![
            rule(
                MatcherType::NamespaceExact,
                "",
                "kube-system",
                TenantSource::Static,
                "platform",
            ),
            rule(
                MatcherType::NamespaceLabel,
                "tenant",
                "",
                TenantSource::LabelValue,
                "",
            ),
            rule(
                MatcherType::PodLabel,
                "team",
                "checkout",
                TenantSource::Static,
                "shop-team",
            ),
        ];
        let sql = compile_rules(&rules, "ns.labels", "pm.labels").unwrap();
        assert_eq!(
            sql,
            "multiIf(namespace = 'kube-system', 'platform', \
             mapContains(ns.labels, 'tenant'), ns.labels['tenant'], \
             pm.labels['team'] = 'checkout', 'shop-team', \
             'unassigned')"
        );
    }

    #[test]
    fn escapes_quotes() {
        let rules = vec![rule(
            MatcherType::NamespaceExact,
            "",
            "it's",
            TenantSource::Static,
            "a'b",
        )];
        let sql = compile_rules(&rules, "n", "p").unwrap();
        assert!(sql.contains("'it\\'s'"));
        assert!(sql.contains("'a\\'b'"));
    }

    #[test]
    fn rejects_invalid_rules() {
        assert!(rule(
            MatcherType::NamespaceExact,
            "",
            "",
            TenantSource::Static,
            "x"
        )
        .validate()
        .is_err());
        assert!(
            rule(MatcherType::PodLabel, "", "", TenantSource::Static, "x")
                .validate()
                .is_err()
        );
        assert!(rule(
            MatcherType::NamespaceExact,
            "",
            "ns",
            TenantSource::LabelValue,
            ""
        )
        .validate()
        .is_err());
        assert!(
            rule(MatcherType::PodLabel, "team", "", TenantSource::Static, "")
                .validate()
                .is_err()
        );
        assert!(rule(
            MatcherType::PodLabel,
            "team",
            "",
            TenantSource::LabelValue,
            ""
        )
        .validate()
        .is_ok());
    }
}
