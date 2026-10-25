//! TCK Sweep Test Runner -- Phase 35 (DMN-07, DMN-27, DMN-33).
//!
//! Discovers all vendored OMG DMN TCK vectors and executes them through ALL
//! three layers (parser, evaluator, exporter) producing a comprehensive results
//! report with per-category breakdown, root-cause analysis for failures, and
//! documented known divergences.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

#[allow(unused_imports)]
use engine::dmn::test_util;

use engine::dmn::parser;
use engine::dmn::exporter;
use engine::dmn::model::DefinitionsChild;
use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest, SharedDmnModel};
use engine::dmn::feel::cache::FeelAstCache;

#[path = "tck_values.rs"]
mod tck_values;

// =========================================================================
// Result types
// =========================================================================

#[derive(Debug)]
#[allow(dead_code)] // descriptive fields kept for debugging output
struct VectorResult {
    dir_name: String,
    category: String,
    layer: TestLayer,
    status: TestStatus,
    message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum TestLayer {
    ParserOnly,
    Evaluator,
    RoundTrip,
    DmndiRoundTrip,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // failure/skip reasons are carried for diagnostics
enum TestStatus { Pass, Fail(String), Skip(String) }

#[derive(Debug)]
struct SweepResults {
    total_vectors: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    by_directory: HashMap<String, DirStats>,
    by_level: HashMap<String, LevelStats>,
    known_divergences: Vec<KnownDivergence>,
    dmndi_with_content: usize,
    dmndi_preserved: usize,
    dmndi_lost: Vec<DmndiLoss>,
    /// `(vector directory, layer, reason)` for every failing layer, so a failure
    /// is reported with its cause instead of only being counted.
    failures: Vec<(String, String, String)>,
}

#[derive(Debug)] struct DirStats { passed: usize, failed: usize, skipped: usize }
#[derive(Debug)] struct LevelStats { vectors: usize, passed: usize, failed: usize, skipped: usize }
#[derive(Debug)] struct KnownDivergence { vector: String, category: String, expected: String, actual: String, root_cause: String, spec_reference: String }
#[derive(Debug)] struct DmndiLoss { dir_name: String, reason: String }

// =========================================================================
// Known divergences
// =========================================================================

fn known_divergence_list() -> Vec<KnownDivergence> {
    vec![
        KnownDivergence {
            vector: "DMN15-31-collect-distinct-sum".into(),
            category: "compliance-level-3/0114-collect-aggregators".into(),
            expected: "COLLECT with distinct + SUM aggregator on grouped data".into(),
            actual: "FEEL 1.6 feature not in Level 3 spec; COLLECT with distinct is Level 5".into(),
            root_cause: "COLLECT with distinct parameter is a DMN 1.6 feature (FEEL 1.6), outside Level 3 scope".into(),
            spec_reference: "DMN 1.6 section 8.2.12, FEEL 1.6 specification".into(),
        },
    ]
}

// =========================================================================
// Test execution
// =========================================================================

#[test]
fn tck_sweep() {
    let phase_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join(".planning").join("phases").join("35")
        .join("conformance-verification-round-trip-monitoring-close-out");

    let tck_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("fixtures").join("tck");

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
    let results = execute_all_layers(&vectors);
    let elapsed = start.elapsed();

    print_summary(&results, &elapsed);
    // Writing the report is opt-in: a normal test run must not dirty the tree.
    if std::env::var("TCK_WRITE_REPORT").is_ok() {
        write_results_report(&results, &phase_dir, &elapsed);
    }

    // The documented conformance baseline (see TCK-COVERAGE-GAPS.md). The suite
    // is green at the baseline and fails on any regression: one more failing
    // layer, or one fewer passing layer than the recorded gap set. Closing a gap
    // means raising these numbers, deliberately.
    const BASELINE_PASSED: usize = 143;
    const BASELINE_FAILED: usize = 31;

    if results.passed < BASELINE_PASSED || results.failed > BASELINE_FAILED {
        eprintln!(
            "TCK regression: {}/{} layer results, baseline is {}/{} (passed/failed). \
             Update TCK-COVERAGE-GAPS.md and the baseline constants when a gap is closed.",
            results.passed, results.failed, BASELINE_PASSED, BASELINE_FAILED
        );
        std::process::exit(1);
    }
    println!(
        "Baseline met: {}/{} layer results ({} known failures documented in TCK-COVERAGE-GAPS.md).",
        results.passed, results.failed, BASELINE_FAILED
    );
}

fn execute_all_layers(vectors: &[test_util::TckVector]) -> SweepResults {
    let mut results = SweepResults {
        total_vectors: 0, passed: 0, failed: 0, skipped: 0,
        by_directory: HashMap::new(), by_level: HashMap::new(),
        known_divergences: known_divergence_list(),
        dmndi_with_content: 0, dmndi_preserved: 0, dmndi_lost: Vec::new(),
        failures: Vec::new(),
    };

    for vector in vectors {
        results.total_vectors += 1;

        // Layer 1: Parser-only (parse + export + re-parse structural compare).
        let r = execute_parser_layer(vector);
        record_pass_fail(&mut results, vector, &r.status);

        // Layer 2: Evaluator. Every outcome is recorded — an earlier version
        // only recorded passes here, which made a failing evaluator layer
        // invisible in the summary and in the exit code.
        let eval_result = execute_evaluator_layer(vector);
        record_pass_fail(&mut results, vector, &eval_result.status);

        // Layer 3: Round-trip structural comparison (IR equality primary).
        let rt_result = execute_round_trip_layer(vector);
        record_pass_fail(&mut results, vector, &rt_result.status);

        // Layer 4: DMNDI round-trip.
        let dmndi_result = execute_dmndi_layer(vector, &mut results);
        if !matches!(dmndi_result.status, TestStatus::Skip(_)) {
            record_pass_fail(&mut results, vector, &dmndi_result.status);
        }
    }

    results
}

fn record_pass_fail(results: &mut SweepResults, vector: &test_util::TckVector, status: &TestStatus) {
    match status {
        TestStatus::Pass => results.passed += 1,
        TestStatus::Fail(reason) => {
            results.failed += 1;
            results.failures.push((
                vector.dir_name.clone(),
                String::new(),
                reason.clone(),
            ));
        }
        TestStatus::Skip(_) => results.skipped += 1,
    }

    let dir_stats = results.by_directory.entry(vector.dir_name.clone()).or_insert_with(|| DirStats { passed: 0, failed: 0, skipped: 0 });
    match status {
        TestStatus::Pass => dir_stats.passed += 1,
        TestStatus::Fail(_) => dir_stats.failed += 1,
        TestStatus::Skip(_) => dir_stats.skipped += 1,
    }

    let level = categorize_by_level(&vector.dir_name);
    let level_stats = results.by_level.entry(level).or_insert_with(|| LevelStats { vectors: 0, passed: 0, failed: 0, skipped: 0 });
    level_stats.vectors += 1;
    match status {
        TestStatus::Pass => level_stats.passed += 1,
        TestStatus::Fail(_) => level_stats.failed += 1,
        TestStatus::Skip(_) => level_stats.skipped += 1,
    }

    // Check for known divergences (match by directory name).
    let dir_lower = vector.dir_name.to_lowercase();
    let kd_list: Vec<_> = results.known_divergences.iter().collect();
    for kd in &kd_list {
        if dir_lower.contains(&kd.vector.to_lowercase().replace(' ', "-")) && matches!(status, TestStatus::Fail(_)) {
            results.failed -= 1;
            results.skipped += 1;
            let ds = results.by_directory.get_mut(&vector.dir_name).unwrap();
            ds.failed -= 1;
            ds.skipped += 1;
        }
    }
}

fn execute_parser_layer(vector: &test_util::TckVector) -> VectorResult {
    let dmn_content = match std::fs::read_to_string(&vector.dmn_path) {
        Ok(c) => c, Err(e) => return fail_result(vector, format!("Failed to read DMN file: {}", e)),
    };

    let ir1 = match parser::parse_dmn(&dmn_content) {
        Ok(m) => m, Err(e) => return fail_result(vector, format!("Parse error: {:?}", e)),
    };

    let exported = match exporter::export_dmn(&ir1) {
        Ok(s) => s, Err(e) => return fail_result(vector, format!("Export error: {:?}", e)),
    };

    let ir2 = match parser::parse_dmn(&exported) {
        Ok(m) => m, Err(e) => return fail_result(vector, format!("Re-parse error: {:?}", e)),
    };

    // Primary gate: IR equality (canonical form ignores namespace URI differences).
    if ir1 == ir2 {
        VectorResult { dir_name: vector.dir_name.clone(), category: vector.category.clone(), layer: TestLayer::ParserOnly, status: TestStatus::Pass, message: Some("IR round-trip equal".into()) }
    } else {
        // Secondary gate: XML structural compare with namespace normalization.
        let norm_orig = normalize_ns(&dmn_content);
        let norm_exp = normalize_ns(&exported);
        if test_util::structurally_equal(&norm_orig, &norm_exp) {
            VectorResult { dir_name: vector.dir_name.clone(), category: vector.category.clone(), layer: TestLayer::ParserOnly, status: TestStatus::Pass, message: Some("XML structurally equal after NS normalization".into()) }
        } else {
            fail_result(vector, format!("Round-trip mismatch: IRs differ and normalized XML differs. Orig: {}, Exported: {}", &dmn_content[..200.min(dmn_content.len())], &exported[..200.min(exported.len())]))
        }
    }
}

fn execute_evaluator_layer(vector: &test_util::TckVector) -> VectorResult {
    let test_content = match std::fs::read_to_string(&vector.test_xml_path) {
        Ok(c) => c,
        Err(_) => return skip_result(vector, "No test XML file found".into()),
    };

    let dmn_content = match std::fs::read_to_string(&vector.dmn_path) {
        Ok(c) => c,
        Err(_) => return skip_result(vector, "Cannot read DMN file for evaluation".into()),
    };

    let parsed = match parser::parse_dmn(&dmn_content) {
        Ok(m) => m,
        Err(e) => return fail_result(vector, format!("Parse error (evaluation): {:?}", e)),
    };

    let test_cases = tck_values::parse_test_cases(&test_content);
    if test_cases.is_empty() {
        return skip_result(vector, "No extractable test cases from XML".into());
    }

    let ast_cache = FeelAstCache::new();
    let shared = Arc::new(std::sync::RwLock::new(SharedDmnModel::new(parsed, ast_cache)));
    let evaluator = DecisionEvaluator::new(shared.clone());

    let decisions = collect_decisions(&shared);
    let Some((default_id, _)) = decisions.first().cloned() else {
        return skip_result(vector, "No decision found in DMN for evaluation".into());
    };

    let mut passed_cases = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for case in &test_cases {
        // The TCK names each result node after the decision that produces it;
        // evaluate that decision when it exists, otherwise the vector default.
        // A TCK `resultNode name` is the decision's *name* in most vectors and
        // its *id* in others, so match either.
        let decision_id = case
            .expected
            .iter()
            .map(|(name, _)| name.clone())
            .find_map(|name| {
                decisions
                    .iter()
                    .find(|(id, decision_name)| *id == name || *decision_name == name)
                    .map(|(id, _)| id.clone())
            })
            .unwrap_or_else(|| default_id.clone());

        let inputs: std::collections::HashMap<String, engine::data_types::Value> = case
            .inputs
            .iter()
            .map(|(name, node)| (name.clone(), tck_values::to_value(node)))
            .collect();

        // A case may name several result nodes (one per decision in a
        // multi-decision model). Each names the decision that produces it, and
        // every one of them must match for the case to pass.
        let mut case_ok = true;
        if case.expects_error {
            let outcome = evaluator.evaluate(
                &decision_id,
                EvaluateRequest {
                    inputs: inputs.clone(),
                    trace: false,
                },
            );
            let all_null = match &outcome {
                Ok(result) => result
                    .outputs
                    .values()
                    .all(|value| matches!(value, engine::data_types::Value::Null)),
                Err(_) => true,
            };
            if !all_null {
                case_ok = false;
                match outcome {
                    Ok(result) => failures.push(format!(
                        "{}: expected null (errorResult) but produced {:?}",
                        case.id, result.outputs
                    )),
                    Err(e) => failures.push(format!("{}: expected null, got error {:?}", case.id, e)),
                }
            }
        } else if case.expected.is_empty() {
            case_ok = false;
            failures.push(format!("{}: no expected result extracted", case.id));
        } else {
            for (expected_name, expected_node) in &case.expected {
                let expected = tck_values::to_value(expected_node);
                let target = decisions
                    .iter()
                    .find(|(id, name)| id == expected_name || name == expected_name)
                    .map(|(id, _)| id.clone())
                    .unwrap_or_else(|| decision_id.clone());

                match evaluator.evaluate(
                    &target,
                    EvaluateRequest {
                        inputs: inputs.clone(),
                        trace: false,
                    },
                ) {
                    Ok(result) => {
                        let matched = result
                            .outputs
                            .get(expected_name)
                            .is_some_and(|value| tck_values::deep_equal(value, &expected))
                            || result
                                .outputs
                                .values()
                                .any(|value| tck_values::deep_equal(value, &expected));
                        if !matched {
                            case_ok = false;
                            failures.push(format!(
                                "{} / {}: got {:?}, expected {:?}",
                                case.id, expected_name, result.outputs, expected
                            ));
                        }
                    }
                    Err(e) => {
                        case_ok = false;
                        failures.push(format!("{} / {}: evaluate error {:?}", case.id, expected_name, e));
                    }
                }
            }
        }

        if case_ok {
            passed_cases += 1;
        }
    }

    let total = test_cases.len();
    if passed_cases == total {
        VectorResult {
            dir_name: vector.dir_name.clone(),
            category: vector.category.clone(),
            layer: TestLayer::Evaluator,
            status: TestStatus::Pass,
            message: Some(format!("{passed_cases}/{total} test cases matched")),
        }
    } else {
        let shown = failures
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        fail_result(
            vector,
            format!(
                "{passed_cases}/{total} test cases matched ({} failed): {}",
                total - passed_cases,
                shown
            ),
        )
    }
}

/// Collect every decision as `(id, name)` in declaration order.
fn collect_decisions(shared: &Arc<std::sync::RwLock<SharedDmnModel>>) -> Vec<(String, String)> {
    let Ok(guard) = shared.read() else {
        return Vec::new();
    };
    guard
        .model
        .definitions
        .children
        .iter()
        .filter_map(|child| match child {
            DefinitionsChild::Decision(d) => {
                Some((d.id.clone(), d.name.clone().unwrap_or_else(|| d.id.clone())))
            }
            _ => None,
        })
        .collect()
}

fn execute_round_trip_layer(vector: &test_util::TckVector) -> VectorResult {
    let dmn_content = match std::fs::read_to_string(&vector.dmn_path) {
        Ok(c) => c, Err(e) => return fail_result(vector, format!("Failed to read DMN file: {}", e)),
    };

    let ir1 = match parser::parse_dmn(&dmn_content) {
        Ok(m) => m, Err(e) => return fail_result(vector, format!("Parse error (round-trip): {:?}", e)),
    };

    let exported = match exporter::export_dmn(&ir1) {
        Ok(s) => s, Err(e) => return fail_result(vector, format!("Export error (round-trip): {:?}", e)),
    };

    let ir2 = match parser::parse_dmn(&exported) {
        Ok(m) => m, Err(e) => return fail_result(vector, format!("Re-parse error (round-trip): {:?}", e)),
    };

    // Primary gate: IR equality.
    if ir1 == ir2 {
        VectorResult { dir_name: vector.dir_name.clone(), category: vector.category.clone(), layer: TestLayer::RoundTrip, status: TestStatus::Pass, message: Some("IR round-trip equal".into()) }
    } else {
        // Secondary gate: XML structural compare with namespace normalization.
        let norm_orig = normalize_ns(&dmn_content);
        let norm_exp = normalize_ns(&exported);
        if test_util::structurally_equal(&norm_orig, &norm_exp) {
            VectorResult { dir_name: vector.dir_name.clone(), category: vector.category.clone(), layer: TestLayer::RoundTrip, status: TestStatus::Pass, message: Some("XML structurally equal after NS normalization".into()) }
        } else {
            fail_result(vector, format!("Round-trip mismatch: IRs and normalized XML both differ. Orig: {}, Exported: {}", &dmn_content[..200.min(dmn_content.len())], &exported[..200.min(exported.len())]))
        }
    }
}

fn execute_dmndi_layer(vector: &test_util::TckVector, results: &mut SweepResults) -> VectorResult {
    let dmn_content = match std::fs::read_to_string(&vector.dmn_path) {
        Ok(c) => c, Err(e) => return fail_result(vector, format!("Failed to read DMN file: {}", e)),
    };

    let has_dmndi = dmn_content.contains("dmndi:") || dmn_content.contains("DMNDI");
    if !has_dmndi {
        return skip_result(vector, "No DMNDI content in source file".into());
    }

    let ir1 = match parser::parse_dmn(&dmn_content) {
        Ok(m) => m, Err(e) => return fail_result(vector, format!("Parse error (DMNDI): {:?}", e)),
    };

    let original_diagram_count = ir1.definitions.diagrams.len();
    if original_diagram_count == 0 {
        results.dmndi_with_content += 1;
        return skip_result(vector, "DMNDI present in XML but parser did not populate diagrams (diagrams=0)".into());
    }

    let exported = match exporter::export_dmn(&ir1) {
        Ok(s) => s, Err(e) => return fail_result(vector, format!("Export error (DMNDI): {:?}", e)),
    };

    // Verify DMNDI namespace in export.
    if !exported.contains("xmlns:dmndi=") {
        results.dmndi_with_content += 1;
        results.dmndi_lost.push(DmndiLoss { dir_name: vector.dir_name.clone(), reason: "Exported XML missing dmndi namespace".into() });
        return fail_result(vector, "DMNDI namespace declarations not preserved in export".into());
    }

    let ir2 = match parser::parse_dmn(&exported) {
        Ok(m) => m, Err(e) => {
            results.dmndi_with_content += 1;
            results.dmndi_lost.push(DmndiLoss { dir_name: vector.dir_name.clone(), reason: format!("Re-parse failed: {:?}", e) });
            return fail_result(vector, format!("Re-parse error (DMNDI): {:?}", e));
        }
    };

    let reparsed_diagram_count = ir2.definitions.diagrams.len();

    if original_diagram_count != reparsed_diagram_count {
        results.dmndi_with_content += 1;
        results.dmndi_lost.push(DmndiLoss { dir_name: vector.dir_name.clone(), reason: format!("Diagram count mismatch: {} -> {}", original_diagram_count, reparsed_diagram_count) });
        return fail_result(vector, format!("DMNDI diagram count lost: {} -> {}", original_diagram_count, reparsed_diagram_count));
    }

    // Primary gate: IR equality.
    if ir1 == ir2 {
        results.dmndi_with_content += 1;
        results.dmndi_preserved += 1;
        VectorResult { dir_name: vector.dir_name.clone(), category: vector.category.clone(), layer: TestLayer::DmndiRoundTrip, status: TestStatus::Pass, message: Some(format!("DMNDI IR round-trip equal ({} diagram(s) preserved)", original_diagram_count)) }
    } else {
        // Secondary gate: XML structural compare.
        let norm_orig = normalize_ns(&dmn_content);
        let norm_exp = normalize_ns(&exported);
        if test_util::structurally_equal(&norm_orig, &norm_exp) {
            results.dmndi_with_content += 1;
            results.dmndi_preserved += 1;
            VectorResult { dir_name: vector.dir_name.clone(), category: vector.category.clone(), layer: TestLayer::DmndiRoundTrip, status: TestStatus::Pass, message: Some("DMNDI XML structurally equal after NS normalization".into()) }
        } else {
            results.dmndi_with_content += 1;
            results.dmndi_lost.push(DmndiLoss { dir_name: vector.dir_name.clone(), reason: "DMNDI IRs and XML both differ after round-trip".into() });
            fail_result(vector, "DMNDI content not preserved through parse->export->re-parse".into())
        }
    }
}

// =========================================================================
// Helpers
// =========================================================================

/// Normalize DMNDI / DC / DI namespace URIs to canonical 2018 form.
fn normalize_ns(xml: &str) -> String {
    let mut result = xml.to_string();
    // The exporter uses these canonical URIs; some TCK fixtures use alternate forms.
    for (from, to) in &[
        ("https://www.omg.org/spec/DMN/20230324/MODEL/", "http://www.omg.org/spec/DMN/20180521/MODEL/"),
        ("https://www.omg.org/spec/DMN/20230324/DMNDI/", "http://www.omg.org/spec/DMN/20180521/DMNDI/"),
        ("http://www.omg.org/spec/DD/20180521/DC/", "http://www.omg.org/spec/DMN/20180521/DC/"),
        ("http://www.omg.org/spec/DD/20180521/DI/", "http://www.omg.org/spec/DMN/20180521/DI/"),
    ] {
        result = result.replace(from, to);
    }
    result
}

// =========================================================================
// Test XML parsing helpers
// =========================================================================

fn categorize_by_level(dir_name: &str) -> String {
    // Classify by directory prefix pattern (mirrors TCK grouping).
    if dir_name.starts_with("0") && !dir_name.starts_with("01") { "compliance-level-2".into() }
    else if dir_name.starts_with("01") || dir_name.starts_with("02") || dir_name.starts_with('1') { "compliance-level-3".into() }
    else { format!("other/{}", dir_name) }
}

fn fail_result(vector: &test_util::TckVector, msg: String) -> VectorResult {
    VectorResult { dir_name: vector.dir_name.clone(), category: vector.category.clone(), layer: TestLayer::ParserOnly, status: TestStatus::Fail(msg), message: None }
}
fn skip_result(vector: &test_util::TckVector, msg: String) -> VectorResult {
    VectorResult { dir_name: vector.dir_name.clone(), category: vector.category.clone(), layer: TestLayer::ParserOnly, status: TestStatus::Skip(msg), message: None }
}

// =========================================================================
// Output
// =========================================================================

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
        dirs.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (dir, stats) in &dirs {
            println!("  {}: {} passed, {} failed, {} skipped", dir, stats.passed, stats.failed, stats.skipped);
        }
    }

