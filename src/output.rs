use crate::rules::FoundIssue;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// SARIF 2.1.0 output formatter
pub struct SarifFormatter;

impl SarifFormatter {
    /// Convert found issues to SARIF format
    pub fn to_sarif(issues: &[FoundIssue]) -> SarifLog {
        let mut results = Vec::new();

        for issue in issues {
            results.push(issue.to_sarif_result());
        }

        SarifLog {
            version: "2.1.0".to_string(),
            schema: "https://json.schemastore.org/sarif-2.1.0.json".to_string(),
            runs: vec![SarifRun {
                tool: SarifTool {
                    driver: SarifToolComponent {
                        name: "scanner".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        information_uri: "https://github.com/crstnmac/tree-sitter-ci-scanner"
                            .to_string(),
                        rules: Self::extract_rules(issues),
                    },
                },
                results,
            }],
        }
    }

    /// Extract unique rules from issues
    fn extract_rules(issues: &[FoundIssue]) -> Vec<SarifRule> {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        let mut rules = Vec::new();

        for issue in issues {
            if seen.insert(&issue.rule_id) {
                rules.push(SarifRule {
                    id: issue.rule_id.clone(),
                    name: issue.rule_name.clone(),
                    short_description: SarifMessage {
                        text: issue.message.clone(),
                    },
                    default_configuration: SarifRuleConfiguration {
                        level: issue.severity.to_sarif_level(),
                    },
                });
            }
        }

        rules
    }
}

impl FoundIssue {
    /// Convert to SARIF result
    fn to_sarif_result(&self) -> SarifResult {
        SarifResult {
            rule_id: self.rule_id.clone(),
            message: SarifMessage {
                text: self.message.clone(),
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: self.file_path.clone(),
                    },
                    region: SarifRegion {
                        start_line: self.start_line as i64,
                        start_column: self.start_column as i64,
                        end_line: Some(self.end_line as i64),
                        end_column: Some(self.end_column as i64),
                        snippet: Some(SarifCodeSnippet {
                            text: self.code_snippet.clone(),
                        }),
                    },
                },
            }],
        }
    }
}

/// JSON output formatter
pub struct JsonFormatter;

impl JsonFormatter {
    /// Convert found issues to JSON format
    pub fn to_json(issues: &[FoundIssue]) -> serde_json::Value {
        json!({
            "version": "1.0.0",
            "issues": issues
        })
    }
}

// SARIF 2.1.0 structures
#[derive(Debug, Serialize, Deserialize)]
pub struct SarifLog {
    pub version: String,
    #[serde(rename = "$schema")]
    pub schema: String,
    pub runs: Vec<SarifRun>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifRun {
    pub tool: SarifTool,
    pub results: Vec<SarifResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifTool {
    pub driver: SarifToolComponent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifToolComponent {
    pub name: String,
    pub version: String,
    #[serde(rename = "informationUri")]
    pub information_uri: String,
    pub rules: Vec<SarifRule>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifRule {
    pub id: String,
    pub name: String,
    #[serde(rename = "shortDescription")]
    pub short_description: SarifMessage,
    #[serde(rename = "defaultConfiguration")]
    pub default_configuration: SarifRuleConfiguration,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifMessage {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifRuleConfiguration {
    pub level: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifResult {
    #[serde(rename = "ruleId")]
    pub rule_id: String,
    pub message: SarifMessage,
    pub locations: Vec<SarifLocation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    pub artifact_location: SarifArtifactLocation,
    pub region: SarifRegion,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifArtifactLocation {
    pub uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifRegion {
    #[serde(rename = "startLine")]
    pub start_line: i64,
    #[serde(rename = "startColumn")]
    pub start_column: i64,
    #[serde(rename = "endLine")]
    pub end_line: Option<i64>,
    #[serde(rename = "endColumn")]
    pub end_column: Option<i64>,
    pub snippet: Option<SarifCodeSnippet>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SarifCodeSnippet {
    pub text: String,
}

// Helper trait for severity conversion
pub trait ToSarifLevel {
    fn to_sarif_level(&self) -> String;
}

impl ToSarifLevel for crate::rules::Severity {
    fn to_sarif_level(&self) -> String {
        match self {
            crate::rules::Severity::Error => "error".to_string(),
            crate::rules::Severity::Warning => "warning".to_string(),
            crate::rules::Severity::Note => "note".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Severity;

    #[test]
    fn test_severity_to_sarif() {
        assert_eq!(Severity::Error.to_sarif_level(), "error");
        assert_eq!(Severity::Warning.to_sarif_level(), "warning");
        assert_eq!(Severity::Note.to_sarif_level(), "note");
    }

    #[test]
    fn test_sarif_formatter_empty() {
        let sarif = SarifFormatter::to_sarif(&[]);
        assert_eq!(sarif.version, "2.1.0");
        assert_eq!(sarif.runs[0].results.len(), 0);
    }
}
