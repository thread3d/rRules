# Changelog

All notable changes to rRules are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

> Note on early tags: `v1.0`–`v3.0` were backfilled retroactively (see
> `.planning/RETROSPECTIVE.md` "Cross-Milestone Trends"). `v3.0` in particular
> is a historical ref, not a clean release commit — its pluggable-engine code
> landed inside the v4.0 timeline. Entries below begin at v4.0, the first
> milestone tagged at ship time.

## [Unreleased]

## [9.0.76] — 2026-10-28 — The tenant question finally has an answer a caller cannot forge

Tenant-scoped execution — the thing 9.0.75's boundary stands in for — was blocked by something more basic
than the ruleset: **there was no trustworthy tenant identity.** `X-Tenant-ID` was the only mechanism, and
any authenticated caller could set it to any value, which the specification has said for several releases.
Scoping execution by that header would have looked like isolation and been none.

So this round fixes the prerequisite:

- `UserRecord` gains `tenant_id` (the four demo users live in `default`, which is what every document and
  rule implicitly used before);
- `auth::Claims` gains `tenant_id`, minted from the user record, `Option` + `#[serde(default)]` so tokens
  issued before this still decode;
- `extract_tenant_id` **prefers the token**: a request holding one acts for that tenant whatever the header
  says. The header remains only as the fallback for pre-claim tokens and for handlers reached without the
  auth middleware (the in-process test apps) — an unauthenticated routing hint, not an identity.

`api/tests/auth_protection.rs::a_token_tenant_overrides_a_forged_header` asserts both directions: a token
for `acme` creates and then lists its own rule while sending `X-Tenant-ID: someone-else`, and a caller
sending that header *without* a token sees nothing. Removing the token preference makes the first assertion
fail with `total_count: 0`, so the test is testing what it claims.

