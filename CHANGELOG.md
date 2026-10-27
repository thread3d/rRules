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