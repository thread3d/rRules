# Known Tech Debt

Living list of tech-debt items surfaced but intentionally *not* fixed in the
housekeeping pass that created this file (2026-07-06). Each entry should become
a phase task or be struck when resolved.

## ✅ RESOLVED (2026-10-25) — Engine: fact-retraction network cleanup

`retract_fact()` was a stub returning `Ok(false)` in all three Rete-family
engines (`engine/src/engine/{rete,treat,leaps}/mod.rs`), which left four
integration tests in `engine/tests/retract_integration.rs` permanently RED.

Fixed by implementing retraction end to end:

- `BetaMemory::remove_fact` / `BetaMemory::contains_fact` — drop every nagram
  that references the retracted fact and prune empty join-node buckets.
- Each engine's `retract_fact` now removes the fact from `working_facts`,
  `alpha_memory`, and `beta_memory`, returning `false` for an unknown id
  (idempotent re-retract).
- TREAT drops its join-result LRU cache; LEAPS resets its aggregation nodes
  (`AggregationNode::reset`) instead of serving an aggregate derived from
  retracted facts.

Verification: `cargo test -p engine --test retract_integration --features
engine-rete,engine-treat,engine-leaps,engine-phreak` → 8/8 pass (was 4/8), plus
two new unit tests in `rete/mod.rs` covering alpha/beta removal.

## Multi-tenancy: v1 tenants route module not wired

`TenantStore` trait + `MemoryTenantStore` impl exist (v3.0/v4.0 hardening, F1),
and `tenant_id` is plumbed partway, but a `v1::tenants` route submodule is not
declared and tenant isolation on rule CRUD is incomplete (per the v3.0 "Track 3
~90%" note in STATE.md). The stale `// pub mod tenants;` placeholder in
`api/src/v1/mod.rs` was replaced with an accurate NOTE comment during
housekeeping; the underlying gap remains.

**Suggested home:** a governance/tenancy milestone.