# Known Tech Debt

Living list of tech-debt items surfaced but intentionally *not* fixed in the
housekeeping pass that created this file (2026-07-06). Each entry should become
a phase task or be struck when resolved.

## Engine: fact-retraction network cleanup (3 sites)

`retract_fact()` was made functional in the v4.0 hardening track (H1 — "Fixed
retract_fact() in Rete/TREAT/LEAPS, 187 tests pass"), but the *network cleanup*
half of retraction is still stubbed. Each engine leaves a TODO at the point
where the Rete/TREAT/LEAPS network nodes ought to be torn down or invalidated
after a fact is retracted:

- `engine/src/engine/rete/mod.rs:557` — `// TODO: Implement retraction with proper network cleanup`
- `engine/src/engine/treat/mod.rs:428` — `// TODO: Implement retraction with proper network cleanup`
- `engine/src/engine/leaps/mod.rs:452` — `// TODO: Implement retraction with proper network cleanup`

**Why deferred:** this is real engine work (correctness/performance of
truth-maintenance under retraction), not mechanical cleanup. It belongs in a
dedicated phase task with RED tests for retraction-then-re-derivation
scenarios, not in a housekeeping commit.

**Suggested home:** a future engine-correctness milestone (candidate alongside
or after v6.0 DMN), or a hardening phase modeled on the v4.0 H1 track.

## Multi-tenancy: v1 tenants route module not wired

`TenantStore` trait + `MemoryTenantStore` impl exist (v3.0/v4.0 hardening, F1),
and `tenant_id` is plumbed partway, but a `v1::tenants` route submodule is not
declared and tenant isolation on rule CRUD is incomplete (per the v3.0 "Track 3
~90%" note in STATE.md). The stale `// pub mod tenants;` placeholder in
`api/src/v1/mod.rs` was replaced with an accurate NOTE comment during
housekeeping; the underlying gap remains.

**Suggested home:** a governance/tenancy milestone.