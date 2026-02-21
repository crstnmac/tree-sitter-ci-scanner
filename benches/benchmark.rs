use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::fs;
use std::path::Path;
use std::time::Duration;
use std::hint::black_box;
use scanner::{Scanner, ScannerConfig};
use scanner::parser::CodeParser;
use scanner::parser::LanguageType;
use scanner::rules::RuleEngine;
use scanner::querier;
use tree_sitter::Query;

/// Benchmark parsing different languages
fn bench_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parsing");
    group.measurement_time(Duration::from_secs(5));

    // JavaScript
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
"#;
    group.bench_function("javascript_medium", |b| {
        b.iter(|| {
            let mut parser = CodeParser::new(LanguageType::JavaScript).unwrap();
            parser.parse(black_box(js_code)).unwrap();
        });
    });

    // TypeScript
    let ts_code = r#"
interface User {
    name: string;
    age: number;
    email?: string;
}

function processUser(user: User): string {
    if (user.age > 18) {
        return `Adult: ${user.name}`;
    }
    return `Minor: ${user.name}`;
}
"#;
    group.bench_function("typescript_medium", |b| {
        b.iter(|| {
            let mut parser = CodeParser::new(LanguageType::TypeScript).unwrap();
            parser.parse(black_box(ts_code)).unwrap();
        });
    });

    // Python
    let py_code = r#"
class DataProcessor:
    def __init__(self, data):
        self.data = data
    
    def process(self):
        return [x * 2 for x in self.data]
    
    def filter(self, predicate):
        return [x for x in self.data if predicate(x)]

def main():
    processor = DataProcessor([1, 2, 3, 4, 5])
    results = processor.process()
    return results
"#;
    group.bench_function("python_medium", |b| {
        b.iter(|| {
            let mut parser = CodeParser::new(LanguageType::Python).unwrap();
            parser.parse(black_box(py_code)).unwrap();
        });
    });

    group.finish();
}

/// Benchmark query execution
fn bench_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("queries");
    group.measurement_time(Duration::from_secs(5));

    let js_code = r#"
console.log("test1");
console.log("test2");
console.log("test3");
console.log("test4");
console.log("test5");
"#;

    let query_str = r#"
(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @prop
    (#eq? @obj "console")
    (#eq? @prop "log")))
"#;

    group.bench_function("simple_query", |b| {
        b.iter(|| {
            let mut parser = CodeParser::new(LanguageType::JavaScript).unwrap();
            let tree = parser.parse(black_box(js_code)).unwrap();
            querier::run_query(&tree, js_code, query_str).unwrap();
        });
    });

    // Larger code
    let large_js = (0..100).map(|i| format!("console.log({});", i)).collect::<Vec<_>>().join("\n");

    group.bench_with_input(BenchmarkId::new("query_large_file", "100_console_logs"), &large_js, |b, code| {
        b.iter(|| {
            let mut parser = CodeParser::new(LanguageType::JavaScript).unwrap();
            let tree = parser.parse(black_box(code)).unwrap();
            querier::run_query(&tree, code, query_str).unwrap();
        });
    });

    group.finish();
}

/// Benchmark full scanning workflow
fn bench_scanning(c: &mut Criterion) {
    let mut group = c.benchmark_group("scanning");
    group.measurement_time(Duration::from_secs(10));

    // Create test files
    let test_dir = tempfile::tempdir().unwrap();
    let dir = test_dir.path();

    // Create various test files
    for i in 0..10 {
        let js_file = dir.join(format!("test_{}.js", i));
        fs::write(
            &js_file,
            format!(r#"
function test{}() {{
    console.log("message");
    eval("code");
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

    group.bench_function("scan_10_files", |b| {
        b.iter(|| {
            let scanner = Scanner::new(rule_engine.clone(), config.clone());
            scanner.scan(black_box(dir)).unwrap();
        });
    });

    group.finish();
}

/// Benchmark query compilation
fn bench_query_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_compilation");

    group.bench_function("compile_simple_query", |b| {
        b.iter(|| {
            let parser = CodeParser::new(LanguageType::JavaScript).unwrap();
            let language_ref = parser.language_type().get_tree_sitter_language();
            Query::new(&language_ref, black_box("(identifier) @id")).unwrap();
        });
    });

    group.bench_function("compile_complex_query", |b| {
        let complex_query = r#"
(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @prop
    (#eq? @obj "console")
    (#eq? @prop "log"))
  arguments: (arguments (string)? @args))
"#;
        b.iter(|| {
            let parser = CodeParser::new(LanguageType::JavaScript).unwrap();
            let language_ref = parser.language_type().get_tree_sitter_language();
            Query::new(&language_ref, black_box(complex_query)).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parsing,
    bench_queries,
    bench_scanning,
    bench_query_compilation
);
criterion_main!(benches);
