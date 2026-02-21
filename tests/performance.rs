//! Simple performance tests for scanner
//! Run with: cargo test --release --bench performance

use std::fs;
use std::path::Path;
use std::time::Instant;
use scanner::{Scanner, ScannerConfig};
use scanner::parser::CodeParser;
use scanner::parser::LanguageType;
use scanner::rules::RuleEngine;
use scanner::querier;

#[test]
#[ignore] // Run manually with: cargo test --release performance -- --ignored
fn bench_parsing_javascript() {
    let js_code = r#"
function example(a, b) {
    const result = a + b;
    console.log(result);
    return result;
}

class MyClass {
    constructor() {
        this.value = 42;
    }
    method() {
        return this.value * 2;
    }
}

// Large code block
for (let i = 0; i < 1000; i++) {
    console.log(i);
}

function nested() {
    function inner() {
        return 42;
    }
    return inner();
}
"#;

    let mut parser = CodeParser::new(LanguageType::JavaScript).unwrap();
    let start = Instant::now();

    for _ in 0..100 {
        parser.parse(js_code).unwrap();
    }

    let duration = start.elapsed();
    println!("JavaScript parsing (100 iterations): {:?}", duration);
    assert!(duration.as_millis() < 1000, "Parsing too slow");
}

#[test]
#[ignore]
fn bench_query_execution() {
    let js_code = r#"
console.log("test1");
console.log("test2");
console.log("test3");
console.log("test4");
console.log("test5");
console.log("test6");
console.log("test7");
console.log("test8");
console.log("test9");
console.log("test10");
"#;

    let query_str = r#"
(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @prop
    (#eq? @obj "console")
    (#eq? @prop "log")))
"#;

    let mut parser = CodeParser::new(LanguageType::JavaScript).unwrap();
    let tree = parser.parse(js_code).unwrap();

    // Run a more realistic number of iterations
    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        querier::run_query(&tree, js_code, query_str).unwrap();
    }

    let duration = start.elapsed();
    let avg_per_query = duration.as_micros() as f64 / iterations as f64;
    println!("Query execution ({} iterations): {:?} (avg: {:.2} µs per query)",
        iterations, duration, avg_per_query);
    assert!(duration.as_millis() < 5000, "Query execution too slow");
}

#[test]
#[ignore]
fn bench_multiple_languages() {
    let codes = vec![
        (LanguageType::JavaScript, r#"function test() { return 42; }"#),
        (LanguageType::TypeScript, r#"function test(): number { return 42; }"#),
        (LanguageType::Python, r#"def test(): return 42"#),
    ];

    let start = Instant::now();
    let mut parse_count = 0;

    for (lang, code) in codes.iter() {
        for _ in 0..100 {
            let mut parser = CodeParser::new(*lang).unwrap();
            parser.parse(code).unwrap();
            parse_count += 1;
        }
    }

    let duration = start.elapsed();
    println!("Multi-language parsing ({} iterations): {:?}", parse_count, duration);
    assert!(duration.as_millis() < 2000, "Multi-language parsing too slow");
}

#[test]
#[ignore]
fn bench_large_file() {
    // Generate a large JavaScript file
    let large_js: String = (0..1000)
        .map(|i| format!("function test{}() {{ console.log({}); return {}; }}\n", i, i, i))
        .collect();

    let mut parser = CodeParser::new(LanguageType::JavaScript).unwrap();
    let start = Instant::now();

    for _ in 0..10 {
        parser.parse(&large_js).unwrap();
    }

    let duration = start.elapsed();
    let lines = large_js.lines().count();
    println!("Large file parsing ({} lines, 10 iterations): {:?}", lines, duration);
    assert!(duration.as_millis() < 5000, "Large file parsing too slow");
}

#[test]
#[ignore]
fn bench_full_scan_workflow() {
    // Create temporary test files
    let test_dir = tempfile::tempdir().unwrap();
    let dir = test_dir.path();

    for i in 0..20 {
        let js_file = dir.join(format!("test_{}.js", i));
        fs::write(
            &js_file,
            format!(r#"
function test{}() {{
    console.log("message");
    return true;
}}
"#, i)
        ).unwrap();
    }

    // Load rule engine
    let rule_engine = if Path::new(".scanner.yaml").exists() {
        RuleEngine::from_yaml(Path::new(".scanner.yaml")).unwrap()
    } else {
        panic!("Please run from project root with .scanner.yaml");
    };

    let config = ScannerConfig {
        recursive: true,
        cache_dir: None,
        specific_rules: None,
    };

    let start = Instant::now();
    let scanner = Scanner::new(rule_engine, config);
    let issues = scanner.scan(dir).unwrap();
    let duration = start.elapsed();

    println!("Full scan (20 files, {} issues): {:?}", issues.len(), duration);
    assert!(duration.as_millis() < 10000, "Full scan too slow");
}