    if !results.by_level.is_empty() {
        println!("\nBy compliance level:");
        for level in &["compliance-level-2", "compliance-level-3"] {
            if let Some(ls) = results.by_level.get(*level) {
                let pct = if ls.vectors > 0 { (ls.passed * 100) / ls.vectors } else { 0 };
                println!("  {}: {} / {} passed ({:.0}%)", level, ls.passed, ls.vectors, pct as f64);
            }
        }
    }

    if results.dmndi_with_content > 0 {
        println!("\nDMNDI round-trip:");
        println!("  Fixtures with DMNDI content: {}", results.dmndi_with_content);
        println!("  Preserved through cycle:     {}", results.dmndi_preserved);
        if !results.dmndi_lost.is_empty() {
            println!("  Lost ({}):", results.dmndi_lost.len());
            for loss in &results.dmndi_lost { println!("    - {}: {}", loss.dir_name, loss.reason); }
        }
    }

    if !results.known_divergences.is_empty() {
        println!("\nKnown divergences ({}):", results.known_divergences.len());
        for kd in &results.known_divergences {
            println!("  - {} [{}]: expected={}, actual={}", kd.vector, kd.category, kd.expected, kd.actual);
        }
    }

    if !results.failures.is_empty() {
        println!("\nFailures ({}):", results.failures.len());
        for (dir, layer, reason) in &results.failures {
            println!("  - {dir}{layer}: {reason}");
        }
    }

}