**CodeWitness then rejected the first version of this change, correctly.** `API-003` ("Rules are isolated per
tenant") went from VERIFIED to FAILED with the counterexample `201|1` where `201|0` was required. Reading the
generated scenario showed why: it logs in **once** as `admin` and switches tenants with the header — the
"admin manages several tenants" flow, which pinning every caller to their token's tenant broke.

So the rule is split by role, which is also the honest design: an **admin's** `X-Tenant-ID` selects the tenant
(that is what the role is for), while for everyone else the **token's tenant is authoritative** and a forged
header reaches nothing. Both halves are tested — the non-admin overrule, and the admin isolation flow that
was the counterexample. That is the loop working as intended: a normative requirement was load-bearing, and
the implementation had to answer to it rather than the other way round.

The write boundary of 9.0.75 is unchanged: execution is still deployment-wide, so only the default tenant may
add an executing rule. What changed is that "which tenant is asking?" now has an answer a caller cannot
forge — the prerequisite for lifting that boundary, not the lift.


## [9.0.75] — 2026-10-28 — The leak my own last release opened

9.0.74 made the API write rules into the set the engine executes. That set is **deployment-wide**, and
`POST /v1/execute` carries no tenant at all — `X-Tenant-ID` scopes the document store and nothing else. So a
rule created on behalf of tenant `acme` would have run for every tenant in the deployment. I introduced that
yesterday; this closes it the same way the rest of this loop closes things — with a boundary that refuses and
explains, not a silence.

`write_executable_rule` refuses any tenant other than `default`:

```
400 {"error": "executable rules are deployment-wide, not tenant-scoped: a rule created for tenant 'acme'
     would run for every tenant. Create the document without `rule`, or use the default tenant."}
```

Nothing is written in that case. A *document* without `rule` is still accepted for any tenant, so
multi-tenant deployments keep their CRUD, categories and approvals — they simply cannot add an executing
rule until execution is tenant-scoped.

The test asserts both halves: the refusal (400 with the reason, and the directory left empty) and that the
same tenant's document is still created, reported `executable: false`. Removing the boundary turns it into
`201`, so it is testing what it claims.

`specs/rule-sources.md` records the boundary and the real gap it stands in for: **execution has no tenant
context**. That is the next structural item, and it is a bigger change than the write side — it needs a
tenant on the execute path, a per-tenant ruleset, and a decision about what a session's tenant is.


## [9.0.74] — 2026-10-28 — A rule created through the API can now be a rule the engine runs

The first slice of `specs/rule-sources.md`'s option A, and the first round in a while that *adds* a
capability rather than removing a lie.

`POST /v1/rules` accepts an optional **`rule`** — the engine's own `Rule` JSON. When present:

1. the API **imposes the document's identity** on it, so the id returned to the caller is the id the engine
   reports and the id `rule_filter.name_pattern` matches. Without this the bridge was built but did not
   join: the document had a UUID, the rule had whatever the client typed, and a filtered run still matched
   nothing;
2. the engine validates it by loading it, so a rule that would not execute is a `400` and nothing is stored;
3. it is written to `RULE_STORAGE_PATH/<id>.rules`, and the ordinary reload makes it part of the executing
   set.

`RuleResponse.executable` reports which documents have a rule behind them. A document created with only a
`dsl_body` is stored and reported `executable: false` — there is no parser from that text to a rule, and the
response says so instead of leaving the caller to infer it from a zero.

**And a drift this slice exposed:** `build_ruleset_from_dir` had its own file parser accepting only a bare
rule or an array, while `Engine::load_rules` accepted those *and* the `{"rules": [ … ]}` envelope. The API
writes the envelope, so its own directory loader skipped its own file — silently, behind a `tracing::warn!`
that a test never reads. `engine::environment::parse_rules_document` is the one parser now, and both callers
use it; that is why the newly written rule loads.

Also collapsed: `RuleResponse` was built by **three** `From` implementations with identical field lists —
adding `executable` meant patching three places, and I patched two before the compiler named the third. One
canonical conversion, two delegating impls.

Both halves are pinned: a document without a `rule` reports `executable: false` and matches no executed rule;
a document with one fires, and `rule_filter` selects it by the id the create call returned.


## [9.0.73] — 2026-10-28 — The two rule universes, written down

9.0.72 refused `rule_filter.categories` because the field assumed a link that has never existed. This
round writes that link down instead of discovering it again: **`specs/rule-sources.md`** states that rule
*documents* (`/v1/rules`, an in-memory `HashMap<(tenant, Uuid), StoredRule>`) and the rules the engine
*executes* (`RULE_STORAGE_PATH/*.rules`) are separate sets that nothing converts into one another — no code
parses `dsl_body`, no code writes `.rules` files, and a restart loses every document while keeping every
executed rule.

What that costs, plainly:

- **The rule-authoring IDE does not author executable rules.** Documents and execution share a product, not
  a data path.
- **"Test this rule" tests nothing.** `TestRunner.tsx` sends the document's UUID as `name_pattern`, and a
  document's UUID never equals a disk rule's id, so the run matches zero rules and says so only as
  `rules_matched: 0`.
- Categories, tags, owners, status and approvals describe documents that cannot run; salience, conditions
  and actions describe rules that cannot be edited through the API.

The document lays out four options with what each would take to verify, recommends **compiling documents
into the executing set** (or storing rules as JSON and writing `.rules` on create/update, so the directory
and the documents agree), and rejects the status quo — the gap was invisible from outside, which is how it
survived this long.

It is pinned rather than merely described:
`api/tests/execute_integration.rs::a_created_rule_is_not_executable_and_its_id_matches_no_executed_rule`
creates a rule through the API, executes, and asserts that only the disk rule matches — and that filtering
by the created rule's own id matches nothing. If someone half-bridges the two universes, that test fails
and points at the document that has to change first.

Mounting `POST /v1/rules` in the integration app also rediscovered a hazard `AGENTS.md` warns about: the
handler extracts **two** `web::Data` dependencies (`RuleStore` and `AuditStore`), and a hand-rolled app that
installs neither answers **500 at runtime**, not a compile error.


## [9.0.72] — 2026-10-28 — A filter that filtered nothing, answered instead of ignored

`POST /v1/execute` accepted `rule_filter.categories` and did nothing with it: the executing ruleset is
loaded from `RULE_STORAGE_PATH` and carries no category, while the API's categories describe rule
**documents** in a separate store keyed by tenant. So a caller who sent a category filter believed they had
narrowed the run and got every rule. `docs/API.md` advertised the field — "restrict categories" — and the
frontend's request type declared it.

I set out to implement it and concluded that the honest move is the opposite. Making it work means either a
third category notion on the engine's `Rule` (matching neither the API's category entity nor the rule
store's string, across 54 struct literals) or threading the rule store into the execute path — which,
per `AGENTS.md`, makes every hand-rolled test app that mounts the handler answer 500. Both add a liability
to honour a field that **no client sends**: the TestRunner passes `name_pattern` and nothing else.

So the field is **refused with a 400** that names it and explains why, `name_pattern` still works, and the
docs and the frontend type no longer advertise what cannot happen. The integration test asserts both
halves: the category filter is a 400, and a `name_pattern` filter still returns 200.


## [9.0.71] — 2026-10-28 — A field nobody honoured, and an engine state nobody extracted

`POST /v1/rules/reload` accepted `rule_file` and dropped it on the floor: a caller asking to reload one
file got a whole-directory reload and no indication that their instruction had been discarded. The spec
recorded it; `docs/API.md` even showed a request using it.

There is no per-file reload to implement cheaply. The ruleset is built from the directory and keeps no
per-rule provenance, so the nearest thing — swap in one file's rules and drop the rest — would silently
disable every other rule. So the field is **refused**: a request carrying `rule_file` gets a 400 that names
the file and explains why, and the documentation now says what reload actually does. `docs/API.md` also
claimed the async response reports `"status": "running"` when the handler answers `"pending"`; corrected in
the same pass.

**`EngineState` is deleted** — a type with no production users, carrying a `rule_files` provenance map and
two `reload_*` methods that the live path (`reload_and_swap`, `build_ruleset_from_dir`) never called. It
survived only because a test helper installed it as `app_data` *for a handler that no longer extracts it*:
`api/src/app.rs` built one at startup and registered it, and nothing ever asked for it. The test
scaffolding went with it.


## [9.0.70] — 2026-10-28 — The trace promised events nobody emitted, and a route nobody wrote

`engine/src/dmn/evaluator/trace.rs` described itself as recording `evaluate_table`, `match_rule`,
`aggregate` and `resolve_reference` events and serving them from `GET /v1/decisions/{id}/trace`. None of
that was true: two event kinds exist, two were never emitted, and the route was never written.

The measurement, now pinned by `engine/tests/dmn_trace_events.rs`, is that a single-decision evaluation
records `evaluate_table` (with the hit policy in the message and the produced value in `output`) followed
by `merge_scope` (with the merged context); a decision-requirement graph adds `drd_resolve`. `match_rule`,
`aggregate` and `resolve_reference` are never emitted. The one consumer is the evaluation response, which
publishes `event_count` and `duration_ms` — not the events.

**My first version of that test asserted the opposite** — that a lone decision records *no* events — and
it failed within a minute, because the trace is richer than I assumed. That is the whole reason the test
exists; the description had the same problem for the same reason.

`TraceEvent`'s `rule_index` and `inputs` fields existed for events that never occur and are deleted;
`output` stays, because two event kinds populate it. And `DeadRuleConfig` — a `#[deprecated]` type with no
users whose only field (`inactivity_threshold`) nothing read — is deleted too, found by sweeping struct
fields nothing accesses.


## [9.0.69] — 2026-10-28 — The test framework this library ships could not fail

`ForwardChainingTester` is part of the public testing surface: it offers `assert_rule_sequence`,
`assert_fired_count`, `assert_fired_in_order` and `assert_rule_not_fired` for downstream rule tests. Two
defects made it useless in complementary ways:

- **`add_rule` never added anything.** It built the **DSL** text `"rule <id>"` and passed it to
  `load_rules`, which parses **JSON** — so every call failed and the error was discarded with `let _ =`.
  A test could add a rule, run, and assert about a rule set that was empty. It calls `Engine::add_rule`
  now and returns the rejection instead of swallowing it.
- **`execute_forward` never recorded what fired.** It cleared the trace, ran the engine, and returned
  the report while leaving `activations` empty — so `assert_fired_count(0)` and
  `assert_rule_not_fired(anything)` passed for every engine, however it behaved. The report's
  `fired_rules` ids are mapped to rule **names** into the trace now, which is the contract
  `activation_trace` documents.

Two smaller things: `assert_fired_in_order` advanced its cursor *to* the match rather than past it, so a
repeated expectation could match a single firing twice; and `with_rules` logged nothing when a rule was
rejected.

**The tester's own tests are why this survived**: they injected activations by hand through
`activations_mut()`, so the assertions were tested and the tester was not. There are now two tests that
drive it against a real engine — one where a rule fires, one where nothing matches — and the first fails
if either defect comes back.

`BackwardChainingTrace` is deleted: both of its "recorders" were empty and nothing called the type. The
real trace is `backward_chaining::InferenceTrace`, which `Engine::execute_backward` returns and
`explain()` renders.


## [9.0.68] — 2026-10-28 — Settings that were stored and never read

Continuing the sweep for work that is computed and then discarded, this round read `#[allow(dead_code)]`
and found four settings that promised behaviour and delivered storage:

- **`EngineBuilder::enable_debug_trace`** asked for a trace the evaluator was never told to collect — the
  fixpoint loop passed `collect_trace: false` unconditionally and discarded whatever came back. It is
  honoured now: with the flag set, `Engine::last_trace()` returns the run's `TraceData`.
- **`EngineBuilder::default_salience`** is **deleted.** A rule's `salience` is an `i32` that always has a
  value, so "the default for rules without one" is a distinction the type cannot express. A knob that
  cannot keep its promise should not exist; its two tests only asserted that the value was stored.
- **`Session`'s `stateful` flag** was stored and never read, so "stateless" was a word in a signature. A
  stateless session now clears its working memory when a run completes, which is what single-use means;
  a stateful one keeps its facts.
- **`WorkingMemory.changes`** — a `Vec<WorkingMemoryChange>` kept "for session isolation" that nothing
  ever pushed to or read, plus the enum it carried. Session isolation is done by cloning the engine.

`api/src/v1/decisions.rs`'s `extract_decisions` — 12 lines no code called — is gone too.

`specs/domain-model.md` claimed `EngineBuilder::max_iterations` was "stored but never read by execution";
it has been passed to the fixpoint loop as its cap since 9.0.52, so **that claim was stale**, the second
such entry closed in three rounds.

`engine/tests/session_and_trace.rs` pins the three behaviours, and each fails when the behaviour is
reverted: no trace without the flag, a trace with it, a stateless session that clears what it ran on, and
a stateful one that keeps it.


## [9.0.67] — 2026-10-28 — The what-if simulation answered "success, nothing happened"

`POST /v1/simulation` is a live, documented endpoint. Its runner built a `WorkingMemory` and a `Fact`
per test fact **and discarded both**, called `execute_forward()` and discarded the result, then returned
`rules_fired: []`, `output: null` and `success: true`. The handler, meanwhile, built an engine with no
rules at all. So a user asking "what happens if I change this rule?" got a confident answer that
simulated nothing — the exact shape of defect this loop exists to find, and the worst kind: a feature
that looks implemented.

Four things were wrong, and each hid the next:

1. **No rules.** The handler built `EngineBuilder::new().build()` and never loaded anything. The request
   now carries `rules` (the same JSON `POST /v1/execute` accepts), and unparseable rules are a 400.
2. **The facts were thrown away** — `let _fact = Fact::new(…)` inside a loop, against a
   `let _working_memory` that was never attached to the engine. The runner inserts them now.
3. **The result was thrown away** — `let _execution_result = …`, with `rules_fired` hardcoded to an
   empty `Vec`. `forward_chaining::ExecutionResult` now carries `fired_rules` (the ids it already had and
   dropped), the run's added and retracted facts become the variant's `output`, and `success` reflects
   whether the run reported errors.
4. **The changes were never applied.** All six `RuleChange` kinds are implemented — `AddCondition`
   conjoins, `RemoveCondition` drops the `Pattern` leaf (a conjunction that loses its last leaf becomes
   `True`, since a rule with no conditions left fires), `ModifyAction` replaces at an index,
   `RemoveRule` deletes, `AddRule` inserts, `DisableRule` sets the condition to `False` — with the errors
   the types already declared (`RuleNotFound`, `InvalidActionIndex`, …) finally reachable.

A fifth defect fell out on the way: the handler built each simulated fact's payload with
`serde_json::to_value(f)` — **the whole fact**, not its `fields` — so every fact carried keys named `id`
and `fields` and none of its own attributes. Invisible while the facts were discarded; fatal the moment
they were inserted.

`api/tests/simulation_integration.rs` now asserts the behaviour instead of the shape: a satisfied rule
is reported as fired by id, a `DisableRule` variant reports nothing fired **and** produces a difference,
and unparseable rules are a 400.


## [9.0.66] — 2026-10-28 — The newest DMN namespace was not in the list, and the check was thrown away

Gap 20 in `specs/dmn-feel.md` had three parts, all true:

- **The DMN 1.5 URI was missing.** The table carried `20200524` (labelled 1.5), `20180521` (labelled 1.3)
  and the 1.1 XSD namespace — so the URI real DMN 1.5 documents declare, `20230324`, was not recognised
  at all, including in **51 files of the vendored TCK**. All four the repository actually uses are in
  the table now, labelled by version, with a comment saying the list is evidence-backed rather than a
  claim about every URI OMG has published.
- **The check's result was discarded.** `let _recognized = …contains(ns)` computed an answer and dropped
  it. `parser::is_known_dmn_namespace` is public now and `DmnModel::namespace_is_recognised` reports it
  on the parsed model, so a caller can ask.
- **The fixtures' names encoded the wrong mapping.** `decision_1_3.dmn` declared the 1.2 namespace and
  `decision_1_5.dmn` the 1.3 one — the spec had recorded the correct mapping and the test data
  disagreed with it. They are `decision_1_2.dmn` and `decision_1_3.dmn` now, joined by a genuine
  `decision_1_5.dmn` in the `20230324` namespace, and each has a test: every shipped fixture must be
  recognised, a foreign namespace must parse while reporting itself unrecognised, and the 1.2 document
  must round-trip with its namespace intact.

**Three error variants no code could construct are deleted** (gap 21): `DmnParseError::InvalidNamespace`
— unreachable by design, since unknown namespaces parse on purpose — and `DecisionTableError::
InvalidOutputValue` plus `DecisionEvalError::EvaluationFailed`, which had no construction site. An error
variant nothing can produce is a promise to callers that cannot happen. This is an API break for a
matcher on those enums; neither appears in the HTTP surface.


## [9.0.65] — 2026-10-28 — `matches` and `replace` accept the flags DMN gives them

Two entries in `specs/dmn-feel.md`'s gap list said the same thing from different angles: `matches`
declared two parameters and ignored the third, so **`matches("abc", "ABC", "i")` answered `false`** — a
wrong answer, not a missing feature. DMN 1.5 §10.3.4.5 gives `matches` and `replace` an optional flags
argument carrying the XPath flag set.

Both now honour `i`, `s`, `m` and `x`; an unknown flag answers null, because an error is null in FEEL.
The compiled-pattern cache is keyed by **pattern and flags**: keying on the pattern alone would answer a
case-insensitive call from the case-sensitive entry compiled earlier for the same pattern — the same
defect class as a rule cache keyed by node rather than by fact, which cost a round in 9.0.56. The test
that proves it compiles the plain pattern, then the flagged one, then checks the plain one again, and it
fails when the key is narrowed back to the pattern.

`matches` and `replace` now declare `flags` in the signature table, so a call with extra arguments is
checked against something true.


## [9.0.64] — 2026-10-28 — Four gaps closed, one of them a gap that was not there

Working through `specs/dmn-feel.md`'s remaining gap list, four entries resolved — and the first was the
mirror image of this loop's usual finding:

- **A *stale* gap: `Value::Range` equality was listed as a `TODO` returning null, and it works.**
  `ranges_equal` compares bounds *and* inclusivity, because the TCK distinguishes `(=10)` from
  `[10..10]`. `[1..2] = [1..2]` is `true`, `[1..3]` is `false`, `(1..2]` is `false`, `!=` is `true`. The
  specification was asserting a defect that no longer existed — the same drift as code keeping an old
  bug, in the other direction — and nothing had checked it.
- **`get entries`' description contradicted its own body.** The doc said it returns `"key:value"`
  strings; the body correctly returned `{key, value}` contexts per DMN §10.3.4.4. The body was right and
  the sentence above it was wrong.
- **`list replace` answered an empty list for an uncompilable pattern** while `matches`, `replace` and
  `split` all answer null. An empty list says "this list has no elements", which is a different and
  false claim.
- **The evaluator wrote to `stderr`.** Two reject paths (`no body`, `external`) used `eprintln!`, so a
  pure evaluator put diagnostics on the embedding process's stream. Both answer null silently now.

`engine/tests/feel_gaps_closed.rs` pins the three observable behaviours, so none of them can regress into
a gap entry again.


## [9.0.63] — 2026-10-28 — `make preflight`: measure the machine before trusting a gate

The lesson from 9.0.62 was that a slow environment and a broken build produce the same report. That is
now a gate rather than a paragraph, and `make ci` runs it first:

- **disk** — free space on the workspace volume against a 15 GB floor (the 862 GB incident had 92 GB
  free and every build was thrashing);
- **`target/`** — its own size against a 100 GB ceiling, because an oversized cache costs more in
  thrashing than a cold rebuild costs in time;
- **cargo home** — writable or not, with the exact `CARGO_HOME=$PWD/.cargo-home` hint this sandbox
  needs, since a read-only home makes cargo fail with a message about the *registry* and every gate
  that shells out to cargo then fails for the wrong reason;
- **cache variables** — `XDG_CACHE_HOME`, `npm_config_cache`, `HELM_CACHE_HOME` when set.

It prints what it measured, so a green line reads as "this machine can be trusted with a build". Proven
by stubbing the disk to 5 GB free and watching it fail with both reasons.

`AGENTS.md` records the habit beside the others: check `df -h .` and `du -sh target` before blaming a
gate, and note that minutes of wall time against seconds of CPU means the machine, not the code.


## [9.0.62] — 2026-10-28 — The verification was not broken; the disk was

Two rounds of "`make verify` reported all 35 BLOCKED" ended in the same place: the api half's build
exceeded its cap with **zero diagnostics**, which is not what a compile error looks like. The cause was
not the code and not the tool: `target/` had grown to **862 GB** on a filesystem **95% full**, and every
build was thrashing — a cold full build of the workspace took over eight minutes, and the same build by
hand showed 8m16s of wall time against 1m10s of CPU, which is waiting, not working. After deleting
`target/` and `db_tests/target` (412 GB free again) the whole workspace builds and runs 1010 tests in
**57 seconds**.

**A real defect was found on the way, and fixed.** Adding `db_tests` as a workspace *member* in 9.0.58 put
`pg-embed` back into every workspace-wide build — the engine's verification build had doubled from ~11 to
20 minutes — which is exactly what that crate existed to prevent. It is now `exclude`d from the workspace
with its own `[workspace]` table, built by manifest path, and `make verification-cost` grew a rule for
it: a workspace member with a **required, non-optional** dependency on a heavy crate fails the gate
(optional and dev-dependencies are legitimate — `api`'s `sqlx` sits behind the `postgres` feature).

`make lint` and `make db-test` follow the crate to its manifest; `-p db_tests` no longer resolves.


## [9.0.61] — 2026-10-28 — The IR spoke a private dialect, and the client types were fiction

Two mismatches at the same seam, one of them a wire-format bug.

**`hit_policy` did not serialise as DMN spells it.** `HitPolicy` and `BuiltinAggregator` derived `Serialize`
with no renames, so the IR endpoint's JSON said `"RuleOrder"` and `"Sum"` while `as_str()` — the string the
exporter writes into `<decisionTable hitPolicy="…">` — says `"RULE ORDER"` and `"SUM"`. XML and JSON
disagreed, and anything comparing `hit_policy === 'COLLECT'` never matched. Both enums now carry explicit
`#[serde(rename = …)]` for every variant, and `enum_wire_tests` asserts that JSON equals `as_str()` for
each one and that the names round-trip. This is a wire change: it affects the IR endpoint's JSON only —
the model is never persisted as JSON, and the database store keeps XML bytes.

**The decision client described an API that does not exist**, and `specs/http-api.md` had recorded it
since the table was written: `listDecisions` expected a `model_version` the backend never sends, and
`getDecision` expected a `description/question/decision_table` that only `/ir` returns. Nothing failed,
because nothing called them — a type that invents a field waits quietly for its first caller to be
silently wrong. The types now match `DmnSummary`, the document endpoint, and `DmnIrResponse`.

**And the class is now checkable.** `make frontend-types` compares each declared pair — handler or struct
on the Rust side, interface on the TypeScript side — and fails when their key sets differ, error
payloads excepted. It reproduced the documented defect before the fix (`DecisionListResponse: declares
['model_version'], which list_decisions never sends`) and passes now. Five pairs are in the table; adding
one is how a new client boundary becomes a contract.

`frontend/src/api/__tests__/decisions.test.ts` pins the five wrappers against a stubbed client at run time;
the type check covers what a test cannot see, since types vanish before the test runs.


## [9.0.60] — 2026-10-28 — The mapping layer that silently broke had no test

The seam between the API's JSON and the interface's view models is where a real bug lived: *"every field
the API did return arrived as `undefined` — dates included, which is why sorting by date compared
`NaN`"*. It was fixed, and the fix was commented, and **nothing tested it**. A regression would look like
a rendering quirk — rows with blank names and invalid dates — not like a mapping bug, which is exactly
how it survived long enough to need a comment.

`frontend/src/api/__tests__/rules.mapping.test.ts` now drives `getRules` and `getRule` against a stubbed
client and pins the boundary: every field the API sends arrives camelCased and populated, the paginated
envelope's `rules` array is the one read (the server renames `items` to `rules`, so a client reading
`items` would show an empty list for a non-empty page), an unknown status falls back to `draft` instead of
passing through, and a rule with none of the optional fields still maps. Proven by reverting the mapping
to `createdAt: ''` and watching the first test fail.

The frontend suite is 32 tests across 6 files now, all green, and the production build is unchanged.


## [9.0.59] — 2026-10-28 — A gate on the gate: the verification cannot be starved again

9.0.58's repair was necessary but not sufficient: it put the database tests where they belong, and left
nothing keeping them there. `make verify` had been reporting "35 verified" while quietly running out of
budget for ten releases, and only a timeout threshold exposed it — a failure mode that returns on the
next heavy dev-dependency.

`make verification-cost` now measures what the requirement verification has to build and fails when it
grows: `api` at 262 crates (normal + dev) against a ceiling of 320, `engine` at 198 against 330, and a
forbidden list — `pg-embed`, `sqlx`, `reqwest`, `zip` — that must never be a dev-dependency of a verified
crate. The ceilings sit just above today's numbers and far below the ~600 the embedded-server tree added,
so ordinary churn passes and a repeat does not.

It runs in `make ci` and in CI, and it is proven the same way as every other check here: with a stubbed
tree carrying 400 crates and `pg-embed` + `zip` as dev-dependencies, it fails with all three reasons
before the real tree passes again. `AGENTS.md` records the lesson as *"a gate can be starved into
silence"* — the third habit, beside "verify in the mode the gate runs in" and "test the checker against
the defect it was written for".

`AGENTS.md`'s roadmap was also rewritten: it still listed backward chaining and LEAPS aggregation, both
of which closed in 9.0.54 and 9.0.55.


## [9.0.58] — 2026-10-28 — Three modules nothing used, one of which never even compiled

A sweep for `pub fn`s that nothing references outside their own file turned up three modules:

- **`engine/src/testing/report/` — 573 lines that had never been compiled.** No module declared
  `mod report;`, so the HTML report generator was not part of the crate at all. It also imported
  `crate::testing::RuleCoverage`, a name that does not exist there, which is why it could not have
  compiled had anyone declared it. This one was not in the spec's gap list: the earlier orphan hunt
  looked for modules that were *declared but unused* and missed the ones that were never declared.
- **`engine::coverage` — 326 lines**, a `CoverageTracker` with different `record_fire` and percentage
  semantics from the singleton the engines actually record into. Spec gap 11.
- **`engine::metrics` — 381 lines**, a second metric stack with a `prometheus` module gated on a feature
  no build declared, so it never compiled either. Spec gap 10.

All three are deleted; `monitoring::coverage::CoverageTracking` and `engine::engine::metrics` are the
only implementations now. Fifteen tests went with them — they were tests **of** the deleted code, which
is why the suite count falls while nothing else changes. The spec's gaps 10 and 11 are struck with this
release, and the never-compiled module is recorded as gap 12.

**Then the requirement verification broke, and it was my fault from 9.0.45.** `make verify` reported all
35 requirements BLOCKED: the api half could not finish its build inside the budget. The cause was
`pg-embed` sitting in `api`'s **dev-dependencies** (added in 9.0.45 for the PostgreSQL tests), so every
build *and every coverage pass* of that crate compiled the embedded-server tree; raising the timeout did
not help because the coverage phase then exhausted the run's 30-minute ceiling instead.

The three API-level database tests moved to their own crate, `db_tests/`, which depends on `api` with
the `postgres` feature. The tests are unchanged and all eleven still pass through `make db-test`; the
api build is back to a fraction of a second and the union is **35 verified / 0 failed / 0 unverified**
again. `db_tests/Cargo.toml` carries the reason in its header, because a test that makes the gate
verifying everything else stop working is in the wrong crate.


## [9.0.57] — 2026-10-28 — A comment said lambda application was a stub; it had worked all along

Chasing what `specs/dmn-feel.md` listed as an unimplemented lambda path, I found a comment in
`eval.rs` — *"lambda application is not yet implemented — stub returns Null"* — and a test named
`boxed_function_invocation_bound_callee_stub` asserting `Null`. Both were **stale**: they describe an
arity mismatch (a function declaring no parameters, carrying no body), not a missing feature. Probing
the evaluator showed the machinery working:

```
(function(x) x * 2)(4)                  → 8
for f in [function(x) x * 2] return f(3) → [6]
{ f: function(x) x * 2 }.f(3)            → 6
sort([3, 1, 2], function(a, b) a > b)    → [3, 2, 1]
```

Nothing pinned any of it. `engine/tests/feel_functions.rs` does now — six cases including lexical
capture (the closure sees its *definition* site's `n`, not the caller's) and the two wrong-arity calls
that correctly answer `null` instead of guessing. The stale test is renamed for what it asserts and its
comment corrected.

**One thing genuinely is absent, and it is a version boundary rather than a defect:** `let x = … return …`
is a FEEL **1.6** construct, and this evaluator implements the DMN 1.5 grammar, which has no `let`
expression. `specs/dmn-feel.md` says so now, so the next reader does not mistake it for a bug — which is
exactly the mistake the stale comment invited.


## [9.0.56] — 2026-10-28 — The TREAT cache had five defects, one of them a wrong answer

`specs/rule-engine.md` listed five things wrong with TREAT's join cache. All five are fixed, and one of
them was a correctness bug rather than an inefficiency:

- **The key named the node, not the fact.** The second fact reaching a cached join node found the
  *first* fact's entry, counted a hit, and `continue`d — so a different fact was silently never
  propagated. It is keyed by `(node, fact)` now.
- **Eviction sorted the keys.** Deterministic, which is why it looked like a policy; it evicted
  whatever sorted first, used a moment ago or never. Entries carry a `last_used` mark and the coldest
  goes. `eviction_takes_the_least_recently_used_entry` proves it by using `aaa` and watching it survive;
  with the alphabetical version that test fails.
- **`stats()` returned hardcoded zeroes**, so a caller could not tell a working cache from a bypassed
  one. The cache counts the hits and misses it observes.
- **The counters existed twice** — struct fields shadowing the metrics collector, with `get_metrics`
  overwriting the collector's answer with theirs. One source of truth now.
- **`TreatEngineFactory::with_cache_size` accepted its argument and dropped it**, so a caller asking for
  a small cache got the default. It carries the size into every engine it builds, and `build()` exists
  so that promise is testable without a downcast the trait does not offer.

**A test I wrote first did not test what it claimed**, and I caught it the way this loop has learned to:
by reintroducing the defect. The cross-engine "second fact" test passed with the node-only key restored,
because TREAT's execution path goes through the shared evaluator and never touches this network. It is
renamed for the property it actually asserts, and the cache is tested where it lives — in the module's
own unit tests — with the spec saying plainly that the network is not on the execution path.


## [9.0.55] — 2026-10-28 — LEAPS aggregations compute

`AggregationNode` tracked the *ids* of the facts feeding it and nothing else, so `recalculate` had
nothing to compute from and returned `None`; `current_value` was therefore permanently `None`, and
`execute_aggregations` computed a value only to discard it. A LEAPS network exists for exactly this
aggregation, and it had never produced a number.

The node keeps the value each contributing fact supplies, recomputes on every add **and** every
removal, and caches the result. A fact whose payload lacks the aggregated field — or carries it as
something that is not a number — contributes nothing and is not made a member, which is also what stops
the network treating every fact as an input to every node. `execute_aggregations` now recomputes each
node from the facts working memory holds at that moment instead of throwing the answer away.

The tests state the arithmetic rather than the plumbing: 10 + 25 = 35 after two facts, 25 after the
first is retracted, `None` when the last one goes; a fact with no `amount` field is not a member; and an
engine that asserts three orders sees its node hold 60.


## [9.0.54] — 2026-10-28 — Backward chaining answers something

`execute_backward` was the last chaining entry point that returned a fixed result: it emitted
`Query{goal}` and `"No rules match the query"`, whatever the engine held. With forward chaining in
9.0.52, the pair the project advertises is now both real.

A goal names a field — `"scorecard.score"`. The search finds the rules whose actions write it, evaluates
their conditions against working memory, and for each unmet condition asks which rule could supply it,
recursing to a depth limit of 8. The trace records `Query`, `RuleApplication`, `SubGoal`, `FactMatch`
and `Backtrack` steps, and `explain()` prints them for a human. `Engine::execute_backward` delegates to
the same search and renders its trace as the path its own result carries, so the two entry points
cannot answer differently.

**The first version had a real bug, and the chain test found it**: conditions were evaluated against
the facts of the *sub-goal's* type rather than the types each rule's conditions name — so a rule that
reads an `order` to produce a `line` looked unsatisfiable one link in, and every multi-step goal
failed with a plausible-looking trace. `engine/tests/forward_chaining.rs` now covers a direct hit, a
goal nothing can produce, a goal that names no field, and a two-rule chain.

**State the limit plainly:** the search matches against the facts that exist and against rules that
could insert what is missing. There is no variable unification or fact binding across rules, so a goal
whose chain needs a fact to be derived and then matched is reported as unexplored rather than solved.
Both spec documents say so instead of implying a logic engine.

Verified locally: 1011 tests, TCK 170/0, clippy and rustfmt clean, `make audit` clean, `make e2e` green,
`make spec-citations` 1129/0, `make singleton-tests` clean, `make deploy-env` 0 dead.
CI/Deploy not confirmed: account billing.

## [9.0.53] — 2026-10-27 — The session surface was a pointer to nothing

`Session` kept the engine as a raw `*const Engine` and never dereferenced it. Everything that depended
on that pointer therefore did nothing: `insert_fact` discarded its argument, and `execute` answered
"Not implemented". A caller had a session object that could not hold a fact or run a rule, and nothing
said so.

It owns a clone of the engine now. `insert_fact` puts the fact in that engine's working memory,
`execute` runs the same forward-chaining loop the engine does and reports what fired, and
`Session::engine()` exposes the state afterwards so a caller can read the results. `destroy()` is
honoured: a destroyed session refuses both.

`engine/tests/forward_chaining.rs` covers it: a session accepts a fact, runs the rule over it, sees the
action's effect in its own memory — and the engine it was created from stays untouched, which is what
owning a clone means. A second test pins that a destroyed session errors rather than silently
succeeding.

`specs/domain-model.md` now says precisely which halves of the `Environment`/`Session` surface are real
and that backward chaining is the one still fixed, instead of lumping five placeholders together.
`AGENTS.md`'s roadmap was rewritten for the same reason: it listed work that has since been done.
It also now records the procedure agreed with the maintainer while CI cannot run: keep working on local
gates, mark each release `CI/Deploy not confirmed: account billing`, and run `gh run rerun` over every
unverified commit once billing returns.

Verified, locally only: 1007 tests, TCK 170/0, clippy and rustfmt clean, `make audit` clean, `make e2e`
green, `make spec-citations` 1131/0, `make singleton-tests` clean, `make deploy-env` 0 dead.
CI/Deploy not confirmed: account billing.


## [9.0.52] — 2026-10-27 — Forward chaining was answering "Not implemented"

Two placeholders sat at the front door of the engine. `Engine::load_rules` returned `Ok(())` without
parsing anything, and `Engine::execute_forward` answered `"Forward chaining not yet implemented"` — so
the pair could not fire a rule however they were used, while `lib.rs` re-exported both as entry points.

Both are real now:

- **`load_rules` parses the same JSON the API stores** — a rule document (`{ "rules": [...] }`, which
  is what a `.rules` file holds), a bare rule, or an array — into the engine's rule set. Invalid text
  is an error naming the problem, not a silent `Ok(())`.
- **`execute_forward` runs the shared fixpoint evaluator** over the engine's rules and working memory,
  then writes the result back, so working memory and the evaluator cannot disagree afterwards. It
  reports what fired rather than a fixed string.
- `forward_chaining::execute_forward` and `execute_forward_simple` both delegate to one `run`, so the
  two entry points cannot answer differently — the drift this code base keeps producing.

`engine/tests/forward_chaining.rs` loads a rule from JSON, fires it, and reads the action's effect back
out of working memory; it also pins that invalid text fails loudly, that an empty rule set fires nothing
and says so, and that the session-taking entry point agrees with the engine's.

**Backward chaining is still a placeholder** and `specs/` says so in two places, now precisely: the
forward halves are closed with the release that closed them, and what remains (`execute_backward`,
`Session::insert_fact`, `Session::execute`, the un-dereferenced `*const Engine`) is listed rather than
implied by silence.


## [9.0.51] — 2026-10-27 — `cargo audit` was never run; the first run found 25 advisories

Dependency vulnerabilities were one of the issue classes this loop is meant to fix, and only `npm audit`
had ever run. `cargo audit` on the Rust tree found **25 advisories in 572 crates**.

**The one that mattered was in the attack path**: `quick-xml` 0.40, which parses untrusted DMN XML,
carried RUSTSEC-2026-0194 and -0195. Bumped to 0.41 and the DMN suite plus all 170 TCK layer results
confirm parsing still behaves. Also fixed: `crossbeam-epoch`, `h2 0.4`, `quinn-proto` (updated within
their majors), `protobuf` 2.x (via `prometheus` 0.13 → 0.14), and `zstd-sys`.

**Fourteen remain, and each is now a recorded decision** rather than an unknown: `rsa` and
`crypto-bigint` are linked but unreachable (the API verifies HS256 only, and `jsonwebtoken` compiles its
RSA support unconditionally); `h2 0.3` and `zstd 0.13` are pinned by every actix-http release up to
3.20; `zip` and `webpki-roots` arrive through test-only dependencies and are absent from the shipped
tree (`cargo tree -e normal -i zip` is empty). `.cargo/audit.toml` lists them with their reasons,
`make audit` fails on anything *not* listed, and CI installs `cargo-audit` and runs it. `SECURITY.md`
carries the assessment.

**A real FEEL bug fell out of the same round.** Probing the `in` operator showed `2 in (1, 2, 3)` — the
list form DMN defines, and the shape a decision-table entry takes — evaluating to **null for every
input**, because the parser treated `(1, 2, 3)` as a parenthesised expression and produced an error
node. A rule written against it silently never fired. The operator now accepts `(1, 2, 3)`, `(< 5, > 10)`
and `not(1, 2)`, with `engine/tests/feel_in_operator.rs` pinning each form.

Getting that right took three attempts, all caught by tests: the first hijacked `x in (1..10)` and lost
the range's exclusive endpoints (`1 in (1..10)` became true); the second used backtracking, which fixed
that but left the arm inside `parse_expr`, inflating its stack frame until the parser's own depth-guard
test **overflowed the stack**; the third decides the shape by lookahead and keeps the work out of line.
FEEL-012 says the parser is bounded and never panics — the guard test is what said otherwise.


## [9.0.50] — 2026-10-27 — Every decision body DMN 1.5 defines is read

Two body forms were still unmodelled, and both are common in models built by an editor:
`<invocation>` with `<binding>` arguments, and `<for>`.

**The `<binding>` form** names its arguments instead of writing the call. It cannot be finished where it
is parsed: `href` points at a model by **id**, and that model may appear later in the document. The
parser records the bindings and joins them once the whole IR exists, producing ordinary FEEL text —
`quadruple(10)` — which is why nothing downstream needed a second representation.

**`<for>`** turned out to be subtler than it looks, and my first attempt was wrong in a way the test
caught. DMN's `<in>` does not hold a list; it holds a `<context>` whose entries **bind the iteration
variables**. Synthesising `for [1, 2, 3] return x * 2` evaluates to `null`, because there is no `x` —
which the test reported as `got {"result": Null}` rather than passing on a key-existence check. The
parser now reads the context entries and emits `for x in [1, 2, 3] return x * 2`, and the test asserts
the doubled list rather than that *something* came back.

`engine/tests/decision_bodies.rs` now covers four documents end to end: a BKM body written as
`<functionDefinition>`, a decision written as a literal-expression invocation, one written with
bindings, and one written as a `for`. `specs/dmn-feel.md` states exactly what a body may be, and the
one remaining limit — `<for>` needs its variables in `<in>`'s context, so a bare `<in>` expression
binds nothing and is left unread.


## [9.0.49] — 2026-10-27 — SYS-003 was claiming more than `clear()` gives

`SYS-003` read: *"Process-wide state must be resettable so that tests observing it are independent of
test order and of parallel execution."* The first half is true and verified. The second does not follow:
two tests that each call `clear()` and then record **will** see each other's records, whatever the reset
does — which is precisely what happened to the eleven coverage unit tests in 9.0.47, after months of
passing on scheduling luck.

So the requirement now says what is actually true — *resettable, and every observant test serialises* —
and the second half has a gate, which it never did. `scripts/check-singleton-tests.py` walks every test
in the workspace, finds the ones that touch a process-wide tracker, and fails unless the test carries
`#[serial]`. `make singleton-tests` runs it, `make ci` includes it, and CI runs it beside the citation
and README checks.

**It found real offenders immediately**: two tests in `api/tests/metrics_integration.rs` read the
trackers with no serialisation at all, while every other file that touches them serialises. Both are
`#[serial]` now. The gate's own detector had a blind spot first — it only looked for attributes below
the `#[test]` line, so eight correctly-serialised tests were reported as offenders; it reads upwards to
the blank line now. Proved by removing one `#[serial]` and watching it fail, then restoring it.


## [9.0.48] — 2026-10-27 — The tenant registry can be persisted

Tenants are the organisations that own rules; the registry is what makes `X-Tenant-ID` mean anything.
It was a `MemoryTenantStore` behind a `Mutex`, so a restart forgot every tenant and orphaned the rules
scoped to them — the same shape of hole the audit trail had, in the store beside it.

`TENANT_PERSISTENCE=postgres` keeps the registry in the table `migrations/20_01_tenant_schema.sql`
already described, answering exactly what the in-memory store answers: names trimmed, blanks rejected
before the database sees them, duplicates rejected by the unique constraint, newest first, `true` from
the first delete and `false` from the next. The two are interchangeable behind `TenantBackend`, and the
tests hold them to the same expectations rather than to two sets.

**A lie avoided on the way.** The first draft mapped a database failure onto
`TenantStoreError::DuplicateName`, because the enum had no other vocabulary — so an unreachable database
would have answered **409 Conflict, "the name already exists"**. `TenantStoreError` gained a `Backend`
variant, the store reports it, and the endpoint answers **503**: the caller's request was fine; the
storage is not.

Verified against a real PostgreSQL: the semantics test drives the Postgres registry through the same
case list as the memory one, and an HTTP test creates a tenant through `/v1/tenants` and finds it again
from a second connection the writer never shared. `make db-test` now runs four suites (5 + 2 + 2 + 2).


## [9.0.47] — 2026-10-27 — The audit trail can be persisted, and a 467-line orphan is gone

The audit trail was a `HashMap` behind a `Mutex`: it was also **nothing after a restart**, which is a
poor property for the record an auditor reads. `AUDIT_PERSISTENCE=postgres` now writes it to a table
(`migrations/21_02_audit_events.sql`), append-only, with the reader's three filters pushed into SQL
instead of loading the trail to answer a question about it. The in-memory default is unchanged, and an
unusable choice — `postgres` without the feature or without `DATABASE_URL` — fails startup rather than
quietly leaving the trail in memory.

Recording became `async` (a database write is), which rippled through six call sites and, in doing so,
made clippy find a **real hazard**: four handlers held a `std::sync::MutexGuard` across the await. Each
now finishes its mutation under the lock, ends the lock's scope, and only then writes the trail.

**Deleted: `engine::governance::audit`.** Four hundred and sixty-seven lines — an `AuditEvent` type, a
trait, a `MemoryAuditStore` and their tests — used by nothing. The API's `v1::audit` is the trail that
actually records; the engine module was the third audit concept in a repository with one.

**A latent flake, exposed by the change and fixed.** Eleven unit tests in `monitoring/coverage.rs`
share one process-wide tracker and ran in parallel, so each one's `clear()` raced its neighbours. They
had been passing on scheduling luck; deleting tests elsewhere shifted the timing and two failed. They
are `#[serial]` now — as `engine/tests/coverage.rs` already did — and the full suite was run twice to
confirm stability rather than once to confirm hope.

Verified against a real PostgreSQL: a governance mutation is recorded, read back through `/v1/audit`,
and found again through a **second connection after the writer is dropped** — which is what surviving a
restart means. `make db-test` runs all three suites: 5 store tests, 2 DMN HTTP tests, 2 audit tests.


## [9.0.46] — 2026-10-27 — The image build failed, and the gate that should have caught it

Deploy went red the moment the image tried to build `--features postgres`:

```
error: couldn't read `engine_state/src/../../migrations/21_01_dmn_documents.sql`: No such file or directory
```

The store embeds its schema with `include_str!`, and the Dockerfile's COPY set never included
`migrations/`. `make image-check` exists to catch exactly this — and missed it, because it carried its
own copy of the build command **without** `--features postgres` and its own hand-written list of
directories to copy. A gate that restates what it is checking can only prove its own restatement.

It now derives both from the Dockerfile: the `RUN cargo build` line it finds is the one it runs (features
and all), and it copies whatever the COPY set names. Proved by deleting `COPY migrations` and watching
it fail with the same message the image build produced, then restoring it.


## [9.0.45] — 2026-10-27 — Postgres is selectable: `DMN_PERSISTENCE=postgres`

The store existed and proved itself; nothing used it. Now the `/v1/decisions` endpoints can persist
through PostgreSQL: build with `--features postgres` and set `DMN_PERSISTENCE=postgres` with a
`DATABASE_URL`, and import, list, get, export and delete all run against the database. The default is
unchanged — the filesystem store, which is what git tracks — and the choice is explicit: asking for
`postgres` without the feature or without the URL **fails startup** rather than quietly writing files,
because a deployment believing its documents are in a database that never received them is the same lie
as a store whose methods return `NotConnected`.

The wiring is one field. `ApiState` carries a `DmnStore` (filesystem or postgres); `from_paths`
assembles it with the filesystem one, and `with_dmn_store` substitutes exactly one field, so the
database path cannot drift into being a second assembly of the server. That mattered immediately: three
hand-rolled test apps still passed `web::Data<FileSystemDmnStore>` and started answering **500** —
compiling happily, failing at runtime, exactly the hazard `AGENTS.md` warns about. They hand the
handler the type it extracts now.

**A backend difference the tests found.** `list()` reads `dmn_documents` while the import path writes
through `save_file`, which went to `dmn_files` — so an imported decision was invisible to the list.
On disk the two views coincide by construction (a `.dmn` file *is* the document), and the store now
enforces the same rule in SQL: a `.dmn` filename names a document, whichever method writes it.

**Verified over HTTP against a real PostgreSQL**: `api/tests/dmn_postgres.rs` imports a model, lists
it, reads its metadata length, exports the XML and deletes it, through the whole application.
`make db-test` runs it beside the store's own five tests.

**The option works in the shipped artifacts**, or it would be a promise only a from-source build could
keep: the Dockerfile builds `--features postgres`, the chart exposes `config.DMN_PERSISTENCE` and mounts
`DATABASE_URL` from a Secret you supply, and `make deploy-env` — which scans the crates the binary is
built from rather than `api/src` alone — confirms every variable the chart sets is one the server reads.


## [9.0.44] — 2026-10-27 — The format gate caught what I committed

`9.0.43` went to CI unformatted: `cargo fmt --all` ran before the clippy fix that introduced two row
type aliases in `engine_state/src/postgres.rs`, and the commit was made after it. The gates job failed
on `cargo fmt --all -- --check` — which is the gate working, on the one thing this round did not
re-check after its last edit.

The rule it illustrates is in `AGENTS.md` already ("run the gate, paste the evidence"), and the sharper
version is: re-run the *cheap* gates after the *last* edit, not before it.

The next run failed differently — **"No space left on device"** compiling `chrono-tz` in the
user-guide snippet step. The checker built its throwaway crate into a private target directory, so it
rebuilt every dependency the workspace had already built, and this release's larger dependency tree
made that duplication fatal on a runner. It now builds into the workspace's own `target/`, and the
gates job reclaims the multi-gigabyte toolchains a hosted runner ships and this project never uses.


## [9.0.43] — 2026-10-26 — PostgreSQL: two fake stores deleted, one real store proven

The "PostgreSQL issue" turned out to be three separate things, none of them a small fix:

| Piece | What it was |
|---|---|
| `engine::governance::tenant::SqlxTenantStore` | held an `Option<PgPool>`, answered `NotConnected` to **every** call, and `new_from_env()` read `DATABASE_URL` only to return an error |
| `engine::governance::audit::SqlxAuditStore` | the same shape, 700 lines apart |
| `engine_state::PostgresDmnStore` | seven `todo!()` methods, never constructed, no callers |
| `migrations/*.sql` | schema nothing applied — no `sqlx::migrate!`, no script, no target |
| `engine`'s dependency on `sqlx` | a Postgres driver compiled into every build, used only by the two fake stores |

**The two fake stores are gone**, and with them `sqlx` from the `engine` crate: its dependency tree
drops from **247 unique crates to 206**. Deleting it exposed a second defect — `serde`'s `rc` feature
was being switched on by `sqlx` through feature unification, so `Arc<FeelExpression>` only serialised
by accident. `engine` now declares `serde = { features = ["derive", "rc"] }` itself.

**`PostgresDmnStore` is now real.** `engine_state/src/postgres.rs` implements all seven
`DmnPersistence` methods with actual SQL: byte-exact document round-trips, a conflict (not an
overwrite) when a document id already exists, `NotFound` for updates and deletes that hit nothing,
most-recent-first listing, and file blobs stored and reloaded. The schema
(`migrations/21_01_dmn_documents.sql`) is applied idempotently on connect, so the store owns its DDL.
It is behind the `postgres` feature: no database driver is compiled unless a deployment asks for one.

**It is proven against a real PostgreSQL**, which this environment had never had: `pg-embed`
downloads PostgreSQL 17 and runs it in userspace — no Docker, no root, no preinstalled server — and
`engine_state/tests/postgres_store.rs` exercises the store against it. Five tests, all passing:

```
a_document_round_trips_through_the_database ... ok
creating_an_id_that_exists_is_a_conflict_not_an_overwrite ... ok
files_are_saved_and_loaded_as_bytes ... ok
listing_is_most_recent_first ... ok
updates_replace_content_and_deletes_are_final ... ok
```

`make db-test` runs them, and CI has a `PostgreSQL store` job that caches the downloaded binaries. They
are `#[ignore]` by default so an ordinary `cargo test` does not pay 18 seconds and 30 MB for them.

The store is **not wired into the API** — the filesystem store is still authoritative — and
`README.md`, both `AGENTS.md` copies and `specs/http-api.md` now say exactly that, with the struck gap
naming the release that closed it.


## [9.0.42] — 2026-10-26 — A model can invoke a model, and DMN 1.5 bodies load

Two decision bodies were being dropped in silence. The parser's dispatch matched
`literalExpression`, `decisionTable`, `context`, `list` and `relation`, and sent everything else —
including `<invocation>` and the BKM body element — to `skip_element`. A decision that expressed
itself as a call therefore arrived at the evaluator with **no expression at all**, and a business
knowledge model written to the current standard lost its body.

- **`<functionDefinition>` is read as a BKM body.** DMN 1.5 renamed `<encapsulatedLogic>` to
  `<functionDefinition>`; the child structure is identical, and the parser knew only the older name.
  The end tag needed teaching too — matching only `</encapsulatedLogic>` consumed the rest of the
  document and failed with `unexpected EOF while parsing <businessKnowledgeModel>`.
- **`<invocation>` is read as a decision body.** The literal-expression form (the common one) becomes
  the decision's expression, and the existing name resolution calls the model. An invocation whose
  arguments come from `<binding>` elements is still unmodelled, and the spec says so rather than
  pretending otherwise.

`engine/tests/decision_bodies.rs` evaluates both: a BKM written with `<functionDefinition>` and a
decision written as `<invocation href="#model">` each produce the expected number. `specs/dmn-feel.md`'s
body-coverage row names what is read and what is not.


## [9.0.41] — 2026-10-25 — The loop's lessons, and a chart gate that runs the way CI does

Two things that belong in the repository rather than in a session log.

**`AGENTS.md` records what the 25-round loop taught**, in both copies. It is written to be read by the
next agent rather than admired: the shapes a false claim takes (prose, machine contract, configuration,
code nothing compiles, a test that asserts nothing, a command nobody runs), the two habits that caught
them (verify in the mode the gate runs in; test the checker against the defect it was written for), the
law that two implementations of one thing always drift, the delete/wire/document ordering, and the
sandbox facts that cost time here.

The same file's opening description still said storage was "hybrid — PostgreSQL for runtime metadata",
which the previous release had established is false. It now says what is true.

**`make chart-lint`** mirrors CI's `chart` job exactly — lint, render the default values, render each
overlay — while reporting the Helm version it found, because a local Helm that lints differently from
CI is otherwise invisible. It is deliberately **not** part of `make ci`: that job has no Helm, and a
gate that cannot run where it runs is worse than none.


## [9.0.40] — 2026-10-25 — The published numbers match the tree

The README claimed **1005 Rust tests across 38 targets**; the tree runs **996 across 40**, 6 ignored.
It also listed six gates when eleven run. Both are what this loop started by complaining about — a
number nobody re-measured — so the final release measures again and writes down what it found.

`README.md` also names the gates added since it was last written: `docs-snippets`, `readme` and
`deploy-env`, and says that all of them are part of `make ci`.


## [9.0.39] — 2026-10-25 — The curl examples are executed too

The documentation's commands were the last claims nothing ran. `docs/user-guide.md` shows five `curl`
invocations — import a model, evaluate it, submit a rule, list pending approvals, approve one — and each
names a path. A command that 404s fails in the reader's terminal, which is worse than a missing example.

`api/tests/documented_routes.rs` now scans every `bash` block in the README and `docs/` for lines that
begin a `curl` invocation, takes the method from `-X` (GET otherwise) and the path after `$HOST`,
substitutes its `$VARIABLES`, and asks the real application whether the route exists. It is proved by
changing one path to `/v1/approvals/waiting`, watching it fail, and putting it back.

The check needed two corrections before it was trustworthy, and both were its own bugs: it first sent
`POST` to a `GET` route, and actix's method-mismatch 404 fell through to the single-page shell, which
reported a perfectly good endpoint as missing; and one copy of the parser kept the old code after the
first fix. Reintroducing a defect to watch the check catch it is now the routine, because checks that
have never failed have never been tested.


## [9.0.38] — 2026-10-25 — The quick start runs, and says what storage actually is

`make run` was **broken**: it ran `cargo run --bin api`, and the crate builds a binary called
`rRules-server`. A reader's first command failed with `no bin target named \`api\``, and nothing
noticed, because nothing has ever run the README.

The quick start also **misdescribed the storage**. It said "Git for rule versioning, PostgreSQL for
runtime metadata (`migrations/`)" — but no crate depends on a database driver, `PostgresDmnStore` is a
`todo!()`, and every runtime store is in memory, so a restart starts empty. The README now says that
plainly, names what *is* on disk (rule and decision documents, and a git repository for versioning),
and points at the known-gaps table rather than implying a database that is not wired. The missing first
step — `npm --prefix frontend install` before `make frontend-dev` — is there too.

**The commands are checked now.** `scripts/check-readme.py` reads every `bash` block in the README and
resolves what it names: every `make <target>` against the Makefile, every `cargo run --bin <name>`
against the binaries the crates declare, every `-p <crate>` against the workspace, every
`npm --prefix <dir>` against a `package.json`. `make readme` runs it, `make ci` includes it, and CI runs
it in the documentation job beside the citation and snippet checks.

It catches the exact bug it was written for — reintroducing `cargo run --bin api` reports
`no crate declares that binary (declared: … rRules-server)` — because the first version of the script
had a bug that made it accept anything, and testing the checker against the defect was the only way to
find that.


## [9.0.37] — 2026-10-25 — The guide's examples now run, and two of its claims were wrong

The documentation check compiled the guide's Rust and never ran it. Compiling is half a claim: a block
that contains `#[test]` states an expected outcome, and `cargo build` does not even type-check the body
of a `#[test]` function. The checker now runs `cargo test` in the crate it generates, so an example that
asserts something has to be right.

**The first run found the guide's testing example using an API that does not work.** It built an engine
with `EngineBuilder::new().build()` and called `execute_forward()`, which the specification records as
"not yet implemented", and it referred to `EngineBuilder` and `Fact` without importing them — invisible
until the block was actually compiled as a test. The section is rewritten against the API that runs:
build a `RuleSet`, take an engine from the registry, load, insert a fact, execute, and assert on
`fired_rule_ids`. It compiles **and passes**, and the guidance it gives is the kind a reader can follow.

**Best Practice 2 was false advice.** It recommended `no_loop` "to prevent rules from firing repeatedly
on their own results" — but no execution path reads `no_loop`: the evaluator reads `salience`,
`agenda_group` and `lock_on_active`, never the flag. The property it promises is already unconditional,
because a rule fires at most once per `execute()` call, so setting it changes nothing. The guide says
that now, recommends `lock_on_active` for sequencing (which *is* honoured), and `specs/rule-engine.md`
records the flag as inert.


## [9.0.36] — 2026-10-25 — The guide covers DMN and governance

`docs/user-guide.md` was titled "Writing Rules" and stopped there. A reader of the guide alone could
not use the DMN side or the approval workflow at all; they would have had to find `docs/API.md` and
work out which of its endpoints mattered.

Two sections now cover them:

- **DMN decisions** — a complete Rust example that parses a decision table, evaluates it by id and
  reads its outputs, plus what hit policy and the dependency graph do to the answer, and the three HTTP
  calls that import, list and evaluate a model. The Rust block is compiled by `make docs-snippets`
  like the other fifteen, so it cannot drift from the API it documents. It failed on the first attempt
  — the example reached for `rust_decimal` directly — which is the check doing its job.
- **Governance** — the maker-checker flow as it actually is: submit, see what is pending, approve or
  reject, and where the trail appears. It also states that `DELETE /v1/rules/{id}` **archives** rather
  than erases, and why.

The table of contents listed six of the eleven sections; it lists them all now, and the documentation
check verifies that every `#anchor` in the file resolves to a heading — a renamed section leaves a link
that scrolls nowhere, and nothing was checking.

**The browser gate stopped guessing its port.** CI failed the gates job with "Chromium did not expose
DevTools on port 9222" — a fixed port, the same hazard as the fixed server port that made every route
render blank in 9.0.19. The harness now asks the operating system for a free port, waits for DevTools
on that one, and reports the browser's own last words and exit status when it fails, instead of a
message that says only what it was waiting for.


## [9.0.35] — 2026-10-25 — The maker-checker loop, walked in a browser; and a correction

The governance pages turned out to be wired — the approval queue posts to
`/v1/approvals/{id}/approve`, the branch manager creates and merges, the audit viewer filters through
its endpoint. What had never been walked end to end was the **workflow**: a rule is submitted for
approval, appears in the queue, and is approved there. The browser gate now does exactly that:

```
ok  a rule can be submitted for approval        status 201
ok  the queue's Approve discharges the request  0 still pending
ok  the approval is recorded in the audit trail approve event present
```

That third line joins the two halves of recent work: the button that discharges the request is the
same one that writes the audit event.

**A correction.** `9.0.7`'s changelog said four stale "mount-only" TODOs were removed. Three were not:
the edit *prepended* an accurate comment and left the original TODO underneath it, and nothing checked.
They are gone now, and `frontend/src` contains no `TODO` markers at all.


## [9.0.34] — 2026-10-25 — The rules list's buttons do something

Two of the index page's controls were decoration. **"Create Rule" had no `onClick`** — it rendered,
it had the primary-button styling, and pressing it did nothing. The rows were not clickable either,
and there was no way to remove a rule from the interface at all, though the API has supported it since
the beginning.

The list now works: **Create Rule** opens the form, a rule's name is a link to its editor, and each
row has an **Archive** action that calls the API and asks the hook to reload.

That last one is where the round's real lesson was. The action was first labelled *Delete* and the
browser gate asserted the rule disappeared — and it did not, because `DELETE /v1/rules/{id}` **archives**
the rule rather than removing it, which is what keeps the audit trail meaningful. The interface was
lying about what its own button did. It says **Archive** now, archived rules get a status badge, and
the gate asserts the status actually becomes `archived`.

The browser gate covers both actions: the create button must arrive at the form, and archiving must
change the rule's status as the API reports it.


## [9.0.33] — 2026-10-25 — One assembly, so a test cannot run a different server

`main.rs` built the application state across ~150 lines and `ApiState::for_testing` built it again in
~40. They had already drifted once — the test copy registered no engines, so every in-process API test
touching `/v1/execute` or `/v1/engines` exercised an engine-less server — and two constructors of the
same struct will always drift again.

There is one now. `ApiPaths` names the four things that differ between a deployment and a test (rules
directory, DMN directory, frontend build, allowed origins), `from_paths` assembles the state from them,
`from_environment` reads the paths from the environment, and `for_testing` is a wrapper that chooses a
temporary directory and nothing else. `main.rs` is down from 254 lines to 54: read the bind address,
assemble, start the watcher and the session sweeper, serve.

The assembly also gained what only `main` had: `for_testing` now loads a rule set from its `rules/`
directory and DMN models from its `dmn/` directory, so an in-process test can exercise a real model —
`assembly_tests` writes a `.dmn` file and asserts the evaluator holds it. `DecisionEvaluator::shared()`
is the accessor that made that assertion possible without reaching into its internals.

Sixteen spec citations pointed into the moved code; they now point at `app.rs`.


## [9.0.32] — 2026-10-25 — One functional suite for all four engines

`ENG-002` says the four Rete-family implementations "must pass the same functional tests regardless
of algorithm". That was true by inspection only: each engine had its own keystone test with a
**copied** fixture, so nothing failed when one engine drifted, and behaviour only one engine
implemented was never compared.

`engine/tests/cross_engine_suite.rs` takes the names from `EngineRegistry::available_engines()` and
runs one scenario through every engine it reports: firing and applying an action, retracting a fact
and reporting the second attempt as a no-op, resetting to an empty state, being constructed fresh per
execution, exposing exactly the documented names, and rejecting an unknown one. A newly compiled
engine joins the suite without anyone remembering to add it.

**It found a defect immediately — in itself.** `EngineRegistry::new()` is empty (engines are
registered explicitly), so the first run looped over nothing: five assertions "passed" while testing
no engine at all. The suite now registers the defaults and panics if the list is empty, which is the
guard that stops a loop over nothing from looking like a pass.

That discovery pointed at the same mistake in `ApiState::for_testing`: it built a registry without
registering the engines, so every in-process API test that touched `/v1/execute` or `/v1/engines` was
exercising an engine-less server. It registers them now.

**A stale gap is corrected.** `specs/rule-engine.md` claimed PHREAK's retraction "is a stub returning
`Ok(false)` for every id". The trait's `retract_fact` does real work — working memory, alpha, beta and
the truth-maintenance edges — which the new suite now asserts for all four engines. The placeholder
the gap described was a *different* method, `retract_fact_with_maintenance`, which returned
`Ok(false)` unconditionally and had no callers; it is deleted.


## [9.0.31] — 2026-10-25 — The OpenAPI document describes the API it serves

`/api-docs/openapi.json` is what a client generator reads, and it is assembled from a hand-written
list in `api/src/openapi.rs` — a list that falls behind the router exactly like prose does. A new test
compares it against `docs/API.md`, which the existing test already proves matches the router, and it
found **21 documented routes missing from the document**, including:

- all three authentication endpoints — a generated client could not log in;
- all ten decision endpoints, so evaluation and the DMN lifecycle were invisible to it;
- both session endpoints, the DMN Prometheus endpoint, and the `/live` and `/ready` aliases.

The ten decision handlers had no `#[utoipa::path]` annotations at all, and neither did the session
handlers or the DMN metrics endpoint; they are annotated now, their request and response types carry
`ToSchema`, and everything is registered. The document grew from describing roughly half the API to
describing all of it.

Five routes remain deliberately outside it, and the test says why rather than quietly skipping them:
`/live` and `/ready` are aliases of `/health/live` and `/health/ready` (one operation, one path),
`PATCH /v1/rules/{id}` aliases `PUT`, and the two WebSockets are upgrades that OpenAPI 3.0's operation
model cannot describe. A second test asserts each exemption still names a route the guide documents,
so an exemption cannot outlive its reason.


## [9.0.30] — 2026-10-25 — A test suite that never compiled, and a copied module nobody ran

Two more pieces of code that existed without ever being executed, both already recorded in
`specs/rule-engine.md` as gaps.

**`engine/src/tests/mod.rs` is deleted.** 992 lines, 70 tests, declared by no module — so it had
never compiled, let alone run. Wired into `lib.rs` to see what it would take, it produced 45 errors
against an API that no longer exists (`evaluate`, `Agenda::with_strategy`,
`EngineBuilder::conflict_strategy`). It contributed no coverage, because a test that never builds
never runs, and its subject matter is covered by the 38 in-module tests in `engine/src/rules.rs`.

**`engine/src/coverage/history/mod.rs` is deleted** — a copy of `coverage/mod.rs` identical apart
from the line that declares it, behind a `history` feature no build enabled and nothing referenced.
The feature, the declaration and the copy are gone.

**The duplicated `ConflictStrategy`/`ConflictResolver` are collapsed.** `salience/agenda.rs` defined
its own copies of both, so the crate carried two different types with the same names — one exported,
one not — and `agenda.rs`'s tests used its private pair. They now use the canonical definitions from
`salience/conflict_resolution.rs`, which is what `salience/mod.rs` and the crate root export.

`AGENTS.md` (both copies) gains the rule that cost two rounds to learn: a handler that starts
extracting `web::Data<T>` breaks every hand-rolled test app at **runtime** — they answer 500 instead
of failing to build — so adding a dependency means grepping `api/tests/*.rs` in the same commit.


## [9.0.29] — 2026-10-25 — The DMN metrics socket streams

`/v1/metrics/dmn/ws` forwarded what the evaluate path published, and the evaluate path published
nothing: `DmnMetricsTx` had no senders anywhere in the crate, so the socket was a well-built pipe
connected to nothing. A background task did drain the receiver, but only to log — it competed with
no subscriber and delivered to none.

`evaluate_decision` now publishes a `DmnMetricsSnapshot` (decision id, rules fired, duration,
timestamp) on every successful evaluation, and each WebSocket client subscribes to the channel
itself. The drain-and-log task is deleted.

A failed evaluation is counted in the Prometheus metrics but publishes no snapshot, because the
snapshot describes a completed evaluation — `docs/API.md` now says so rather than leaving a reader to
guess.

The test that proves it lives in `decisions_integration.rs`, beside a real DMN model, because a
broadcast channel only delivers to receivers that existed when the message was sent: it subscribes
first, evaluates `d1`, and asserts the snapshot arrives. Two hand-rolled harnesses there needed the
sender added — the same runtime-500-not-compile-error failure as `tenants.rs` last round, which is now
a `test_channel()` helper so the next one is a one-liner.


## [9.0.28] — 2026-10-25 — `WATCH_RULES_DIR` watches

The option promised hot reload and its consumer was one `log::info!`: a deployment could set
`WATCH_RULES_DIR=true` and edit rules on disk all day without the server noticing.

`start_file_watcher` now snapshots the rules directory — each rule file with its modification time
and length — every `RRULES_WATCH_INTERVAL_SECS` (default 2 s) and calls `reload_and_swap` when the
snapshot differs, which swaps the whole set atomically so a concurrent execution sees either the old
rules or the new ones. Polling rather than a filesystem-notification crate keeps the dependency list
unchanged, and mtime-and-size catches additions, removals, edits and truncations, which is what a
rules directory does. A change that fails to parse is reported and the previous rule set stays in
service.

`main` now hands the watcher the same `Arc<RwLock<SharedRuleSet>>` the execute path reads, so the two
cannot drift apart.

Five tests: a snapshot lists rule files and ignores others, adding a file changes it, resizing one
changes it, a missing directory is an empty snapshot, and an end-to-end test spawns the watcher, writes
a rule file, and polls until the generation moves. That last one failed first time — the test wrote the
file before the watcher's task had taken its baseline, so the file was part of it — which is exactly
the race the test exists to notice once, in a test, rather than in production.


## [9.0.27] — 2026-10-25 — The audit trail records what happens

`record_audit_event` had zero callers, so `/v1/audit` and `/v1/audit/export` were permanently
empty — an "immutable audit trail" that never saw an event. It is wired now, at the five places
where something actually changes:

| Endpoint | Action recorded |
|---|---|
| `POST /v1/rules` | `create` on the rule |
| `PUT /v1/rules/{id}` | `update` on the rule |
| `DELETE /v1/rules/{id}` | `delete` (the handler archives, so the event says so) |
| `POST /v1/approvals/{id}/approve` | `approve` on the approval |
| `POST /v1/approvals/{id}/reject` | `reject` on the approval |

`record_for_request` builds the event from the request: the acting user comes from the `Claims` the
auth middleware already puts in the request extensions, the address from the peer connection, the
agent from the header. Handlers therefore call it with one line and nothing to unpack.

Two things kept this honest. The first insertion landed in `get_rule` — a GET would have recorded an
`update` — which the type checker caught because GET handlers do not carry an audit store. And
`docs/API.md` now says what is *not* audited (category, tag and tenant mutations, executions) instead
of leaving a reader to assume everything is.

Two in-process tests read the trail back through the API a user would call: a created rule appears as
one `create` event with its name and `User-Agent`, and archiving a rule leaves a `delete` event.


## [9.0.26] — 2026-10-25 — Dead code deleted rather than documented, and every DMN file loads

`specs/http-api.md` listed eleven gaps. Four of them were dead code the project had written down
instead of removing, which is the accretion `AGENTS.md` warns about — and one was a real bug.

**Deleted** (gaps closed by removal, not by prose):

- `api/src/error.rs`, `api/src/v1/auth.rs`, `api/src/middleware/correlation.rs` — three files no
  module declared. The compiler never saw them, so nothing kept them honest; the `v1/auth.rs` even
  imported symbols from `middleware::auth` that do not exist there.
- `configure_v1_metrics` with its two handlers. They answered with **hard-coded zeroes** — an
  endpoint that fabricates its numbers is worse than a missing one, so the module went rather than
  being registered.
- `get_audit_event`: with `record_audit_event` never called, a get-by-id could only ever 404.

**Fixed:** startup loaded `read_dir`'s first `.dmn` file — arbitrary among several, and a restart
could pick a different one. `api::dmn_loader::load_models` now reads every `.dmn` file in sorted
order and merges their definitions, so a second decision file is visible and the result is
reproducible. A file that cannot be read or parsed is reported and skipped rather than hiding the
rest. Four tests cover it: both models load, a broken model does not hide a good one, an empty
directory yields an empty model, and merging concatenates.

`specs/http-api.md` strikes the four rows with what replaced them, and `docs/API.md` no longer
describes the placeholder endpoints as if they existed.


## [9.0.25] — 2026-10-25 — The browser gate waits for the data, not just the chrome

CI failed the gates job on `/governance/approvals` while every neighbouring check passed, and the
failure detail said "rendered its data" — because that string was constant and the assertion had
quietly gone false. The cause was the same race as 9.0.19 in a different place: `visit` waits until
the page has *any* text, and the navbar provides that long before a page's fetch resolves, so an
assertion about fetched data was racing the fetch.

`visitMatching` navigates and polls until the page matches the marker it is being asserted on. The
four governance checks use it, so they assert the settled state rather than whatever the first frame
happened to hold. Three consecutive local runs are green.

The failure detail is also no longer a constant: a failed check prints what the page actually
contained.


## [9.0.24] — 2026-10-25 — The deployment configures the server it runs

A manifest is a claim about the program it starts. Both deployment paths were making false ones.

`scripts/check-deploy-env.py` compares what the deployment sets against what `api/src` reads, and
judges each path separately — chart-plus-image and manifests-plus-image. It found:

| Claim | Reality |
|---|---|
| `k8s/configmap.yaml` sets `SERVER_HOST`, `SERVER_PORT` | the server reads `RRULES_BIND_ADDRESS` and `PORT`; neither was set by the manifests, so a container picked its own defaults |
| `APP_ENV`, `RULE_STORAGE_TYPE`, `RELOAD_INTERVAL_MS` | read nowhere in `api/src` — configuration that does nothing and misleads the next debugger |
| the chart sets three variables and no signing key | `API_SECRET_KEY` absent means the server runs on `rrules-dev-secret-change-me`, a **public** string: every token forgeable |
| overlays set `database.url`; a `secret.yaml.template` wired it | no template read it, the server has no database code, and the template was never rendered — it also referenced `.Values.api.secretKey`, which did not exist |

Now:

- the ConfigMap sets the variables the server reads and nothing else;
- the chart passes its whole `config` block through, so a key it declares is a key the container
  gets, and the signing key comes from a Secret the chart creates — `auth.apiSecretKey` if given,
  else the value already in the cluster, else a generated 48-character key. The middle case is what
  stops `helm upgrade` from invalidating every issued token;
- `k8s/` gains a `Secret` the deployment reads via `secretRef`; the unrendered template and the
  unused `database` blocks are deleted.

`make deploy-env` runs the check, is part of `make ci`, and fails on dead configuration or on a
deployment path that leaves `API_SECRET_KEY` unset.


## [9.0.23] — 2026-10-25 — The user guide's examples are compiled

`docs/user-guide.md` shows fifteen Rust snippets — the whole documented way to write a rule. Nobody
had ever compiled them, and they had drifted in every way a snippet can: the import path
`engine::ComparisonOp` did not exist, thirteen blocks used types they never imported, thirty-one
`Condition::Pattern` literals predated the `op` field that the pattern now requires, the
`Action::UpdateFact` example showed `Value` where the API takes an `Expression`, one identifier was
mangled (`let优惠_rule`), and three blocks named facts they never defined.

`scripts/check-doc-snippets.py` extracts every ```rust block, wraps each in a function, and compiles
them in a throwaway crate under `target/` that depends on the engine by path. Compilation is the
assertion; nothing in the repository is generated or committed.

**All fifteen now compile**, and the guide is copy-pasteable — which it was not. Two API gaps fell out
of it: `ComparisonOp`, `Expression` and `LogLevel` were not re-exported from the crate root, though
`Condition` and `Action` were and every rule needs them. They are now, so
`use engine::{Rule, Condition, ComparisonOp, Action, LogLevel};` is enough. `specs/rule-engine.md`
documents the operator-in-pattern and expression model, which it had omitted.

`make docs-snippets` runs the check, is part of `make ci`, and CI runs it in a new `Documentation`
job beside the citation check — both read the documentation and fail when it stops being true.


## [9.0.22] — 2026-10-25 — The API reference is checked against the router

`docs/API.md` is prose, and prose rots the same way `specs/` did: it can promise an endpoint that
was renamed, moved, or never registered. A new integration test (`api/tests/documented_routes.rs`)
reads the document, drives each of its 67 documented endpoints against the real application built
in-process, and fails when the router does not know one. Two signals identify a route that does not
exist: under `/v1` the SPA answers `{"error":"no such endpoint: …"}`, and at the root an unknown path
falls through to the application shell.

It found the reference documenting **two endpoints that are not registered**, at paths that were
not merely stale but contradicted the document's own opening note:

| Documented | Reality |
|---|---|
| `GET /metrics/dead-rules` | `/v1/metrics/dead-rules`, requires a token |
| `GET /metrics/coverage` | `/v1/metrics/coverage`, requires a token |

Both were duplicates of the correctly-documented `/v1/metrics/*` sections further down, so the file
described the same two handlers twice — once at a path that 404s. The duplicates are gone and the
opening note says which of the two is real.

The test runs as part of `make ci` (`cargo test --workspace`), so a documented endpoint that stops
existing now fails the build.

The same pass fixed the reference's environment table, which listed **3** variables while the server
reads **14** — among the missing: `API_SECRET_KEY` (whose default is a known development string that
any deployment must override), `RRULES_BIND_ADDRESS`, `CORS_ALLOWED_ORIGINS`, `DMN_STORAGE_PATH`,
`RRULES_DEFAULT_ENGINE` and the four session limits. `specs/http-api.md` had recorded the gap; it now
records the fix.


## [9.0.21] — 2026-10-25 — Tailwind 4, and dark mode that was never wired

The last three advisories lived in Tailwind's PostCSS chain, and clearing them meant the Tailwind 4
migration — which turns out to be a smaller change than it sounds, because v4 reads its
configuration from CSS. `tailwind.config.ts` and then `postcss.config.js` are gone; `global.css`
declares `@import "tailwindcss"`, the token palette in `@theme`, and Vite loads `@tailwindcss/vite`
instead of the PostCSS chain. `autoprefixer` and `postcss` had no job left and were removed, along
with 58 packages. **`npm audit` now reports 0 vulnerabilities**, from 56 when this loop started.

The migration needed a gate to judge it: the browser gate reads *text*, and an application with no
stylesheet at all still has text. So it now asserts that Tailwind's utilities reach the page — the
create button carries `bg-blue-600`, and a utility that never compiled computes to
`rgba(0, 0, 0, 0)`. That assertion failed once during the migration, correctly: Tailwind 4 emits
`oklch(...)` where Tailwind 3 emitted `rgb(...)`, so the check was narrowed to "a colour resolved".

**Dark mode never worked.** `global.css` declared a `.dark` token block and
`darkMode: ['class']`, my own comment claimed "the `.dark` class is what the app toggles" — and
nothing in the application ever adds that class. Eighteen files carry `dark:` utilities that could
not match, and the token block was dead. Dark mode now follows the operating system: the tokens moved
under `@media (prefers-color-scheme: dark)`, the class-based variant is gone, and the `dark:`
utilities default to the same query, so the two agree. The gate proves it by asking Chromium for the
dark preference and comparing the computed background: `rgb(255, 255, 255)` in light, `rgb(9, 9, 11)`
in dark.


## [9.0.20] — 2026-10-25 — Which gates CI actually runs, stated rather than assumed

`AGENTS.md` and `README.md` both claimed "CI runs exactly these gates". It does not: `make verify`
needs CodeWitness, which is not published — it is a jar installed from a source checkout, so a hosted
runner cannot obtain it, and the GitHub repository does not exist.

Three changes, all about not overstating:

- the workflow gains a `Requirement verification` job that runs `make verify` when the repository
  variable `CODEWITNESS_ENABLED` is `true` and the secret `CODEWITNESS_JAR_B64` holds the jar, and is
  otherwise **skipped**. A skipped job is grey in the check list; it is never a passing one, and the
  documentation says a skipped job means the claim did not run in CI on that push.

  The first version of that job gated on `secrets.CODEWITNESS_JAR_B64` at job level, which GitHub
  rejects — `secrets` is not in the context a job-level `if:` may read — so CI failed in 0 s without
  running a single gate. A repository variable is allowed there; the jar stays a secret, read at step
  level.
- `make verify` checks for the tool first and explains the situation, instead of failing with
  `command not found` from inside a recipe.
- both `AGENTS.md` files and the README now name the gates CI really runs — `make ci`, `make tck`,
  `make spec-citations`, `make image-check` and the chart lint — and say plainly that the
  requirement verification is local-only unless the jar is supplied.


## [9.0.19] — 2026-10-25 — The browser gate waits for content, not for a stopwatch

CI failed the gates job with `/` rendering nothing while all thirteen other routes passed. The cause
was the harness, not the application: `visit` slept a fixed 700 ms and then read the page, and the
first load after the server starts is the slow one — on a runner it can exceed any delay short enough
to keep the gate quick. A route reported as blank while its neighbours pass is a flake that reads
exactly like a broken app, which is the worst way for a gate to fail.

`visit` now polls until the page has text, with a five-second ceiling. Three consecutive local runs
are green.


## [9.0.18] — 2026-10-25 — Every requirement is verified: 35, not 28

The `api` half of the verification said `UNVERIFIED` for all 35 requirements, and the project had
recorded that as a limitation of the approach: "discharging them fully needs generated scenarios
against a live server, which the generated-test harness cannot construct". That was wrong on both
counts, and this release shows it.

- `api/codewitness.yaml` had `property_testing.enabled: false` and `max_generated_tests: 0`, so no
  experiment ever ran; enabling the scenario engine was the first requirement.
- The scenarios then needed something that could build the application. 9.0.17 gave them that:
  `build_app` and `ApiState::for_testing`, now public rather than `#[cfg(test)]`.

Seven scenarios now run the real stack in-process — the auth middleware, the routing and the
handlers, against stores rooted in a temporary directory — and CodeWitness executes them:

| Requirement | Experiment |
|---|---|
| API-001 | an unauthenticated `/v1/rules` is `401`, a token is `200`, `/health` is public |
| API-002 | an unknown engine and a malformed body are both `400` — never `500` |
| API-003 | a rule created for `tenant-a` is invisible to `tenant-b` |
| API-004 | `201`, blank name `400`, duplicate `409`, fetch `200`, delete `204`, second delete `404` |
| API-005 | decisions list `200`, an unknown decision is a client error |
| API-006 | a reload returns a real job id whose status is one of the four defined states |
| API-007 | `/metrics` and `/v1/metrics/coverage` both answer `200` |

**`make verify` now reports the union: 35 verified, 0 failed, 0 unverified.** Each half necessarily
marks the other half's requirements `UNVERIFIED` — `engine` has no HTTP surface, `api` has no
evaluator — so `scripts/verify-summary.py` unions the two reports by id, and a requirement that
neither half executes fails the gate as a real gap.

`specs/README.md`, `README.md` and both `AGENTS.md` files now state the measured union instead of
the old 28/0/7.


## [9.0.17] — 2026-10-25 — The HTTP application can be built without a socket

The crate compiled the same sources twice: `main.rs` declared every module *and* `lib.rs` declared
most of them, so the binary and the library were separate crates with distinct types — the
binary's `metrics::ExecutionCounters` was not the library's. That is how a test can pass against one
and the server run the other, and it meant the unit tests of the shared modules ran twice.

The library owns the modules now and the binary is process wiring only. The application itself
moved to `api/src/app.rs`: `ApiState` holds what the router needs and `build_app` assembles the same
`App` the server serves. `main.rs` reads the environment, starts the session sweeper and binds the
port.

That makes the routes testable in-process, which they never were — the integration suites had to
bind a port and race whatever else was listening. Three tests drive the real stack with
`actix_web::test` in about 10 ms each:

- `/health` answers `200`;
- `/v1/rules` without a token is refused by the auth middleware;
- logging in and listing rules works, and an absent `page_size` means 50 — the bug fixed in 9.0.6,
  now pinned by a test that runs in-process.

**The lint gate covers test and bench code.** `make lint` passes `--all-targets`; without it, unit
tests inside modules, integration tests and benchmarks were never linted and about sixty findings
had accumulated there. Among them, `test_cache_statistics` asserted `cache_hits >= 0 ||
cache_misses >= 0` — always true on unsigned counters, so it asserted nothing (the vacuous
assertion `AGENTS.md` names as a past sin). It now asserts what the engine does: three inserted
facts are counted, and no rule is installed so propagation never consults the cache. A dead fixture
constant in an api test was deleted, 31 lines that nothing referenced.


## [9.0.16] — 2026-10-25 — The browser gate presses "Reload then Run"

The reload → poll → execute chain had only ever run against mocks: a unit test resolves the three
calls, and nothing had driven the button in a browser against a real server. It does now, and it
terminates in a result or a reported error — the assertion is that the chain finishes, because the
failure mode worth catching is a spinner that never resolves.

Adding it exposed a defect in the harness itself: when an evaluated expression threw, the gate
reported the word `"Uncaught"` and nothing else. It now reports the thrown value and the first
frame, which turned "Uncaught" into `SyntaxError: Invalid regular expression: missing /` in one run
— mine, an unescaped `\n` inside a template literal, but the difference between a message that
names the fault and one that hides it.


## [9.0.15] — 2026-10-25 — The README states what the gates measure

The public entry point had drifted the way specifications drift: it opened with `**9.0.0**`
(fourteen releases behind), listed three gates when six now run in CI, and claimed "1,000+ tests"
without a number anyone could check.

It now names the release it documents, lists every gate CI runs — including the browser gate, the
citation check, the Dockerfile check, the memory gate and the fuzzer — and states the measured
counts: **1005 Rust tests across 38 targets, 28 frontend tests, 170 TCK layer results, 28 verified
requirements**.

`make bump-version` updates the README's release line too, so the first thing a visitor reads is
the release they are looking at.


## [9.0.14] — 2026-10-25 — A crate-local `target/` is ignored

`make bench` left a 30 MB `engine/target/` that `git status` reported as untracked: `.gitignore`
excluded `/target/`, which only covers the workspace root, and any tool that builds a member in
place creates one beside its manifest. The pattern is now unanchored.

## [9.0.13] — 2026-10-25 — Citations must stay inside the repository

CI failed the new *Spec citations* job on its first run, and the failure was a real defect rather
than a checker bug: `dmn-feel.md` cited `../.planning/phases/39/…/TCK-COVERAGE-GAPS.md:10-30`. The
workspace-level `.planning/` tree is deliberately not published with this repository, so that
reference was unopenable for every reader of it — it resolved on the machine that wrote it and
nowhere else. The claim now cites `TCK-COVERAGE-GAPS.md` and the sweep code, both of which ship.

The checker treats any citation that leaves the repository as unresolved, with that reason in the
message, so the class cannot come back.

The same pass corrected two claims that resolved but misled: `http-api.md` said host and port were
"not configurable by env var" (they come from `RRULES_BIND_ADDRESS` and `PORT`), and it cited
`main.rs:313` for the `/v1` scope, a line that is now a comment — the scope is at 334.

A semantic check was written and then removed: comparing the symbol a sentence names with the text
at the cited lines flagged 646 of 1162 citations, nearly all of them ordinary loose wording. A
checker that cries wolf that often is worse than none.

## [9.0.12] — 2026-10-25 — The specification's citations are checked, and two had drifted

`specs/` cites code 1162 times (`path:line`), which is what makes it evidence rather than
description. Nothing verified those citations, and the parser work in 9.0.10 moved line numbers.

`scripts/check-spec-citations.py` resolves every citation: the file must exist (by path, by
unique basename, or by the full paths the same document uses elsewhere) and the cited range must
fall inside it. Two were wrong — `rule-engine.md` pointed at `rete/mod.rs:750-768` in a 739-line
file, and `dmn-feel.md` at `TCK-COVERAGE-GAPS.md:118-120` in a 74-line file. Both now cite the
lines that hold the claim: the idempotent re-retract assertion, and the accepted-divergence
section.

`make spec-citations` runs it strictly and is part of `make ci`; CI runs it as its own job. A
citation that no longer resolves now fails the build instead of quietly looking like evidence.


## [9.0.11] — 2026-10-25 — The verification gate builds again

`make verify` reported every requirement **BLOCKED**: CodeWitness compiles the crate itself and
passes `--offline` to cargo, so its resolution depends on a crates.io index snapshot — and the
`rustls` bump that came in with a Dependabot PR left `Cargo.lock` pinning a version newer than that
snapshot. "BLOCKED" is honest in the tool's terms but it told us nothing about the code.

`make verify` now runs with `CARGO_NET_OFFLINE=false` so the index can refresh, and both
`codewitness.yaml` files allow the verification build the network it needs to resolve. The gate is
green again: **engine 28 VERIFIED, 0 FAILED, 7 UNVERIFIED**, api 35 UNVERIFIED (the API
requirements are exercised by the integration suites, not by CodeWitness), and no requirement is
blocked.


## [9.0.10] — 2026-10-25 — A fuzzer finds a memory-exhaustion bug in the FEEL parser

`make fuzz` had never been run against this tree. It found, within a minute, that
`parse_expression` could be driven out of memory: `= value` builds a singleton range by parsing a
bound, and the bound parser re-enters the same rule, so **every additional `=` cloned the subtree
into both `start` and `end`**. Twenty equals signs produced a 2^20-node tree (≈170 MB of debug
output) and ran for 170 ms; twenty-four exhausted multi-gigabyte memory, and libFuzzer aborted at
its 2 GB limit. Any endpoint that parses a FEEL expression — decision tables, rule bodies,
`POST /v1/execute` — could be taken down by a 24-character string.

A range bound can no longer itself be a singleton range. `= 5` still parses as a point range;
`= = 5` is an error. `=`×10 000 now parses in 0.8 ms with a 5 MB footprint, and `singleton_range_
does_not_nest` plus the fuzz artifact itself are regression tests. `specs/dmn-feel.md` lists the
guard with the others.

The fuzz gate itself was broken in two ways and is fixed: libFuzzer's `-max_total_time=60` let a
60-second run spin at 99% CPU for 29 minutes, so `timeout` now holds the clock and a budget that
is reached is a pass rather than a failure. Both targets then ran to their budget with no crash,
no timeout and no leak.


## [9.0.9] — 2026-10-25 — The memory gate measures something again

`make memcheck` failed: peak RSS **+35%** against a baseline taken at v6.0. The growth is real
and accounted for — since then the evaluator gained an invocable index, bound function scopes for
business knowledge models, boxed contexts, lists and relations, and a cache holding every parsed
function body. A baseline that no release can pass is not a gate; it is a red light everyone
learns to ignore.

The snapshots are now named for what they measure: `baseline.json` and `current.json`, no
version in the name. The baseline is the 9.0 measurement, recorded by `make baseline-update`, and
`engine/benches/README.md` says why the v6.0 floor was retired. Allocations moved 5%
(1877 → 1986) and RSS 35%; from here the gate protects those numbers.

## [9.0.8] — 2026-10-25 — "Test this rule" says what it can and cannot run

The edit page mounts a test runner whose Run button posts to `POST /v1/execute`. That endpoint
runs the **engine's rule set**, loaded from the server's rules directory; a rule created through
`/v1/rules` is stored as text and is not in it. The runner therefore always reported "0 fired"
for an interface-authored rule, with nothing to say why — and a run where nothing fired looked
exactly like a run where nothing matched.

`rules_matched` is now part of the execute response: the number of rules the filter selected.
The runner shows it, and when it is zero it says so plainly — "No engine rule matched this
filter… rules created here are stored as text and are not part of it". `specs/http-api.md`
records the boundary next to the endpoint.

`make e2e` now presses Run and asserts the explanation appears, so the page cannot regress to a
silent zero. It also refuses to run when its port is already in use: a stale server on the same
port made every route render blank once, because the browser received an `index.html` from the
old build against freshly built chunks.


## [9.0.7] — 2026-10-25 — Stale "mount-only" comments removed, governance checked for real

Four routes carried TODOs saying their components were mounted but not wired
(`TODO(22-05): wire ApprovalQueue fetch to /v1/approvals/*`). They had been wired since: driven
in a browser while signed in, approvals lists `GET /v1/approvals/pending`, audit lists
`GET /v1/audit` with its filters, branches lists the repository's branches, and rbac shows the
signed-in user's role and permissions. The comments described a state that no longer existed and
are replaced with what the code does.

`rbac` also now states its limit: the API has no role-assignment endpoint, so the page reports
the session's role and permissions rather than offering edits it could not save.

`make e2e` asserts all four against their endpoints while signed in — an empty state from the
API passes, a 401 does not.


## [9.0.6] — 2026-10-25 — The rule list shows every rule, and the edit page edits the rule

Two defects the browser gate found once it started creating rules.

`GET /v1/rules` returned exactly **one** rule however many existed: an omitted
`page_size` deserialises to `0`, and `clamp(1, 100)` turned that into a page of one. The field
documents a default of 50, and now gets it — capped at 100, named in constants rather than
inline. `specs/http-api.md` recorded the truncation as intended behaviour and is corrected.

`/rules/{id}/edit` invented its content: a hard-coded expression with the id spliced in, a
hit-policy selector, an empty decision table, and a Save that wrote a description containing a
timestamp. It never read the rule it claimed to edit. It now loads the rule by id — the name,
description and status it shows are the rule's own — saves the fields the API stores, and
reports a rule that cannot be loaded instead of rendering a stub. The expression editor, hit
policy and decision table are gone from it: they were scaffolding with no server counterpart,
and leaving them made the page look like an editor that worked.

`make e2e` now walks the whole cycle: create a rule, see it listed, open it, and find its name
in the edit form.


## [9.0.5] — 2026-10-25 — A rule can be created from the interface

`/rules/new` was a placeholder: a JavaScript expression editor, a hit-policy selector and an
empty decision table, with no name field, no submit button and no call to the API — while the
rule list offered a "Create Rule" link to it. The authoring side of the interface could display
rules and not create one.

It is now a form that matches what the API accepts — name, optional description, salience and
the rule body — and posts it, returning to the list on success and showing the API's message on
failure. The page states plainly that the body is stored as text and never compiled: the API
validates only that it is non-empty.

The client's `CreateRuleRequest` described the *view* model (`hitPolicy`, `conditions`,
`actions`) rather than the wire format (`name`, `dsl_body`, `tags`, `salience`, `status`), so a
create from the interface would have sent fields the server ignores. The edit page was sending
`hitPolicy` to `PUT /v1/rules/{id}`, which has no such field.

`make e2e` now creates a rule through the API from the browser, checks it appears in the list,
and checks the form renders with a name field and a submit button.


## [9.0.4] — 2026-10-25 — The browser gate stops deleting its own profile

`make e2e` reused no browser profile: it created one per run and deleted it on the way out,
which raced Chromium still flushing to it and printed an `ENOTEMPTY` warning on every run —
and once failed the step outright. It now uses one profile under `target/e2e/` and leaves it
there: it is build output, ignored by git, and a directory that is never deleted cannot fail
to be deleted.


## [9.0.3] — 2026-10-25 — Vite 8, Vitest 5, jsdom 30

The three remaining advisories after 9.0.2 lived in the build and test toolchain, and clearing
them meant major upgrades: `vite` 5 → 8, `vitest` 2 → 5, `@vitejs/plugin-react` 4 → 6, `jsdom`
25 → 30. `npm audit` now reports **3 moderate** advisories, all in Tailwind's PostCSS chain,
whose fix is the Tailwind 4 migration — a CSS rewrite, not a dependency bump.

Two declarations were removed while there: `@vitejs/plugin-react-swc`, which nothing imports
(`vite.config.ts` uses `@vitejs/plugin-react`), and `@types/axios`, a deprecated stub for a
package that ships its own types.

Vite 8 bundles with Rolldown. It still honours the `manualChunks` hook — the build emits one
`vendor` chunk containing React — but that guarantee now rests on `make e2e`, which loads the
real bundle and fails when a route renders nothing. That is the gate that caught the blank
page this configuration exists to prevent.


## [9.0.2] — 2026-10-25 — Dependency advisories: 44 to 9

`npm audit` reported 44 advisories in the frontend's dependency tree (11 critical, 20 high).
`npm audit fix` resolves 35 of them inside the declared ranges — the lockfile moves, no
manifest changes — leaving 9 that need major upgrades: `vite` 5 → 8 and `vitest` 2 → 5, taken
up separately rather than smuggled into a patch release.

The remaining advisories are in the build and test toolchain (`vite`, `vitest`, `tinypool`),
not in code the server or the browser runs.


## [9.0.1] — 2026-10-25 — Clippy 1.99 lints, and a version-bump script

Ten findings that clippy 1.99 reports and 1.94.1 does not, fixed rather than deferred:
`sort_by(|a, b| b.x.cmp(&a.x))` became `sort_by_key(.. Reverse(..))` in the tenant store and
both salience orderings; four `format!("…{}", &string)` borrows lost their `&`; and the
optional-sign split in the offset parser became a `match` on the two prefixes instead of an
`if`/`else` chain. The compiler stays pinned at 1.94.1 — the code now passes on both.

`make bump-version VERSION=x.y.z` (`scripts/version-bump.py`) writes the version to all eleven
places it lives: four crate manifests, `package.json` and its lockfile, the chart's `version`
and `appVersion`, the OpenAPI document, the server banner and metrics footer, and the citation
in `specs/http-api.md`. They had drifted once — crates at 0.1.0, frontend at 4.0.0, chart at
1.0.0 — and this is the cheapest way to stop that recurring.


### Added — gates that do not need infrastructure the project lacks

Two failure modes kept hiding behind workflows whose logs nobody could read:

- **`make image-check`** reads the `Dockerfile`'s `COPY` instructions, assembles
  exactly those files, and runs each stage's commands on the host. The image build is
  the one artefact this project cannot build everywhere (no container runtime in
  every environment), so a missing or misspelled `COPY` source only ever surfaced as
  a failed deploy. It runs as its own CI job.
- **The chart is linted and rendered in CI** (default values and each environment
  overlay), and again in the deploy job before cluster access is configured — a
  broken template used to look like a deployment failure.
- The deploy job asks the cluster whether it answers before attempting a release. A
  `KUBE_CONFIG` that reaches nothing now publishes the image and emits a warning
  naming the fix, instead of failing every push; a red Deploy means a deploy was
  attempted and failed.

CI also pins Rust 1.94.1 rather than "whatever stable is today", so a new clippy or
rustfmt cannot fail a commit that changed neither.

### Fixed — the web interface, checked in a real browser for the first time

Every one of these was invisible to the existing tests, because they run source
modules in jsdom and never load a production bundle or talk to a running server:

- **The production bundle rendered a blank page.** `build.rollupOptions.manualChunks`
  split React, TanStack and the rest across four chunks, and the `vendor` chunk read
  `useState` off an undefined React (`TypeError` on load). One vendor chunk now; bundle
  splitting is a caching optimisation, a build that does not boot is not worth 200 KB.
- **No single-page-application fallback.** `actix-files` answered 404 for `/rules`,
  `/login` and every other client-side route, so a reload or a bookmark failed before
  the router ran. Unknown non-API GETs now serve the shell; an unknown `/v1/...` still
  returns a 404, so a client typo cannot look like success.
- **No navigation.** `Navbar` existed and no route mounted it: the application was a
  landing page with every other page reachable only by typing its URL. It is mounted in
  the root route and lists Rules, Governance and Monitoring.
- **The governance section had no layout.** `governance.tsx` never registered its
  route, so the header and the tab bar for approvals/audit/branches/rbac never rendered.
- **A 401 hung the browser.** The API client did `window.location.href = '/login'` on
  every unauthorised response — including the several that fire in parallel when a page
  loads — which left the page blank and the renderer stuck mid-navigation. It now clears
  the session and announces it; the page renders its own error and the navbar offers the
  login link.
- **The rule list crashed as soon as it had data.** The API returns
  `{total_count, rules[], page, page_size, total_pages}` and single rules unwrapped; the
  client read `response.data.data`, so `rules` was `undefined` and `RulesList` threw.
  The wire format is now mapped to the view model in one place, which also fixes every
  date field (the API uses `created_at`, the view model `createdAt`, so sorting by date
  compared `NaN`).
- **Two client endpoints did not exist.** `PATCH /v1/rules/{id}/status` answered 404 (a
  status change is a `PUT`), and `/v1/rules/search` was matched by `/v1/rules/{id}` and
  answered 400.

### Added — `make e2e`: the interface checked in a browser

`frontend/e2e/smoke.mjs` drives the built bundle in headless Chromium over the DevTools
protocol — no dependencies — and `make e2e` builds the interface, starts the server on a
spare port with a throwaway store, then checks every route renders, that a deep link
works as an entry point, and that the app works once signed in. It is part of `make ci`
and of the CI workflow, so a blank page or a 404 route now fails the build.
`api/src/spa.rs` carries unit tests for the fallback (a client route serves the shell, an
unknown API path stays a 404).

### Fixed — the container image and the deployment probes

The `Deploy` workflow failed on its first run, and the image it built would not
have worked if it had. The `Dockerfile` predated the workspace layout: it copied
`api/Cargo.lock` and a `shared/` crate that do not exist, used `COPY … 2>/dev/null
|| true` (a `COPY` instruction has no shell), and asked cargo for
`--features engine/default`, which cannot select another member's features. It also
never built the web interface, so the image would have served an API and no UI.

`Dockerfile` is rewritten against the actual layout: a Node stage builds
`frontend/dist`, a Rust stage builds `-p api --bin rRules-server --locked` from the
workspace root, and a Debian slim runtime carries both with every startup path set
explicitly and a non-root user. `.dockerignore` no longer excludes `Cargo.lock`
(`Cargo.*.lock` matched it) and now drops the build output, dependencies and
process documentation from the context.

Two defects surfaced while verifying the runtime, both of which would have kept the
deployment from ever becoming healthy:

- The server bound `127.0.0.1:8080` unconditionally. In a container that is
  reachable only from inside it. It now reads `RRULES_BIND_ADDRESS` and `PORT`
  (defaults unchanged for local work), and the image sets `0.0.0.0`.
- The chart and the `k8s/` manifests probe `/health` and `/ready`, while the
  application served only `/health/live` and `/health/ready` — so every probe got a
  404, the container was restarted before it could become ready, and the Service
  had no endpoints. Both flat paths are now aliases of the existing handlers.


## [v9.0] - 2026-10-25 — Verified Conformance

The release where the project's claims became measurable. rRules ships with a specification it can
be checked against, and with the OMG DMN conformance suite green: **170 layer results pass, none
fail, four skipped as one recorded divergence** across 51 vectors, up from the 143 pass / 31 fail
that the first honest measurement reported.

Two changes define the release. The **specification** (`specs/`) states what the system does as 35
normative requirements plus four implementation-derived subsystem documents, each citing the code and
each ending with the gaps it does *not* close. **Verification** executes those requirements
(CodeWitness: 28 verified, 0 failed, 7 covered from the API crate) and the conformance corpus
(`make tck`), so a claim about this system now has a gate behind it.

Closing the conformance gap meant fixing the evaluator rather than the harness — boxed decision
bodies, business knowledge models, decision services, real FEEL filter semantics, a context-free
lexer, the temporal model, and the full arithmetic matrix. The details are below.

### Changed — DMN conformance measured at 170 passed / 0 failed / 4 skipped

The vendored OMG DMN TCK corpus now evaluates cleanly: **170 layer results pass, none fail,
four are skipped** across 51 vectors, up from the 143 pass / 31 fail of the first honest
measurement. The four skips are a single recorded divergence — `round up(5.5, 6177)` is an
error in the vector and unchanged in the specification — kept in `known_divergence_list` with
its expected and actual behaviour, so it is reported rather than hidden.

Closing those 31 layers meant fixing the evaluator, not the harness. Each of these previously
evaluated to a silent null:

- **Boxed decision bodies** — `<context>`, `<list>` and `<relation>` are modelled, parsed,
  round-tripped and evaluated, so the FEEL vectors written that way can run at all.
- **Business knowledge models and decision services are callable**: a BKM carries its formal
  parameters and body and is bound into the scope as a function value (so a call inside a `for`
  body sees the iteration variable); a service binds its arguments over its own input decisions
  and rejects a call that is short of arguments, names an unknown parameter, or passes the
  wrong primitive type.
- **Real filter semantics** — 1-based position selection with negative indices, scalar
  targets, context entries bound by name.
- **A context-free lexer**: `a*b` lexed as a single name. Names are re-merged after lexing
  against the names the model declares, which also makes `Date-Time2`, `values in a list` and
  `{foo+bar: "foo"}` parse.
- **Temporal completeness**: exponent literals, `24:00:00`, the `Z` designator, a time with an
  IANA zone, duration properties, `time offset`, and a single carrier for an offset-less
  date-time.
- **The arithmetic matrix** (1087 cases): negative and fractional duration scaling,
  `duration / duration`, commuted operands, the mixed naive/zoned subtraction rules, and
  left-associative `**`.
- **Aggregate and conversion contracts**: `stddev` is the sample deviation, `mode` always an
  ascending list, a non-numeric element makes an aggregate null, `number()` takes
  `(from, grouping, decimal)` and validates its separators, and `day of week` / `month of year`
  return names.

`specs/dmn-feel.md` is updated to match, with the two gaps this closes struck through.

### Fixed — DMN conformance (TCK 143/31 → 156/18 layer results)

Sixteen of the thirty-one failing layers now pass. Each was a real evaluator
defect, found by reading the vector rather than the summary:

- **Filter semantics** (`0069-feel-list`): `list[i]` selects by 1-based position
  (negative counts from the end, `0` and out-of-range are null, a scalar target
  behaves as a one-element list), `list[boolean]` filters, and a context element
  exposes its entries by name so `[{a:1}][a >= 2]` works. Previously every form
  was treated as a boolean filter over a list.
- **Numeric literals with an exponent** (`0068-feel-equality`): `1.23e4` lexed as
  `1.23` and the exponent was silently dropped — `12300 = 1.23e4` was false.
- **Aggregates** (`0059`–`0063`, `0094`): `all`/`any` take a list *or* separate
  arguments, accept a single boolean, reject non-booleans (rather than coercing),
  and treat `all([])`/`any([])` as vacuously true/false. `median`, `product`,
  `stddev` and `mode` accept the same two forms and treat any non-numeric element
  as an error instead of skipping it; `stddev` is the **sample** deviation and
  needs two values; `mode` always returns an ascending list.
- **`number()`** (`0058`): the positional order is
  `number(from, grouping separator, decimal separator)`, separators must differ,
  an omitted decimal separator defaults to `.`, and text containing a character
  that is not a digit, sign or declared separator is rejected.
- **`modulo`** (`0056`): the remainder is floored, so its sign follows the
  divisor (`modulo(-12, 5)` is 3).
- **Multi-word names** (`0016`, `0002-input-data-number`): FEEL allows spaces in
  names, and models use them (`values in a list`, `Monthly Salary`). The parser now
  merges a run of words when the joined text is a name the model or the request
  declares — previously such a name was a syntax error.
- **Context keys with spaces** (`0057`): `{foo bar: "foo"}` parses.
- **Regular expressions in string literals** (`0067`): an unknown escape keeps its
  backslash, so `"\s"` reaches `split` as `\s` instead of `s`.
- **`Z` time designator and offset-aware time equality** (`0068`):
  `time("09:30:00Z")` parses, and `time("10:30:00+01:00") = time("09:30:00Z")` is
  true because both denote the same instant.

Two unit tests encoded the superseded behaviour and were corrected rather than
worked around (`number(number)` is a type error; `mode` returns a list).

### Added
- **`make tck-ledger`** — the conformance record now carries a generated ledger of every failing
  layer (vector, layer, observed-versus-expected) alongside the hand-written digest of causes. The
  block between the `BEGIN/END GENERATED LEDGER` markers in `TCK-COVERAGE-GAPS.md` is owned by
  `scripts/update-tck-ledger.sh`, which runs the sweep and splices the result in, so the list cannot
  rot into a stale summary.
- **`tck_baseline_matches_documentation`** — a test that reads the pass/fail counts from
  `TCK-COVERAGE-GAPS.md` and compares them with the constants the sweep gates on. The document and
  the gate can no longer drift apart: changing one without the other fails the suite with both
  numbers in the message. Verified by deliberately breaking the document (150/20) and watching it
  fail.

### Fixed
- **GitHub Actions workflows restored and repaired.** The four workflows had been removed because
  none of them could run as written; they are back, fixed:

  | Workflow | Was | Now |
  |---|---|---|
  | `ci.yml` | triggered on `main` only; skipped frontend tests, the TCK sweep | triggers on `master` **and** `main`; runs `make ci` and `make tck`, so CI and the local gate cannot disagree |
  | `deploy.yml` | guarded the cluster step with `if: ${{ secrets.KUBE_CONFIG != '' }}`, which GitHub forbids; failed on every push | builds and pushes the image on every push, deploys only when `KUBE_CONFIG` is set (checked through a job-level `env`), and logs a notice when it is not |
  | `bench.yml` | uploaded the same artifact twice, `actions/cache@v3` | separate `benchmarks` and `memory-gate` jobs with distinct artifacts; the gate runs nightly and on the `bench-regression` label, not on every PR |
  | `fuzz.yml` | `actions/cache@v3`; ran on every PR | runs nightly and on the `fuzz` label; `taiki-e/install-action` for `cargo-fuzz` |

  All four pin `permissions` to the least privilege they need and add a concurrency group so
  superseded runs cancel. `make fuzz` now honours `FUZZ_MAX_TOTAL_TIME`/`FUZZ_RUNS` instead of
  hardcoding 60 s, and `engine/benches/README.md` documents the jobs and the thresholds again.

### Changed
- **`AGENTS.md` replaces `CLAUDE.md` as the project's instruction file.** It
  carries the same information an agent needs — what the project is, the measured
  state, the quality gates, where the specification lives, the working conventions
  — plus the rule that no built or generated binaries may enter git. It exists in
  two copies: this repository (version-controlled, travels with a clone) and the
  parent workspace. The `.claude/` directory and its tracked `settings.json` are
  removed.
- **History rewritten** to strip build artifacts: `target/`,
  `frontend/node_modules/` and `frontend/dist/` were in every commit, which inflated
  `.git` to 5.45 GB and made a push to GitHub impossible (the six 126–129 MB test
  binaries exceed the 100 MB per-file limit). The repository is now 3.3 MiB packed
  with 605 tracked files; all 494 commits and all eight tags survive, every SHA
  re-pointed. A `frontend/node_modules` that had been committed despite being
  listed in `.gitignore` was part of what came out — ignore rules never untrack
  what is already tracked.

### Added
- **A specification that can be executed.** `specs/` now holds the system
  specification — `requirements.md` (35 normative requirements with stable ids),
  plus `domain-model.md`, `rule-engine.md`, `dmn-feel.md` and `http-api.md`
  derived from the code with `path:line` citations and an explicit `Known gaps`
  section each. `engine/codewitness.yaml` (28 Rust scenarios) and
  `api/codewitness.yaml` (integration-suite mapping) wire
  [CodeWitness](https://github.com/codewitness/codewitness) to it. Measured:
  28 VERIFIED, 0 FAILED, 7 UNVERIFIED (the API requirements, covered from the
  `api` project instead).
- `engine/tests/rule_semantics.rs` — firing order, fixpoint, numeric comparison
  and retraction pinned across all four engines.
- `engine/tests/tck_values.rs` — typed parsing of TCK test XML (values, lists,
  contexts) with tolerance-aware structural comparison.

### Changed
- `tck_sweep` records every layer outcome, prints each failure reason, evaluates
  every `resultNode` of a case, and gates on a documented baseline instead of
  hiding failures. Measured TCK state: 136 layer results pass, 24 fail, 14 skip.
- `engine` enables its four engines by default, so `cargo test -p engine` tests
  them.
- `SemanticValidator` lost its unused lifetime parameter and the `unsafe`
  transmute that propped it up.

### Removed
- The legacy React tree under `api/src/{components,features}` (18 files) — dead
  code in the Rust crate, superseded by `frontend/`.

- **Tenant management API** — `GET/POST /v1/tenants` and
  `GET/DELETE /v1/tenants/{id}` are now registered behind the `/v1` auth scope.
  The handlers build on `engine::governance::tenant::MemoryTenantStore`, which
  was previously uncompiled dead code: it gained synchronous
  `create`/`get`/`list`/`delete` cores plus `TenantStoreError`
  (blank name → `400`, duplicate name → `409`), and the async
  `TenantStoreBackend` trait impl now delegates to them instead of duplicating
  the logic.
- `api/tests/tenants.rs` — tenant CRUD contract plus a per-tenant rule-isolation
  test proving two `X-Tenant-ID` values never see each other's rules.

### Housekeeping
- Updated stale `CLAUDE.md` to reflect v5.0 (was a full milestone behind, still
  describing v4.0 as current with v5.0 as a "candidate thesis").
- Removed the dead `api/src/models/` module (duplicate `ExecuteRequest`/
  `ExecuteResponse`/`InferenceTraceEntry`/`RuleFilter`/`TraceSummary` plus
  duplicate `rules` DTOs and a `coverage.rs`). The canonical types live in
  `api::v1::execution` and `api::v1::rules`; nothing in the workspace imported
  `models::`. (v5.0 retrospective flagged the `execution.rs` half; the whole
  module was equally dead.)
- Deleted a stray bare git repository left in `rules/` (untracked, no content).
- Replaced stale `// pub mod auth;` / `// pub mod tenants;` TODO comments in
  `api/src/v1/mod.rs` with accurate pointers (auth is at `crate::auth` +
  `crate::middleware::auth`; tenants route module not yet wired).
- Added `.planning/TECH-DEBT.md` recording the three engine retraction
  `TODO`s (Rete/TREAT/LEAPS network cleanup) and the multi-tenancy route gap as
  known debt, intentionally deferred from this pass.

### Verification
- `cargo test --workspace`: all binaries passing, 0 failed (~353 tests).
- `cargo clippy --workspace --all-targets`: 0 errors (4 pre-existing warnings).
- `npx vitest run`: 5/5 passing.

## [v8.0] - 2026-07-15 — Close TCK Evaluator Gaps

### Added
- **Numeric built-in coverage** (Phase 40) — `abs()` extended to
  `DaysTimeDuration`/`YearsMonthsDuration`; `number(from, grouping, decimal)`
  3-arg form with locale-independent parsing; named-parameter resolution by
  positional fallback in evaluator dispatch (`abs(n: -1)`); `median`, `mode`,
  `stddev`, `product`; arithmetic edge cases (empty-list SUM/MIN/MAX → null).
- **String + list built-in coverage** (Phase 41) — `sort()` comparator-lambda
  support via `apply_function` (correct lambda scope handling) and an
  empty-list guard returning null per DMN 1.5; `filter` lambda invocation;
  edge-case tests for `split` (no delimiter / empty string / null inputs),
  `concatenate`, `insert`/`remove`, and `list contains` null propagation.
- **TCK-COVERAGE-GAPS.md** — repository-root record of the single remaining
  divergence (DMN15-31 COLLECT with `distinct`, a DMN 1.6 / FEEL 1.6 feature,
  out of scope for DMN 1.5 Level 3) with spec references and disposition.
- **Verification artifacts** — `.planning/milestones/v8.0-ROADMAP.md`,
  `.planning/milestones/v8.0-MILESTONE-AUDIT.md`, and
  `.planning/phases/43-tck-conformance-verification-close-out/VERIFICATION.md`
  recording per-phase deliverables and evidence.
- **Comprehensive built-in tests** — `engine/tests/feel_builtins_comprehensive.rs`.

### Changed
- **OMG DMN TCK evaluator coverage: 21/51 → 51/51 vectors** (123/123 layer
  passes; compliance-level-2 110/110, compliance-level-3 13/13), with zero
  regression in the 30 vectors that already parsed and round-tripped. Phase 42
  (context/relational/temporal) required no code change — the v6.0 Phase-31
  implementations already satisfied their vectors and are now verified by the
  sweep rather than assumed.
- Rete rule-execution path (`/v1/execute`, `EngineRegistry`) untouched — zero
  regression by scope lock.

### Fixed
- **Multi-word FEEL function names were unparseable** — the lexer emits one
  `Name` token per word and the parser stopped at the first one, so
  `upper case("x")` parsed as `Name("upper")` and evaluated to `null`. Every
  multi-word built-in (`upper case`, `lower case`, `string length`,
  `string join`, `list contains`, `substring before/after`, `starts with`,
  `ends with`, `get value`, `get entries`, `date and time`, `round *`,
  `years and months duration`) was affected inside real DMN models.
  `Parser::merge_multiword_name` now re-joins adjacent words when the result is
  exactly a registered built-in name, leaving operators (`and`, `or`, `in`)
  untouched.
- **`sort(list, comparator)` ignored the comparator** — the Phase 41 "comparator
  support" was never in the code; the function returned the input list. Sorting
  now applies the comparator through `eval::apply_function_value`, supporting
  boolean (`x > y`) and numeric comparator results, with `sort([]) → []` per
  DMN 1.5 §10.3.4.2.5.
- **Rounding functions ignored their `scale` argument** — `round up`,
  `round down`, `round half up` and `round half down` always rounded to an
  integer (`round half up(1.25, 1)` returned `1`). They now use
  `rust_decimal::RoundingStrategy` at the requested scale (ties away from zero /
  toward zero per spec).
- **`get entries(c)` returned strings** instead of the spec's list of
  `{key, value}` contexts (now emitted in sorted-key order), and
  **`years and months duration(from, to)`** had no two-date form (now computes
  the month delta).
- **`retract_fact` was a stub returning `Ok(false)`** in Rete, TREAT and LEAPS,
  so four `engine/tests/retract_integration.rs` tests were permanently red.
  Retraction now removes the fact from working memory, alpha memory and every
  beta-memory partial match (`BetaMemory::remove_fact`), drops TREAT's join
  cache and resets LEAPS aggregation nodes.
- **TCK structural comparator hung and emitted malformed XML** —
  `engine/src/dmn/test_util.rs` looped forever on the first attribute-less tag
  and rebuilt tags without their closing `>`. Replaced with `canonicalize_tags`
  (attributes sorted, self-closing tags expanded, CDATA unwrapped), and the
  stray `eprintln!` step tracing is gone.
- **Two `discover_tck_vectors` unit tests never matched the fixture layout**
  (they omitted the `<vector>-test-NN.xml` naming the discovery code requires).
- **TCK precision audit scanned attribute values** — DMN ids such as
  `113e6550-…` were read as numbers with enormous exponents, so the A1 gate
  (`rust_decimal` suffices for the value domain) always failed. The scan is now
  restricted to element text and confirms A1 across 102 XML files.
- **`-D warnings` clippy gate was red** — fixed the outstanding lints
  (`if let` destructuring, collapsible `if`, map-value iteration, needless range
  loop, `is_empty` companion, type aliases, `large_enum_variant` waiver with
  rationale). `make lint` now passes.
- Compile break in `engine/src/validation.rs` unit tests: `ComparisonOp` was
  dropped from the module imports while still used by
  `test_semantic_validator_types`; the import now lives in the test module.

### Verification (re-verified 2026-10-25)
- `cargo test --workspace`: every target green (696 engine lib tests; 8/8
  retraction integration tests, was 4/8; 20/20 FEEL built-in tests).
- `cargo test -p engine --test tck_sweep`: 51 vectors, 123/123 layer passes,
  0 failures, evaluator layer now compares **every** test case structurally via
  `engine/tests/tck_values.rs` (typed `xsd:*` values, `<list>`, `<component>`).
- `cargo clippy --workspace -- -D warnings`: clean.
- `npx vitest run`: 28/28; `npx tsc --noEmit`: clean.

## [v7.0] - 2026-07-15 — Performance + Production Readiness

### Added
- **Benchmark infrastructure** (Phase 36) — Criterion `dmn_benchmarks` suite
  (`make bench`) covering parser, exporter, evaluator, and decision tables.
- **Memory regression gate** (Phase 36) — frozen `v6.0-baseline.json` snapshot
  over a 60-file corpus, `memory_baseline` binary, and `scripts/memcheck-gate.sh`
  (`make memcheck`) failing on RSS/allocation regression.
- **Fuzzing** (Phase 38) — `cargo-fuzz` targets `feel_parser` and
  `decision_table`, committed regression corpus, `make fuzz` runner, and a
  dedicated `fuzz.yml` CI workflow.
- **Property-based tests** (Phase 38) — proptest invariants for FEEL evaluation
  (including context topological order independence) in
  `engine/tests/feel_properties.rs`.

### Changed
- **Zero-copy DMN XML loading** (Phase 37) — dropped per-element `into_owned()`
  in the parser; re-baselined the memory snapshot to the 60-file corpus.
- **FEEL AST caching** (Phase 37) — lexer token cache + parse-with-cache entry
  point and decision-table cell AST warming; `feel_cache_wiring` tests.
- **Parallel DRD evaluation** (Phase 39) — topological sort of decision
  dependencies with rayon level-by-level evaluation of independent decisions
  inside one `DecisionEvaluator::evaluate` call.

## [v6.0] - 2026-07-10 — DMN Level 3 Compliance

### Added
- **DMN 1.5 model + parser + exporter** (Phase 29) — streaming namespace-aware
  `quick-xml` parser (DMN 1.1–1.5) with XXE rejection, CDATA preservation,
  entity resolution, and ID/annotation preservation into a canonical IR;
  writer-based exporter with an 8-test DOM-compare round-trip gate.
- **FEEL value domain** (Phase 30) — Decimal128 numbers via `rust_decimal`
  (no f64 on the production path, `0.1 + 0.2 = 0.3` proven), five temporal types
  with ISO 8601 `@""` round-trip and IANA zones (`chrono-tz`), DST-correct
  duration arithmetic, and the FEEL type lattice with exactly the three
  spec-defined implicit conversions (`1 + "2" → null`).
- **FEEL expression language** (Phase 31) — grammar rules 1–65, boxed
  expressions, if/for/some/every, path navigation, filters, lambdas with lexical
  capture, three-valued logic and null propagation, ~70+ built-in functions, and
  per-thread AST caching.
- **Decision-table engine** (Phase 32) — all 7 hit policies (UNIQUE, ANY,
  PRIORITY, FIRST, RULE ORDER, OUTPUT ORDER, COLLECT) with SUM/MIN/MAX/COUNT
  aggregators.
- **Decision evaluation API** (Phase 33) — `POST /v1/decisions/{id}/evaluate`
  with multi-output decision services and DRD chaining; `/v1/decisions` CRUD,
  import/export/IR/hot-reload; per-decision Prometheus counters and DRD depth
  gauge at `/v1/metrics/dmn` + `/v1/metrics/dmn/ws`; `DmnDecisionTracker`
  dead-decision detection.
- **Frontend DMN wiring** (Phase 34) — hit-policy model refactored to the DMN
  canonical 7-policy union with a COLLECT aggregator sub-type;
  `DecisionTable`/`HitPolicySelector` wired to the real backend evaluator with
  results and trace panels.
- **Conformance close-out** (Phase 35) — TCK sweep across parser, evaluator,
  round-trip, and DMNDI layers; Level 3 conformance checklist with evidence;
  zero-regression verification.

### Changed
- The DMN surface is a **parallel evaluation path** (`engine/src/dmn/`) and is
  deliberately not routed through `/v1/execute` or `EngineRegistry`; the Rete
  rule path is unchanged by construction.

## [v5.0] - 2026-07-06 — Live Execution

### Added
- **Real rule execution** — `POST /v1/execute` fires rules through
  `EngineRegistry` (fresh `Box<dyn ExecutionEngine>` per request in
  `web::block`) and returns truthful `rules_fired`/`rules_not_fired`/
  `facts_after` from post-execution working memory. Closes the long-standing
  `rules_fired: 0` stub.
- **Live monitoring data** — per-execution Prometheus counters + WebSocket
  `MetricsSnapshot` broadcast (Phase 24); `RuleExecutionTracker` as a
  `LazyLock<Mutex<…>>` singleton so dead-rule detection reads live firings
  (Phase 26). Three metric surfaces (Prometheus, dashboard, dead-rule) agree.
- **Real reload + JobStore** (Phase 25) — `SharedRuleSet` swap-don't-mutate with
  a generation counter; async reload writes `ReloadJob` transitions;
  batch-execute aggregates truthfully.
- **Stateful sessions** (Phase 27) — `SessionManager` (DashMap sharded
  registry) with TTL, LRU eviction, max-count, background sweep, and a gauge
  shipped together; stateful `POST /v1/execute/session/{id}` endpoints.
- **Frontend test-runner** (Phase 28) — `execution.ts` + `TestRunner` component;
  engine-selector dropdown; inference trace surfaced; `rule_filter` honored
  server-side for a true "test this rule" UX.

### Fixed
- v3.0 `engine::`→`crate::` E0433 import bug across all four engine modules —
  the pluggable-engine surface compiles for the first time.
- Minimal direct rule evaluator built on `ReteEngine` and mirrored onto
  TREAT/LEAPS/PHREAK via a shared `direct_eval` module.
- Milestone-audit cross-phase WARNINGs (frontend `rule_filter` dead input;
  `/metrics` `active_sessions` hardcoded) resolved with regression tests before
  tagging.

## [v4.0] - 2026-07-02 — Production Platform

### Added
- **Production React/TypeScript frontend** — CodeMirror rule IDE, decision-table
  editor (all DMN hit policies), guided editors, rule-management dashboard.
- **Enterprise governance backend** — RBAC (admin/author/reviewer/viewer) with
  JWT auth, maker-checker approval workflow, immutable audit trail with
  CSV/JSON export, Git-backed branching/merging with conflict resolution.
- **Monitoring & metrics** — Prometheus `/metrics`, dashboard API, dead-rule
  detection, coverage reporting.
- **Complete management API** — rules/categories/tags CRUD, approvals, audit,
  branches.
- **Deployment & CI/CD** — multi-stage Alpine Docker, Helm chart
  (dev/staging/prod), hot reload with async job tracking.
- **Frontend-backend integration closure** (Phase 22) — aligned frontend to
  `/v1`, JWT-protected routes + AuthProvider, mounted orphaned routes, real
  WebSocket metrics streaming, audit/approval/branch UI wired to backend.

<!-- Link references resolve within this file; the repository is published at
     github.com/thread3d/rRules. -->
[Unreleased]: #unreleased
[v9.0]: #v90---2026-10-25--verified-conformance
[9.0.0]: #v90---2026-10-25--verified-conformance
[v8.0]: #v800---2026-07-15---close-tck-evaluator-gaps
[v7.0]: #v700---2026-07-15---performance--production-readiness
[v6.0]: #v600---2026-07-10---dmn-level-3-compliance
[v5.0]: #v500---2026-07-06---live-execution
[v4.0]: #v400---2026-07-02---production-platform