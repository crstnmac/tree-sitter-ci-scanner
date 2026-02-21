use serde::{Deserialize, Serialize};
use scanner::output::SarifLog;

/// Organisation-level policy stored as JSONB.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrgPolicy {
    /// Severity levels that cause a scan to fail (e.g. `["error"]`).
    #[serde(default)]
    pub fail_on_severity: Vec<String>,

    /// Whether to block merge (enforced via GitHub commit status `failure` state).
    #[serde(default = "default_block_merge")]
    pub block_merge: bool,
}

fn default_block_merge() -> bool {
    true
}

impl OrgPolicy {
    pub fn from_json(value: &serde_json::Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }
}

/// A single finding extracted from SARIF during policy evaluation.
#[derive(Debug, Clone, Serialize)]
pub struct PolicyFinding {
    pub rule_id: String,
    pub severity: String,
    pub file_path: String,
    pub line_number: Option<i64>,
    pub message: String,
}

/// Result of evaluating a SARIF log against an OrgPolicy.
#[derive(Debug, Clone)]
pub struct PolicyResult {
    pub passed: bool,
    pub findings: Vec<PolicyFinding>,
}

/// Evaluate `sarif` against `policy` and return all findings together with a
/// pass/fail determination.
pub fn evaluate(policy: &OrgPolicy, sarif: &SarifLog) -> PolicyResult {
    let mut findings: Vec<PolicyFinding> = Vec::new();
    let mut passed = true;

    for run in &sarif.runs {
        // Build a rule-id → level map from the tool descriptor.
        let level_map: std::collections::HashMap<&str, &str> = run
            .tool
            .driver
            .rules
            .iter()
            .map(|r| (r.id.as_str(), r.default_configuration.level.as_str()))
            .collect();

        for result in &run.results {
            let severity = level_map
                .get(result.rule_id.as_str())
                .copied()
                .unwrap_or("note");

            let location = result.locations.first();
            let file_path = location
                .map(|l| l.physical_location.artifact_location.uri.clone())
                .unwrap_or_default();
            let line_number = location.map(|l| l.physical_location.region.start_line);

            let finding = PolicyFinding {
                rule_id: result.rule_id.clone(),
                severity: severity.to_string(),
                file_path,
                line_number,
                message: result.message.text.clone(),
            };

            if policy
                .fail_on_severity
                .iter()
                .any(|s| s.eq_ignore_ascii_case(severity))
            {
                passed = false;
            }

            findings.push(finding);
        }
    }

    PolicyResult { passed, findings }
}
