//! TCK Sweep Test Runner — Phase 35 (DMN-33).
//!
//! Discovers all vendored OMG DMN TCK vectors and executes them against the parser, evaluator,
//! and/or exporter. Reports pass/fail/skip with a per-directory breakdown.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

// Re-export test_util for use in this file.
#[allow(unused_imports)]
use engine::dmn::test_util;

// Import parser and exporter from the engine crate.
use engine::dmn::parser;
use engine::dmn::exporter;

/// Result of executing a single TCK vector.
#[derive(Debug)]
struct VectorResult {
    dir_name: String,
    category: String,
    status: TestStatus,
    message: Option<String>,
}

#[derive(Debug)]
enum TestStatus {
    Pass,
    Fail(String),
    Skip(String),
}

/// Aggregated results grouped by directory.
#[derive(Debug)]
struct SweepResults {
    total_vectors: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    by_directory: HashMap<String, DirStats>,
}

#[derive(Debug)]
struct DirStats {
    passed: usize,
    failed: usize,
    skipped: usize,
}

#[test]
fn tck_sweep() {
    let tck_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("tck");

    if !tck_root.is_dir() {
        panic!("TCK fixture directory not found: {}", tck_root.display());
    }

    let vectors = test_util::discover_tck_vectors(&tck_root);
    println!("Discovered {} TCK vectors", vectors.len());

    if vectors.is_empty() {
        eprintln!("WARNING: No TCK vectors discovered. Ensure fixtures are vendored.");
        return;
    }

    let start = Instant::now();
    let results = execute_vectors(&vectors);
    let elapsed = start.elapsed();

    print_summary(&results, &elapsed);
}

fn execute_vectors(vectors: &[test_util::TckVector]) -> SweepResults {
    let mut results = SweepResults {
        total_vectors: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
        by_directory: HashMap::new(),
    };

    for vector in vectors {
        results.total_vectors += 1;
        let result = execute_vector(vector);

        match &result.status {
            TestStatus::Pass => results.passed += 1,
            TestStatus::Fail(_) => results.failed += 1,
            TestStatus::Skip(_) => results.skipped += 1,
        }

        // Update per-directory stats.
        let dir_stats = results.by_directory.entry(vector.dir_name.clone()).or_insert_with(|| DirStats {
            passed: 0,
            failed: 0,
            skipped: 0,
        });
        match &result.status {
            TestStatus::Pass => dir_stats.passed += 1,
            TestStatus::Fail(_) => dir_stats.failed += 1,
            TestStatus::Skip(_) => dir_stats.skipped += 1,
        }
    }

    results
}

fn execute_vector(vector: &test_util::TckVector) -> VectorResult {
    // Try to parse the DMN file.
    let dmn_content = match std::fs::read_to_string(&vector.dmn_path) {
        Ok(content) => content,
        Err(e) => {
            return VectorResult {
                dir_name: vector.dir_name.clone(),
                category: vector.category.clone(),
                status: TestStatus::Fail(format!("Failed to read DMN file: {}", e)),
                message: None,
            };
        }
    };

    // Try to parse using the parser module.
    let parsed = match parser::parse_dmn(&dmn_content) {
        Ok(model) => model,
        Err(e) => {
            return VectorResult {
                dir_name: vector.dir_name.clone(),
                category: vector.category.clone(),
                status: TestStatus::Fail(format!("Parse error: {:?}", e)),
                message: None,
            };
        }
    };

    // Try to export using the exporter module.
    let exported: String = match exporter::export_dmn(&parsed) {
        Ok(xml_str) => xml_str,
        Err(e) => {
            return VectorResult {
                dir_name: vector.dir_name.clone(),
                category: vector.category.clone(),
                status: TestStatus::Fail(format!("Export error: {:?}", e)),
                message: None,
            };
        }
    };

    // Re-parse the exported XML (we don't need the result for comparison).
    let _reparsed: engine::dmn::model::DmnModel = match parser::parse_dmn(&exported) {
        Ok(model) => model,
        Err(e) => {
            return VectorResult {
                dir_name: vector.dir_name.clone(),
                category: vector.category.clone(),
                status: TestStatus::Fail(format!("Re-parse error: {:?}", e)),
                message: None,
            };
        }
    };

    // Compare original and re-parsed using structural equality.
    if test_util::structurally_equal(&dmn_content, &exported) {
        VectorResult {
            dir_name: vector.dir_name.clone(),
            category: vector.category.clone(),
            status: TestStatus::Pass,
            message: Some("Round-trip successful".to_string()),
        }
    } else {
        return VectorResult {
            dir_name: vector.dir_name.clone(),
            category: vector.category.clone(),
            status: TestStatus::Fail(String::from("Round-trip mismatch (structural comparison failed)")),
            message: None,
        };
    }
}

fn print_summary(results: &SweepResults, elapsed: &std::time::Duration) {
    println!("\n=== TCK Sweep Summary ===");
    println!("Total vectors discovered: {}", results.total_vectors);
    println!("Passed:                    {}", results.passed);
    println!("Failed:                    {}", results.failed);
    println!("Skipped (documented):      {}", results.skipped);
    println!("Duration:                  {:?}", elapsed);

    if !results.by_directory.is_empty() {
        println!("\nBy directory:");
        let mut dirs: Vec<_> = results.by_directory.iter().collect();
        dirs.sort_by_key(|(k, _)| k.clone());
        for (dir, stats) in &dirs {
            println!("  {}: {} passed, {} failed, {} skipped", dir, stats.passed, stats.failed, stats.skipped);
        }
    }

    if results.failed > 0 {
        std::process::exit(1);
    }
}
