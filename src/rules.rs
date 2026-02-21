use serde::{Deserialize, Serialize};
use std::fmt;
use anyhow::Context;

/// Severity level for rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Note => write!(f, "note"),
        }
    }
}

/// A rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Unique rule identifier
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Severity level
    pub severity: Severity,

    /// Tree-sitter query string
    pub query: String,

    /// Message to display when the rule matches
    pub message: String,

    /// Language this rule applies to
    pub language: String,

    /// Optional: Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A configuration file containing multiple rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    /// List of rules
    pub rules: Vec<Rule>,
}

/// Issue found by a rule
#[derive(Debug, Clone, serde::Serialize)]
pub struct FoundIssue {
    /// Rule that triggered this issue
    pub rule_id: String,

    /// Rule name
    pub rule_name: String,

    /// Severity level
    pub severity: Severity,

    /// Issue message
    pub message: String,

    /// File path
    pub file_path: String,

    /// Start line (1-indexed)
    pub start_line: usize,

    /// Start column (1-indexed)
    pub start_column: usize,

    /// End line (1-indexed)
    pub end_line: usize,

    /// End column (1-indexed)
    pub end_column: usize,

    /// Code snippet that triggered the issue
    pub code_snippet: String,
}

/// Rule engine that executes rules against parsed code
#[derive(Clone)]
pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    /// Create a new rule engine with the given rules
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Load rules from a YAML configuration file
    pub fn from_yaml(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read rules file: {}", path.display()))?;

        let config: RulesConfig = serde_yaml::from_str(&content)
            .context("Failed to parse rules YAML")?;

        Ok(Self::new(config.rules))
    }

    /// Get all rules
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Get rules for a specific language
    pub fn rules_for_language(&self, language: &str) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| r.language.eq_ignore_ascii_case(language))
            .collect()
    }

    /// Check a rule against parsed code
    pub fn check_rule(
        &self,
        rule: &Rule,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> anyhow::Result<Vec<FoundIssue>> {
        let mut issues = Vec::new();

        // Create a query from the rule's query string
        let root_node = tree.root_node();
        let query =
            tree_sitter::Query::new(&root_node.language(), &rule.query)
            .map_err(|e| anyhow::anyhow!("Failed to create query for rule {}: {}", rule.id, e))?;

        // Run the query
        let mut cursor = tree_sitter::QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        for m in matches {
            for capture in m.captures {
                let node = capture.node;

                let start_line = node.start_position().row;
                let start_col = node.start_position().column;
                let end_line = node.end_position().row;
                let end_col = node.end_position().column;

                // Extract code snippet
                let snippet = &source[node.byte_range()];

                issues.push(FoundIssue {
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    severity: rule.severity,
                    message: rule.message.clone(),
                    file_path: file_path.to_string(),
                    start_line: start_line + 1, // Convert to 1-indexed
                    start_column: start_col + 1,
                    end_line: end_line + 1,
                    end_column: end_col + 1,
                    code_snippet: snippet.to_string(),
                });
            }
        }

        Ok(issues)
    }

    /// Run all applicable rules against parsed code
    pub fn check(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
        language: &str,
    ) -> anyhow::Result<Vec<FoundIssue>> {
        let mut all_issues = Vec::new();

        for rule in self.rules_for_language(language) {
            match self.check_rule(rule, tree, source, file_path) {
                Ok(mut issues) => all_issues.append(&mut issues),
                Err(e) => {
                    tracing::warn!("Failed to check rule {}: {}", rule.id, e);
                }
            }
        }

        Ok(all_issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_display() {
        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(format!("{}", Severity::Note), "note");
    }
}
