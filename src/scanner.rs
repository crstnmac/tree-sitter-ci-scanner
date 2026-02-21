use crate::parser::{CodeParser, LanguageType};
use crate::rules::RuleEngine;
use crate::rules::FoundIssue;
use anyhow::{Context, Result};
use std::path::Path;
use walkdir::WalkDir;

/// Scanner configuration
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    pub recursive: bool,
    pub cache_dir: Option<String>,
    pub specific_rules: Option<Vec<String>>,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            recursive: false,
            cache_dir: None,
            specific_rules: None,
        }
    }
}

/// Main scanner for code analysis
pub struct Scanner {
    rule_engine: RuleEngine,
    config: ScannerConfig,
}

impl Scanner {
    /// Create a new scanner with a rule engine
    pub fn new(rule_engine: RuleEngine, config: ScannerConfig) -> Self {
        Self {
            rule_engine,
            config,
        }
    }

    /// Scan a single file
    pub fn scan_file(&self, path: &Path) -> Result<Vec<FoundIssue>> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let language = LanguageType::from_extension(ext)
            .context(format!("Unsupported file extension: {}", ext))?;

        let mut parser = CodeParser::new(language)?;
        let (tree, source) = parser.parse_file(path)?;

        self.rule_engine.check(
            &tree,
            &source,
            &path.display().to_string(),
            language.name(),
        )
    }

    /// Scan a directory
    pub fn scan_directory(&self, path: &Path) -> Result<Vec<FoundIssue>> {
        let mut all_issues = Vec::new();

        let walker = if self.config.recursive {
            WalkDir::new(path).into_iter()
        } else {
            WalkDir::new(path).max_depth(1).into_iter()
        };

        for entry in walker {
            let entry = entry.context("Failed to read directory entry")?;

            if entry.file_type().is_file() {
                if let Ok(issues) = self.scan_file(entry.path()) {
                    all_issues.extend(issues);
                }
            }
        }

        Ok(all_issues)
    }

    /// Scan a path (file or directory)
    pub fn scan(&self, path: &Path) -> Result<Vec<FoundIssue>> {
        if path.is_file() {
            self.scan_file(path)
        } else if path.is_dir() {
            self.scan_directory(path)
        } else {
            anyhow::bail!("Path does not exist: {}", path.display());
        }
    }

    /// Get the rule engine
    pub fn rule_engine(&self) -> &RuleEngine {
        &self.rule_engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_config_default() {
        let config = ScannerConfig::default();
        assert!(!config.recursive);
        assert!(config.cache_dir.is_none());
        assert!(config.specific_rules.is_none());
    }
}
