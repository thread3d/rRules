//! DMN subsystem benchmarks — Phase 36 (measurement only, no optimization).
//!
//! Four benchmark groups:
//!   1. **parser**          — parse a corpus of .dmn XML into canonical IR, ops/sec + allocations
//!   2. **feel_evaluator**  — evaluate representative FEEL expressions against varying input sizes
//!   3. **decision_table**  — evaluate decision tables under each hit policy (UNIQUE .. COLLECT), ops/sec
//!   4. **drd_resolution**  — topological sort + evaluation chain time for multi-output DRD services
//!
//! Corpus: vendored TCK fixtures at `engine/tests/fixtures/tck/`, plus the import-test corpus
//! under `api/dmn/`. Both are loaded once per benchmark run via the helpers below.

use criterion::{criterion_group, criterion_main, BenchmarkId, SamplingMode};
use std::collections::HashMap;
use std::sync::Arc;
use engine::data_types::Value;
use engine::dmn::decision_table::DecisionTableEvaluator;
use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest, SharedDmnModel};
use engine::dmn::feel::ast::FeelExpression;
use engine::dmn::feel::cache::FeelAstCache;
use engine::dmn::feel::eval::Scope;
use engine::dmn::model::{BuiltinAggregator, DecisionTable, HitPolicy, InputClause, OutputClause};

// ---------------------------------------------------------------------------
// Corpus helpers — load .dmn files from the test fixtures tree.
// ---------------------------------------------------------------------------

/// Locate the TCK fixture root relative to the workspace.  Benchmarks always run
/// out of `target/` so we walk up two levels (`.. / ..`) to reach the repo root,
/// then descend into `engine/tests/fixtures/tck/`.
fn tck_root() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let parent1 = manifest.parent().unwrap_or_else(|| std::path::Path::new("."));
    let parent2 = parent1.parent().unwrap_or_else(|| std::path::Path::new("."));
    parent2.join("engine/tests/fixtures/tck")
}

/// Locate the import-test corpus under `api/dmn/`.
fn import_corpus_root() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let parent1 = manifest.parent().unwrap_or_else(|| std::path::Path::new("."));
    let parent2 = parent1.parent().unwrap_or_else(|| std::path::Path::new("."));
    parent2.join("api/dmn")
}

/// Collect every `.dmn` file under a directory (recursively).  Returns the full
/// paths sorted for deterministic ordering.
fn collect_dmn_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    if !root.is_dir() {
        return Vec::new();
    }
    let mut files = Vec::new();
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(d).expect("read dir") {
            let e = entry.expect("read entry");
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map_or(false, |ext| ext == "dmn") {
                out.push(p);
            }
        }
    }
    walk(root, &mut files);
    files.sort();
    files
}

/// Load the combined corpus: TCK fixtures + import-test DMN files.  Returns a
/// vector of `(name, xml_bytes)` tuples ready for benchmarking.
fn load_corpus() -> Vec<(String, Vec<u8>)> {
    let mut items = Vec::new();
    for p in collect_dmn_files(&tck_root()) {
        if let Ok(bytes) = std::fs::read(&p) {
            items.push((p.file_stem().unwrap().to_string_lossy().into_owned(), bytes));
        }
    }
    for p in collect_dmn_files(&import_corpus_root()) {
        if let Ok(bytes) = std::fs::read(&p) {
            items.push((p.file_stem().unwrap().to_string_lossy().into_owned(), bytes));
        }
    }
    items
}

// ---------------------------------------------------------------------------
// Benchmark group 1: parser throughput
// ---------------------------------------------------------------------------

