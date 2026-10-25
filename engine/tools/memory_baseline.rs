//! Memory baseline benchmark — Phase 36.
//!
//! Measures RSS peak, allocation counts, and per-phase breakdown for the DMN subsystem.
//! Runs against the v6.0 import corpus and saves results as JSON in `engine/benches/snapshots/`.

use engine::dmn::bench_util::{
    load_corpus_files, tck_root, write_baseline_snapshot, MemoryBaseline, PhaseBreakdown,
    RssSnapshot,
};
use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest, SharedDmnModel};
use std::time::Instant;

fn main() {
    println!("=== DMN Memory Baseline Benchmark ===");
    println!();

    // Load corpus.
    let corpus_files = load_corpus_files();
    if corpus_files.is_empty() {
        eprintln!("ERROR: no DMN files found in corpus.");
        std::process::exit(1);
    }
    println!("Loaded {} DMN files from corpus.", corpus_files.len());

    // Take cold-start RSS snapshot.
    let cold_rss = RssSnapshot::take();
    println!(
        "Cold start RSS: {:.2} MB",
        cold_rss.vm_rss_bytes as f64 / 1024.0 / 1024.0
    );

    let mut phases = Vec::new();
    let mut peak_rss = cold_rss.vm_rss_bytes;
    let start_time = Instant::now();

    // Phase 1: Parse all files.
    println!("\nPhase 1: Parsing {} DMN files...", corpus_files.len());
    let pre_parse_rss = RssSnapshot::take();
    let mut models = Vec::new();
    for (i, file) in corpus_files.iter().enumerate() {
        if i % 5 == 0 {
            println!("  Parsed {} files...", i);
        }
        let content = std::fs::read_to_string(file).expect("read DMN file");
        let model = engine::dmn::parser::parse_dmn(&content).expect("parse DMN");
        models.push(model);
    }
    let post_parse_rss = RssSnapshot::take();
    let _parse_duration = Instant::now().duration_since(start_time);

    phases.push(PhaseBreakdown {
        phase: "parse".into(),
        rss_delta_bytes: post_parse_rss.delta_from(&pre_parse_rss),
        allocation_count: corpus_files.len() as u64, // Approximate.
    });

    println!("  Parse RSS delta: {} bytes", phases[0].rss_delta_bytes);
    peak_rss = peak_rss.max(post_parse_rss.vm_rss_bytes);

    // Phase 2: Evaluate decision tables (warm + cold).
    println!("\nPhase 2: Evaluating decision tables...");
    let pre_eval_rss = RssSnapshot::take();
    let mut total_evals = 0u64;
    for model in &models {
        // Extract decisions from the model.
        let decisions = extract_decisions(model);
        if !decisions.is_empty() {
            // Evaluate each decision with empty inputs.
            let shared = std::sync::Arc::new(std::sync::RwLock::new(SharedDmnModel::new(
                (*model).clone(),
                engine::dmn::feel::cache::FeelAstCache::new(),
            )));
            let evaluator = DecisionEvaluator::new(shared);

            for decision in &decisions {
                let request = EvaluateRequest {
                    inputs: std::collections::HashMap::new(),
                    trace: false,
                };
                if let Ok(_result) = evaluator.evaluate(decision, request) {
                    total_evals += 1;
                }
            }
        }
    }
    let post_eval_rss = RssSnapshot::take();

    phases.push(PhaseBreakdown {
        phase: "evaluate".into(),
        rss_delta_bytes: post_eval_rss.delta_from(&pre_eval_rss),
        allocation_count: total_evals,
    });

    println!("  Evaluated {} decisions.", total_evals);
    println!("  Evaluate RSS delta: {} bytes", phases[1].rss_delta_bytes);
    peak_rss = peak_rss.max(post_eval_rss.vm_rss_bytes);

    // Phase 3: DRD chain evaluation.
    println!("\nPhase 3: DRD chain evaluation...");
    let pre_drd_rss = RssSnapshot::take();

    // Find a multi-output model for DRD testing.
    let tck_root_path = tck_root();
    let multi_path = tck_root_path.join("0010-multi-output-U/0010-multi-output-U.dmn");
    if multi_path.is_file() {
        let content = std::fs::read_to_string(&multi_path).expect("read multi-output DMN");
        let (model, cache) =
            engine::dmn::parser::parse_dmn_with_cache(&content).expect("parse with cache");

        let shared = std::sync::Arc::new(std::sync::RwLock::new(SharedDmnModel::new(model, cache)));
        let evaluator = DecisionEvaluator::new(shared.clone());

        // Collect decision IDs.
        let guard = shared.read().unwrap();
        let decisions = extract_decisions_from_model(&guard.model);
        drop(guard);

        for decision in &decisions {
            let request = EvaluateRequest {
                inputs: std::collections::HashMap::new(),
                trace: false,
            };
            let _ = evaluator.evaluate(decision, request);
        }
    }

    let post_drd_rss = RssSnapshot::take();
    phases.push(PhaseBreakdown {
        phase: "drd_resolution".into(),
        rss_delta_bytes: post_drd_rss.delta_from(&pre_drd_rss),
        allocation_count: 0, // Not tracked precisely.
    });

    println!("  DRD RSS delta: {} bytes", phases[2].rss_delta_bytes);
    peak_rss = peak_rss.max(post_drd_rss.vm_rss_bytes);

    let total_duration = start_time.elapsed();

    // Build and write baseline snapshot.
    let baseline = MemoryBaseline {
        corpus_name: "v6.0-tck".into(),
        file_count: corpus_files.len(),
        cold_start_rss_bytes: cold_rss.vm_rss_bytes,
        phases,
        peak_rss_bytes: peak_rss,
        total_duration_ns: total_duration.as_nanos(),
    };

    // PERF-04: by default write a CURRENT snapshot to a separate file so the
    // frozen `v6.0-baseline.json` (committed to the repo) is never overwritten
    // by a regression-gate run. Pass `--update-baseline` to intentionally
    // refresh the frozen baseline (used by `make baseline-update` when
    // accepting new numbers).
    let snapshot_name = if std::env::args().any(|a| a == "--update-baseline") {
        "v6.0-baseline"
    } else {
        "v6.0-current"
    };
    let snapshot_path =
        write_baseline_snapshot(&baseline, snapshot_name).expect("write baseline snapshot");

    println!("\n=== Memory Baseline Summary ===");
    println!("Corpus: {} files", baseline.file_count);
    println!(
        "Cold start RSS: {:.2} MB",
        baseline.cold_start_rss_bytes as f64 / 1024.0 / 1024.0
    );
    println!(
        "Peak RSS: {:.2} MB",
        baseline.peak_rss_bytes as f64 / 1024.0 / 1024.0
    );
    println!("Total duration: {:?}", total_duration);
    println!("\nPer-phase breakdown:");
    for phase in &baseline.phases {
        println!(
            "  {}: RSS delta = {} bytes, allocations ≈ {}",
            phase.phase, phase.rss_delta_bytes, phase.allocation_count
        );
    }
    println!(
        "\nBaseline snapshot written to: {}",
        snapshot_path.display()
    );
}

/// Extract decision IDs from a DmnModel.
fn extract_decisions(model: &engine::dmn::model::DmnModel) -> Vec<String> {
    let mut decisions = Vec::new();
    for child in &model.definitions.children {
        if let engine::dmn::model::DefinitionsChild::Decision(d) = child {
            decisions.push(d.id.clone());
        }
    }
    decisions
}

/// Extract decision IDs from a shared model snapshot.
fn extract_decisions_from_model(model: &engine::dmn::model::DmnModel) -> Vec<String> {
    extract_decisions(model)
}
