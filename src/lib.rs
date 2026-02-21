//! # Scanner
//!
//! A fast, tree-sitter based static analysis tool for CI/CD pipelines.

pub mod parser;
pub mod querier;
pub mod rules;
pub mod scanner;
pub mod output;
pub mod utils;

pub mod cache;

pub use rules::{Rule, Severity, RuleEngine};
pub use scanner::{Scanner, ScannerConfig};
pub use output::{SarifFormatter, JsonFormatter};