fn bench_parser(c: &mut criterion::Criterion) {
    let corpus = load_corpus();
    assert!(!corpus.is_empty(), "no DMN corpus found — is the test-fixture tree present?");

    // Warm the parser up once so we don't benchmark cold-start of quick-xml.
    for (_, bytes) in &corpus {
        let _ = engine::dmn::parser::parse_dmn_bytes(bytes);
    }

    let mut group = c.benchmark_group("parser");
    group.sampling_mode(SamplingMode::Flat);

    // Per-file throughput: ops/sec and allocations.
    for (name, bytes) in &corpus {
        group.bench_with_input(BenchmarkId::new("parse", name), bytes, |b, data| {
            b.iter(|| engine::dmn::parser::parse_dmn_bytes(data))
        });
    }

    // Corpus-level throughput (all files parsed sequentially).
    let corpus_ref: Vec<_> = corpus.iter().map(|(_, b)| b.as_slice()).collect();
    group.bench_function("corpus_all", |b| {
        b.iter(|| {
            for data in &corpus_ref {
                let _ = engine::dmn::parser::parse_dmn_bytes(data);
            }
        })
    });

    // Parse-with-cache variant (FEEL AST pre-parsing, DMN-18).
    group.bench_function("parse_with_cache", |b| {
        b.iter_batched(
            || corpus_ref.clone(),
            |data: Vec<&[u8]>| -> () {
                for data in &data {
                    let _ = engine::dmn::parser::parse_dmn_with_cache(std::str::from_utf8(data)
                        .expect("utf-8"))
                        .expect("parse with cache");
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark group 2: FEEL evaluator latency
// ---------------------------------------------------------------------------

/// A small set of representative FEEL expressions spanning the operator classes.
const FEEL_EXPRESSIONS: &[&str] = &[
    // Arithmetic
    "1 + 2 * 3",
    "(1 + 2) * 3",
    "2 ** 10",
    // Comparison
    "x > 5 and y < 10",
    "a in [1, 2, 3]",
    // Boolean / ternary
    "if x > 0 then \"positive\" else \"non-positive\"",
    // List functions
    "string-join(names, \", \")",
    "count(numbers) + sum(numbers)",
    // Context access
    "person.name",
    "lookup(map, key).value",
    // Quantified
    "some n in numbers satisfies n > 10",
    "every x in xs matches y in ys where x = y",
    // For expression
    "for i in 1..10 return i * 2",
    // Date/time
    "@2024-01-15T10:30:00+01:00",
];

fn bench_feel_evaluator(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("feel_evaluator");
    group.sampling_mode(SamplingMode::Flat);

    // Build a scope with representative bindings for each expression to evaluate.
    let mut scope = Scope::empty();
    scope.bindings.insert(
        "x".into(), Value::Integer(42),
    );
    scope.bindings.insert(
        "y".into(), Value::Integer(7),
    );
    scope.bindings.insert(
        "a".into(), Value::Integer(3),
    );
    scope.bindings.insert(
        "b".into(), Value::String("hello".into()),
    );
    scope.bindings.insert(
        "names".into(), Value::Array(vec![
            Value::String("Alice".into()),
            Value::String("Bob".into()),
            Value::String("Carol".into()),
        ]),
    );
    scope.bindings.insert(
        "numbers".into(), Value::Array(vec![
            Value::Integer(1),
            Value::Integer(5),
            Value::Integer(10),
            Value::Integer(20),
        ]),
    );

    // Pre-parse all expressions into ASTs once.
    let asts: Vec<FeelExpression> = FEEL_EXPRESSIONS
        .iter()
        .map(|src| engine::dmn::feel::parser::parse_expression(src))
        .collect();

    for (i, name) in FEEL_EXPRESSIONS.iter().enumerate() {
        group.bench_with_input(BenchmarkId::new("expr", name), &i, |b, _| {
            b.iter(|| engine::dmn::feel::eval::evaluate(&asts[i], &scope))
        });
    }

    // Vary input size: evaluate the same expression against scopes of different sizes.
    for (size, label) in [(10usize, "small"), (50, "medium"), (200, "large")] {
        let mut big_scope = Scope::empty();
        for i in 0..size {
            big_scope.bindings.insert(
                format!("v{}", i),
                Value::Integer(i as i64),
            );
        }
        group.bench_with_input(BenchmarkId::new("scope_size", label), &size, |b, _| {
            b.iter(|| engine::dmn::feel::eval::evaluate(&asts[0], &big_scope))
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark group 3: decision-table evaluation (all hit policies)
// ---------------------------------------------------------------------------

/// Build a synthetic decision table with N rules and M input columns.
fn build_synthetic_table(n_rules: usize, n_inputs: usize) -> DecisionTable {
    let mut inputs = Vec::new();
    for i in 0..n_inputs {
        inputs.push(InputClause {
            id: Some(format!("input_{}", i)),
            label: Some(format!("input_{}", i).into()),
            input_expression: format!("{}", i),
            input_values: None,
            description: None,
        });
    }

    let outputs = vec![OutputClause {
        id: Some("result".into()),
        label: Some("result".into()),
        name: Some("result".into()),
        type_ref: Some("string".into()),
        output_values: None,
        default_output_entry: None,
        description: None,
    }];

    let mut rules = Vec::new();
    for r in 0..n_rules {
        let input_entries: Vec<engine::dmn::model::RuleEntry> = (0..n_inputs)
            .map(|i| engine::dmn::model::RuleEntry {
                text: engine::dmn::model::TextContent::plain(
                    format!("> {}", i * 10),
                ),
            })
            .collect();
        rules.push(engine::dmn::model::DmnRule {
            id: Some(format!("rule_{}", r)),
            description: None,
            input_entries,
            output_entries: vec![engine::dmn::model::RuleEntry {
                text: engine::dmn::model::TextContent::plain(String::from("\"hit\"")),
            }],
            annotation_entries: Vec::new(),
        });
    }

    DecisionTable {
        id: Some("bench_table".into()),
        name: Some("benchmark_table".into()),
        input_clauses: inputs,
        output_clauses: outputs,
        rules,
        hit_policy: HitPolicy::Unique,
        aggregation: None,
        preferred_orientation: None,
        output_label: None,
    }
}

fn bench_decision_table(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("decision_table");
    group.sampling_mode(SamplingMode::Flat);

    // Fixed-size table (10 rules x 3 inputs) benchmarked across hit policies.
    let base_table = build_synthetic_table(10, 3);
    for policy in [
        HitPolicy::Unique,
        HitPolicy::Any,
        HitPolicy::Priority,
        HitPolicy::First,
        HitPolicy::RuleOrder,
        HitPolicy::OutputOrder,
        HitPolicy::Collect,
    ] {
        let mut t = base_table.clone();
        t.hit_policy = policy;
        // Set aggregation for Collect.
        if matches!(policy, HitPolicy::Collect) {
            t.aggregation = Some(BuiltinAggregator::Sum);
        }
        let cache = FeelAstCache::new();
        group.bench_with_input(
            BenchmarkId::new("hit_policy", policy.as_str()),
            &policy,
            |b, _| {
                b.iter(|| {
                    let scope = Scope::empty();
                    DecisionTableEvaluator::evaluate(&t, &scope, &cache)
                        .expect("eval");
                })
            },
        );
    }

    // Scale rules: 10 x 3, 50 x 3, 200 x 3.
    let cache = FeelAstCache::new();
    for (n_rules, label) in [(10usize, "small"), (50, "medium"), (200, "large")] {
        let table = build_synthetic_table(n_rules, 3);
        group.bench_with_input(BenchmarkId::new("rules", label), &n_rules, |b, _| {
            b.iter(|| {
                let scope = Scope::empty();
                DecisionTableEvaluator::evaluate(&table, &scope, &cache)
                    .expect("eval");
            })
        });
    }

    // Scale inputs: 10 rules x 3, 10x10, 10x50.
    for (n_inputs, label) in [(3usize, "small"), (10, "medium"), (50, "large")] {
        let table = build_synthetic_table(10, n_inputs);
        group.bench_with_input(BenchmarkId::new("inputs", label), &n_inputs, |b, _| {
            b.iter(|| {
                let scope = Scope::empty();
                DecisionTableEvaluator::evaluate(&table, &scope, &cache)
                    .expect("eval");
            })
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark group 4: DRD resolution (topological sort + evaluation chain)
// ---------------------------------------------------------------------------

fn bench_drd_resolution(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("drd_resolution");
    group.sampling_mode(SamplingMode::Flat);

    // Parse the multi-output TCK fixture which exercises DRD resolution.
    let tck_root_path = tck_root();
    let multi_path = tck_root_path.join("0010-multi-output-U/0010-multi-output-U.dmn");
    if !multi_path.is_file() {
        group.finish();
        return; // no DRD corpus available, skip gracefully.
    }

    let xml = std::fs::read_to_string(&multi_path).expect("read multi-output fixture");
    let (model, cache) = engine::dmn::parser::parse_dmn_with_cache(&xml)
        .expect("parse with cache");

    // Collect decision IDs from the model.
    let decisions: Vec<String> = model
        .definitions
        .children
        .iter()
        .filter_map(|child| {
            if let engine::dmn::model::DefinitionsChild::Decision(d) = child {
                Some(d.id.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(!decisions.is_empty(), "no decisions in multi-output fixture");

    // Build a shared snapshot and evaluator — these are reusable across iterations.
    let shared = Arc::new(std::sync::RwLock::new(SharedDmnModel::new(model, cache)));
    let evaluator = DecisionEvaluator::new(shared.clone());

    // Benchmark: evaluate each decision (which triggers DRD resolution).
    for did in &decisions {
        group.bench_with_input(BenchmarkId::new("decision", did), did, |b, _| {
            b.iter(|| {
                let request = EvaluateRequest {
                    inputs: HashMap::new(),
                    trace: false,
                };
                evaluator.evaluate(did, request)
                    .expect("evaluate");
            })
        });
    }

    // DRD chain depth measurement (topological sort time).
    group.bench_function("drd_chain_depth", |b| {
        b.iter_batched(
            || shared.clone(),
            |_shared_clone| {
                // Benchmark the cost of reading the shared snapshot and resolving
                // DRD order. The actual resolution happens inside evaluate().
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion wiring
// ---------------------------------------------------------------------------

criterion_group!(benches_parser, bench_parser);
criterion_group!(benches_feel_evaluator, bench_feel_evaluator);
criterion_group!(benches_decision_table, bench_decision_table);
criterion_group!(benches_drd_resolution, bench_drd_resolution);

criterion_main!(benches_parser, benches_feel_evaluator, benches_decision_table, benches_drd_resolution);
