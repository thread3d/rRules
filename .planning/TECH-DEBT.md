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

## ✅ RESOLVED (2026-10-25) — Multi-tenancy: tenants routes + rule isolation

`engine::governance::tenant` (types + `MemoryTenantStore` + `SqlxTenantStore`
placeholder) existed but was not declared in `engine/src/lib.rs`, and the API's
`v1::tenants` module was not declared in `api/src/v1/mod.rs`, so neither was
compiled. Wired end to end:

- `pub mod tenant;` in `engine/src/lib.rs`; the store gained synchronous
  `create`/`get`/`list`/`delete` cores (with `TenantStoreError` for
  empty/duplicate names) that the async `TenantStoreBackend` impl delegates to —
  one implementation, no duplication.
- `api/src/v1/tenants.rs` reuses the engine types instead of its own copies and
  is registered in `api/src/main.rs` (`GET/POST /v1/tenants`,
  `GET/DELETE /v1/tenants/{id}`) behind the `/v1` auth scope.
- `api/tests/tenants.rs` covers the CRUD flow, duplicate → 409, blank → 400,
  unknown → 404, delete idempotency, and per-tenant rule isolation (two
  `X-Tenant-ID` values never see each other's rules). 6/6 pass.

Remaining follow-up (not blocking): persist tenants in PostgreSQL rather than
the in-memory store, and decide whether deleting a tenant should archive its
rules.

