#!/bin/bash
# Performance benchmark script for tree-sitter CI/CD scanner
# This runs all performance tests and reports results

set -e

cd "$(dirname "$0")"

echo "=========================================="
echo "Tree-sitter CI/CD Scanner Benchmark"
echo "=========================================="
echo ""

echo "Building release version..."
cargo build --release --quiet

echo ""
echo "Running performance benchmarks..."
echo ""

# Run performance tests
cargo test --release --test performance -- --ignored 2>&1 | tee benchmark_results.txt

echo ""
echo "=========================================="
echo "Benchmark Summary"
echo "=========================================="

# Extract timing information
echo ""
grep -E "(parsing|Query execution|Multi-language|Large file|Full scan)" benchmark_results.txt | \
    sed 's/^/  /'

echo ""
echo "Run 'cargo test --release --test performance -- --ignored' for full details."
echo ""
