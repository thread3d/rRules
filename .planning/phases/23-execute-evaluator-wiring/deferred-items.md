# Phase 23 — Deferred Items (out-of-scope discoveries)

Findings logged during plan execution that are outside the current task's
scope (pre-existing, unrelated files). Per the executor scope-boundary rule,
these are NOT fixed by the executing agent — they are recorded here for a
future phase or dedicated cleanup.

## 1. `cargo build -p engine --all-features` fails on the `metrics`/`prometheus` feature (PRE-EXISTING)

- **Discovered during:** Plan 23-06 Task 1 verification
- **File:** `engine/src/engine/metrics.rs:288-293`
- **Symptom:** `cargo build -p engine --all-features` emits 3 compile errors:
  - `error[E0308]: mismatched types` at `metrics.rs:289:20` (`namespace: "rrules"` — expected `String`, got `&str`)
  - `error[E0063]: missing field \`variable_labels\` in initializer of \`Opts\`` at `metrics.rs:288:16`
  - `error[E0308]: mismatched types` at `metrics.rs:293:23` (`const_labels` type mismatch)
- **Root cause:** The `prometheus` crate version pinned in the workspace
  exposes a `prometheus::Opts` struct whose fields differ from what
  `metrics.rs` constructs (likely a prometheus 0.13 -> newer API drift:
  `namespace` is now `String` not `&'static str`, and `variable_labels`
  is a required field). The `metrics` feature (`metrics = ["prometheus"]`)
  is the only thing `--all-features` enables beyond the four engine
  features.
- **Scope rationale:** Plan 23-06 touches `engine/src/engine/{treat,leaps,phreak}/mod.rs` and `engine/src/engine/direct_eval.rs` — none touch `metrics.rs`. The failure reproduces on the parent commit (`da05fe0d0`, before any 23-06 source changes) with the same 3 errors, confirming it is not caused by this plan.
- **Relevance to 23-06:** None. The plan's actual verification scope —
  `cargo build -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak`
  and `cargo check -p api --tests` (which transitively enables all four
  engine features) — both compile clean. The four-engine guarantee D-01/D-02
  is unaffected.
- **Suggested fix (for a future phase):** Update `metrics.rs:288-293` to the
  current `prometheus::Opts` API — `namespace: "rrules".to_string()`, add
  `variable_labels: vec![]` (or use `Opts::new(...)` builder). Bump or pin
  the `prometheus` workspace dep to a version whose `Opts` fields match the
  call site. Add a CI build with `--all-features` to catch this drift
  earlier.

## 2. Pre-existing dead-code warnings in `engine/src/engine/treat/mod.rs`

- **Discovered during:** Plan 23-06 Task 1 build
- **Files:** `engine/src/engine/treat/mod.rs:20` (`struct CacheEntry` never constructed), `:78` (`fn get_oldest_entry` never used)
- **Scope rationale:** These warnings pre-date Plan 23-06 (the
  `CacheEntry` struct and `get_oldest_entry` method are part of the
  original TREAT module scaffolding from v3.0). Plan 23-06 did not add or
  remove them.
- **Suggested fix:** Remove the unused `CacheEntry` struct and
  `get_oldest_entry` method, or wire them into the LRU cache eviction
  path. Low priority — warnings only, no functional impact.