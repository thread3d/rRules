# DMN Subsystem Benchmarks (Phase 36)

This directory contains benchmark infrastructure for the rRules DMN subsystem. These benchmarks measure parser throughput, FEEL evaluator latency, decision-table evaluation time, and memory consumption — providing regression baselines for Phase 37+ optimization work.

## Running Locally

### Criterion Benchmarks

Run all four benchmark groups (parser, feel_evaluator, decision_table, drd_resolution):

```bash
make bench
# or equivalently:
cargo bench --bench dmn_benchmarks -p engine
```

This produces Criterion output with ops/sec and allocation counts for each benchmark group. Results are printed to stdout and can also be found in `target/release/.cargo-benchjson/`.

### Memory Baseline Snapshot

Generate a memory baseline against the v6.0 corpus:

```bash
make memcheck
# or equivalently:
cargo run -p engine --bin memory_baseline
```

This writes a JSON snapshot to `engine/benches/snapshots/v6.0-baseline.json` containing:
- Cold-start RSS (bytes)
- Per-phase RSS deltas and allocation counts (parse, evaluate, drd_resolution)
- Peak RSS across all phases
- Total wall-clock duration

## Benchmark Corpus

The benchmarks use two sources of DMN files:

1. **TCK fixtures:** `engine/tests/fixtures/tck/` — 21+ vendored OMG TCK vectors spanning hit policies (UNIQUE, ANY, PRIORITY, FIRST, RULE ORDER, OUTPUT ORDER, COLLECT), arithmetic, date/time, list functions, and multi-output decision services.

2. **Import-test corpus:** `api/dmn/` — DMN files from the import-test suite used by Phase 29+ integration tests.

The combined corpus is loaded automatically; no configuration needed.

## Running the gate

Benchmarks and the memory-regression gate run locally; the project has no hosted CI at present
(the GitHub Actions workflows were removed because they could not run without a configured remote,
cluster and secrets).

```bash
make bench       # Criterion DMN benchmarks, results under target/criterion/
make memcheck    # regenerate the snapshot and compare it to the frozen v6.0 baseline
make baseline-update   # accept new numbers as the baseline (commit the result)
```

`make memcheck` compares current measurements against the frozen `v6.0-baseline.json` and exits
non-zero when RSS or allocation counts regress beyond the threshold:

| Variable | Default | Description |
|----------|---------|-------------|
| `BENCH_RSS_THRESHOLD_PCT` | 10 | RSS regression threshold (% above baseline) |
| `BENCH_ALLOC_THRESHOLD_PCT` | 10 | Allocation count regression threshold (%) |

It is worth running before a release, and whenever the parser or evaluator changes shape.

## Benchmark Groups

### parser
- **What:** Parse .dmn XML files into canonical IR (`DmnModel`).
- **Metrics:** ops/sec, allocations per file, corpus-level throughput.
- **Variants:** Standard parse vs. `parse_dmn_with_cache` (FEEL AST pre-parsing).

### feel_evaluator
- **What:** Evaluate representative FEEL expressions against a Scope.
- **Metrics:** ops/sec per expression, scaling with input scope size.
- **Expressions tested:** Arithmetic, comparison, boolean/ternary, list functions, context access, quantified, for-expressions, date/time literals.

### decision_table
- **What:** Evaluate synthetic decision tables under each hit policy.
- **Metrics:** ops/sec across policies and table sizes.
- **Hit policies:** UNIQUE, ANY, PRIORITY, FIRST, RULE ORDER, OUTPUT ORDER, COLLECT (with SUM aggregator).
- **Scales tested:** 10×3, 50×3, 200×3 rules; 10×3, 10×10, 10×50 inputs.

### drd_resolution
- **What:** Topological sort + evaluation chain for multi-output decision services.
- **Metrics:** ops/sec per decision, DRD chain depth distribution.
- **Corpus:** `0010-multi-output-U` TCK fixture (the only multi-decision model in the vendored set).

## Adding New Benchmarks

To add a new benchmark group:

1. Add a `bench_*` function in `engine/benches/dmn_benchmarks.rs`.
2. Create a new `criterion_group!` and add it to `criterion_main!`.
3. (Optional) Add corresponding memory profiling to `engine/benches/memory_baseline.rs` if the benchmark affects RSS/allocations significantly.

## Notes

- Benchmarks are **measurement only** — they do not optimize. Phase 37+ handles actual performance improvements.
- The baseline snapshot (`v6.0-baseline.json`) is committed to the repository and serves as the regression floor for nightly checks.
- If you change benchmark code or corpus files, regenerate the baseline with `make memcheck` and commit the updated snapshot.