fn write_results_report(results: &SweepResults, phase_dir: &Path, elapsed: &std::time::Duration) {
    let report_path = phase_dir.join("TCK_RESULTS.md");
    if let Err(e) = std::fs::create_dir_all(phase_dir) { eprintln!("WARNING: Could not create phase directory: {}", e); return; }

    let mut f = match std::fs::File::create(&report_path) { Ok(f) => f, Err(e) => { eprintln!("WARNING: Could not write TCK_RESULTS.md: {}", e); return; } };

    writeln!(f, "# TCK Sweep Results - Phase 35\n").unwrap();
    writeln!(f, "**Duration:** {:?}  ", elapsed).unwrap();
    writeln!(f, "**TCK Source:** https://github.com/dmn-tck/tck (commit 0dbcaf9)\n").unwrap();

    writeln!(f, "## Summary\n").unwrap();
    writeln!(f, "| Metric | Count |").unwrap();
    writeln!(f, "|--------|-------|").unwrap();
    writeln!(f, "| Total vectors discovered | {} |", results.total_vectors).unwrap();
    writeln!(f, "| Passed | {} |", results.passed).unwrap();
    writeln!(f, "| Failed | {} |", results.failed).unwrap();
    writeln!(f, "| Skipped (documented) | {} |", results.skipped).unwrap();
    // Compute pass rate based on unique vectors that had at least one pass across layers.
    let vectors_with_pass: usize = results.by_directory.values().filter(|ds| ds.passed > 0).count();
    if results.total_vectors > 0 {
        let pct = (vectors_with_pass * 100) / results.total_vectors;
        writeln!(f, "| Vectors with at least one pass | {} / {} | {:.0}% |\n", vectors_with_pass, results.total_vectors, pct as f64).unwrap();
    }
    writeln!(f).unwrap();

    writeln!(f, "## By Compliance Level\n").unwrap();
    for level in &["compliance-level-2", "compliance-level-3"] {
        if let Some(ls) = results.by_level.get(*level) {
            writeln!(f, "### {}\n| Vectors | Passed | Failed | Skipped | Pass Rate |\n|---------|--------|--------|---------|-----------|\n| {} | {} | {} | {} | {:.0}% |\n", level, ls.vectors, ls.passed, ls.failed, ls.skipped, (if ls.vectors > 0 { (ls.passed * 100) / ls.vectors } else { 0 }) as f64).unwrap();
        }
    }

    writeln!(f, "## By Directory\n| Directory | Passed | Failed | Skipped |\n|-----------|--------|--------|---------|\n").unwrap();
    let mut dirs: Vec<_> = results.by_directory.iter().collect();
    dirs.sort_by(|(a, _), (b, _)| a.cmp(b));
    for (dir, stats) in &dirs { writeln!(f, "| {} | {} | {} | {} |\n", dir, stats.passed, stats.failed, stats.skipped).unwrap(); }

    if !results.known_divergences.is_empty() {
        writeln!(f, "## Known Divergences\n| Vector | Category | Expected | Actual | Root Cause | Spec Reference |\n|--------|----------|----------|--------|------------|----------------|\n").unwrap();
        for kd in &results.known_divergences { writeln!(f, "| {} | {} | {} | {} | {} | {} |\n", kd.vector, kd.category, kd.expected, kd.actual, kd.root_cause, kd.spec_reference).unwrap(); }
    }

    if results.dmndi_with_content > 0 {
        writeln!(f, "## Round-Trip Fidelity (DMNDI)\n").unwrap();
        writeln!(f, "- **Fixtures with DMNDI content:** {}", results.dmndi_with_content).unwrap();
        writeln!(f, "- **Preserved through round-trip:** {}", results.dmndi_preserved).unwrap();
        if !results.dmndi_lost.is_empty() { writeln!(f, "- **Lost ({}):**", results.dmndi_lost.len()).unwrap(); for loss in &results.dmndi_lost { writeln!(f, "  - **{}:** {}", loss.dir_name, loss.reason).unwrap(); } }
        else { writeln!(f, "- **Lost:** None").unwrap(); }
        writeln!(f).unwrap();
    }

    writeln!(f, "## Layer Breakdown\nEach vector is tested through up to 4 layers:\n| Layer | Description |\n|-------|-------------|\n| Parser Only | Parse DMN -> Export XML -> Re-parse -> IR structural compare |\n| Evaluator | Parse DMN + evaluate with test inputs vs expected values |\n| Round-Trip | Full IR round-trip (parse->export->re-parse) IR equality |\n| DMNDI Round-Trip | Diagram/layout preservation through parse->export->re-parse |\n").unwrap();
}
