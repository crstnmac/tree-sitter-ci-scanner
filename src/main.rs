use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::ExitCode;

use scanner::{Scanner, ScannerConfig, RuleEngine, SarifFormatter, JsonFormatter};

#[derive(Parser)]
#[command(name = "scanner")]
#[command(about = "A fast, tree-sitter based static analysis tool for CI/CD pipelines")]
#[command(long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan files or directories for issues
    Scan {
        /// Path to scan (file or directory)
        ///
        /// Examples:
        ///   scanner scan app.js
        ///   scanner scan src/ --recursive
        path: String,

        /// Scan recursively
        ///
        /// Traverse subdirectories when scanning a directory
        #[arg(short, long)]
        recursive: bool,

        /// Configuration file
        ///
        /// Path to YAML configuration file with rule definitions
        /// Default: .scanner.yaml
        #[arg(short, long, default_value = ".scanner.yaml")]
        config: String,

        /// Output file
        ///
        /// Write scan results to this file. If not specified, prints to stdout.
        #[arg(short = 'o', long)]
        output: Option<String>,

        /// Output format
        ///
        /// Format for results: sarif (default) or json
        /// SARIF is recommended for CI/CD integration
        #[arg(short = 'f', long, default_value = "sarif")]
        format: String,

        /// Cache directory
        ///
        /// Directory for storing cached scan results (optional)
        #[arg(long)]
        cache_dir: Option<String>,

        /// Specific rules to run
        ///
        /// Comma-separated list of rule IDs to run.
        /// Example: scanner scan src/ --rules js-no-console-log,js-no-eval
        #[arg(long)]
        rules: Option<String>,
    },
    /// List available rules
    Rules {
        /// List rules for a specific language
        ///
        /// Supported languages: javascript, typescript, python, html
        /// Example: scanner rules --language javascript
        #[arg(short = 'l', long)]
        language: Option<String>,
    },
    /// Show version information
    Version,
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            path,
            recursive,
            config,
            output,
            format,
            cache_dir,
            rules: specific_rules,
        } => {
            // Load rules from config
            let config_path = Path::new(&config);
            let rule_engine = if config_path.exists() {
                RuleEngine::from_yaml(config_path)
                    .with_context(|| format!("Failed to load config from: {}", config))?
            } else {
                // Use default rules if config file doesn't exist
                eprintln!("Config file not found: {}", config);
                eprintln!("Using default rules...");
                return Ok(ExitCode::FAILURE);
            };

            // Filter rules if specific ones were requested
            let rule_engine = if let Some(ref rule_list) = specific_rules {
                let rule_ids: Vec<&str> = rule_list.split(',').map(|s| s.trim()).collect();
                let filtered_rules = rule_engine.rules()
                    .iter()
                    .filter(|r| rule_ids.contains(&r.id.as_str()))
                    .cloned()
                    .collect();
                RuleEngine::new(filtered_rules)
            } else {
                rule_engine
            };

            // Create scanner config
            let scanner_config = ScannerConfig {
                recursive,
                cache_dir,
                specific_rules: specific_rules.map(|r| r.split(',').map(|s| s.trim().to_string()).collect()),
            };

            // Create scanner
            let scanner = Scanner::new(rule_engine, scanner_config);

            // Scan the target path
            let scan_path = Path::new(&path);
            let issues = scanner.scan(scan_path)
                .with_context(|| format!("Failed to scan: {}", path))?;

            // Output results
            match format.as_str() {
                "json" => {
                    let json_output = JsonFormatter::to_json(&issues);
                    if let Some(out_path) = output {
                        std::fs::write(&out_path, serde_json::to_string_pretty(&json_output)?)
                            .with_context(|| format!("Failed to write output to: {}", out_path))?;
                        eprintln!("Wrote {} issues to {}", issues.len(), out_path);
                    } else {
                        println!("{}", serde_json::to_string_pretty(&json_output)?);
                    }
                }
                "sarif" => {
                    let sarif = SarifFormatter::to_sarif(&issues);
                    let sarif_json = serde_json::to_string_pretty(&sarif)?;
                    if let Some(out_path) = output {
                        std::fs::write(&out_path, sarif_json)
                            .with_context(|| format!("Failed to write output to: {}", out_path))?;
                        eprintln!("Wrote {} issues to {}", issues.len(), out_path);
                    } else {
                        println!("{}", sarif_json);
                    }
                }
                _ => {
                    eprintln!("Unsupported format: {}. Use 'sarif' or 'json'", format);
                    return Ok(ExitCode::FAILURE);
                }
            }

            // Return appropriate exit code
            if issues.is_empty() {
                Ok(ExitCode::SUCCESS)
            } else {
                eprintln!("Found {} issues", issues.len());
                Ok(ExitCode::FAILURE)
            }
        }
        Commands::Rules { language } => {
            // Load rules from default config
            let config_path = Path::new(".scanner.yaml");
            let rule_engine = if config_path.exists() {
                RuleEngine::from_yaml(config_path)?
            } else {
                eprintln!("Config file not found: .scanner.yaml");
                return Ok(ExitCode::FAILURE);
            };

            let rules = if let Some(lang) = language {
                rule_engine.rules_for_language(&lang)
            } else {
                rule_engine.rules().iter().collect()
            };

            if rules.is_empty() {
                println!("No rules found.");
            } else {
                println!("Available rules:");
                for rule in rules {
                    println!("  {} - {} ({})",
                        rule.id,
                        rule.name,
                        rule.severity
                    );
                }
            }

            Ok(ExitCode::SUCCESS)
        }
        Commands::Version => {
            println!("scanner version {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
    }
}
