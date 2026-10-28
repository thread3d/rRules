# rRules — agent instructions

This file travels with the repository. The same guidance exists in the parent workspace's
`AGENTS.md`; keep the two in sync.

## What the project is

rRules is a production-grade **Business Rules Management System** built in Rust, with a React
frontend. A team defines business decisions as reusable rules, deploys them without a rebuild, and
watches their impact.

**Core value:** users define complex business decisions as reusable rules, deploy them instantly, and
monitor their impact — without modifying application code.

**Layout:** a Cargo workspace (`engine/`, `api/`, `engine_state/`) plus `frontend/` (React 19 +
TypeScript + Vite + TanStack). Rule and decision documents live on disk, rule versioning is a git
repository, and every runtime store is in memory — a restart starts empty. A real PostgreSQL-backed DMN
store exists behind the `postgres` feature, is selected with `DMN_PERSISTENCE=postgres`, and is tested
against a live server (`make db-test`); the filesystem store remains the default. Ships as a Docker image and a Helm chart
(`helm/rrules-server`, with dev/staging/prod overlays; `k8s/` holds the equivalent raw manifests for
`kubectl apply -k`).

**Key requirements:** a Rete-family rule engine with forward/backward chaining and runtime-selectable
algorithms (Rete, TREAT, LEAPS, PHREAK); a web rule-authoring IDE; a REST API for execution and
management; RBAC with maker-checker approval; OMG DMN 1.5 support with measured TCK conformance.

## Current state

Release **9.0.0** (tag `v9.0`) is complete: the project has a specification and independent
verification, and the measured state below is the only state worth quoting.

- **`specs/` is the specification.** `requirements.md` holds 35 normative requirements with stable
  ids (`ENG-*`, `DMN-*`, `FEEL-*`, `API-*`, `SYS-*`). `domain-model.md`, `rule-engine.md`,
  `dmn-feel.md` and `http-api.md` are implementation-derived, cite `path:line`, and each ends with a
  `Known gaps` section stating what the code does **not** do.
- **Verification:** [CodeWitness](https://github.com/codewitness/codewitness) executes the
  requirements — **35 VERIFIED, 0 FAILED, 0 UNVERIFIED**, unioned over the two crates: `engine`
  executes the 28 engine/DMN/FEEL requirements and `api` the 7 HTTP ones, each through scenarios
  that run the real code (`api` builds the application in-process). `make verify` runs both and
  prints the union; a requirement neither half executes is treated as a gap and fails the gate.
- **DMN conformance:** **170 layer results pass, 0 fail, 4 skipped** across 51 vectors — every
  evaluator layer in the vendored corpus, with the four skips being one recorded divergence
  (`round up(5.5, 6177)`, a reference implementation's scale limit that the specification does not
  state). `TCK-COVERAGE-GAPS.md` holds the measured history and a generated ledger; the sweep gates on
  that baseline and `tck_baseline_matches_documentation` fails if the document and the gate disagree.
- **Build:** `cargo test --workspace` green, `cargo clippy --workspace -- -D warnings` clean,
  `cargo fmt --all -- --check` clean.

The compiler is pinned (1.94.1, in `.github/workflows/ci.yml` and the Dockerfile's
builder), because a drifting compiler fails a commit that changed nothing: clippy 1.99
already reports about twenty-five findings in this tree that 1.94.1 does not. Bumping
the pin is therefore a deliberate task — expect to fix those lints in the same change.

**A claim without a gate behind it is not a claim this project should make.** The v8.0
"51/51 TCK vectors" figure was written from phase summaries and did not survive execution; it is now
recorded as wrong wherever it appeared outside the historical archives.

## Quality gates — run these before saying anything is done

```bash
make ci          # build, test, clippy -D warnings, fmt --check, frontend build + tests
make tck         # DMN TCK sweep, baseline-gated, per-layer report
make tck-ledger  # regenerate the failure ledger inside TCK-COVERAGE-GAPS.md
make verify      # CodeWitness against specs/requirements.md (engine + api)
make e2e         # browser smoke test of the built UI against a live server
make image-check # check the Dockerfile's build logic without a container runtime
make chart-lint  # lint + render the Helm chart (needs helm v3.14.0; CI's chart job)
make db-test     # the PostgreSQL store against a real server pg-embed downloads and starts
```

`make help` lists the rest (benchmarks, memory-regression gate, fuzzing, frontend dev server).
`make bench` runs on the pinned stable toolchain and `make memcheck` compares against the tracked
frozen baseline (`engine/benches/snapshots/baseline.json`); `make fuzz` is the only one that needs
the nightly toolchain, plus `cargo-fuzz`.

CodeWitness compiles the crate itself and passes `--offline` to cargo, so `make verify` runs with
`CARGO_NET_OFFLINE=false`: a lockfile pinning a version newer than its index snapshot otherwise fails
to resolve and every requirement is reported BLOCKED.

`make ci` includes the browser check, which drives the real bundle in headless
Chromium: every route must render, a deep link must load, and the app must work once
signed in. Unit tests in jsdom cannot see a bundle that throws at load or a route the
server answers with 404 — both of which shipped before this gate existed.

**CI runs these gates** — `make ci`, `make tck`, `make spec-citations`, `make image-check`, the chart
lint (`make chart-lint`) and the database tests (`make db-test`) — so a green check means the same
thing as a green run locally.
`bench.yml` runs the benchmark and memory-regression gates nightly, `fuzz.yml` fuzzes the parser and
decision-table loader nightly, and `deploy.yml` publishes the image on every push and deploys only
when `KUBE_CONFIG` is configured.

`make verify` is the exception, and the difference matters. CodeWitness is not published: it is a jar
installed from a source checkout, so a hosted runner cannot obtain it. The workflow's
`Requirement verification` job runs only when the repository variable `CODEWITNESS_ENABLED` is
`true` *and* the secret `CODEWITNESS_JAR_B64` holds the jar (base64); otherwise the job is
**skipped** — a skipped job, never a passing one. When it is skipped the requirement verification did
not run in CI on that push: run it locally. Run every gate yourself anyway — do not push something
you have not seen pass.

## Where things live

| Artifact | Location |
|----------|----------|
| Specification (start here) | `specs/README.md` |
| Normative requirements | `specs/requirements.md` |
| HTTP application | `api/src/app.rs` — state and the builder `main` and the tests share |
| Verification config | `engine/codewitness.yaml`, `api/codewitness.yaml` |
| DMN TCK conformance | `TCK-COVERAGE-GAPS.md` |
| API reference / user guide | `docs/API.md`, `docs/user-guide.md` |
| Documentation index | `docs/README.md` |
| Historical documents (superseded) | `docs/archive/` — do not treat as current |
| Roadmap, state, milestones, tech debt | `../.planning/` (workspace level, outside this repository) |
| Changelog | `CHANGELOG.md` |
| Public entry point | `README.md` (what the system does and what is verified) |

`../.planning/phases/**` and `../.planning/milestones/**` are the project's process history: records
of what past phases claimed, not current documentation. Do not cite them as truth. They live at the
workspace root rather than in this repository: they document how the project was built, and are not
published with it.

## Working conventions

- **A handler that gains a dependency breaks every hand-rolled test app at runtime, not at
  compile time.** Ten integration tests in `api/tests/` mount handlers on a bare `App` to bypass
  auth; when a handler starts extracting `web::Data<T>`, those apps answer **500** instead of
  failing to build. Adding a dependency to a handler means grepping `api/tests/*.rs` for
  `to(<handler>)` in the same commit, and the stores carry `test_channel()`/`init_store()` helpers so
  the fix is a one-liner.
- **Verify before claiming.** Run the gate, paste the evidence, then report. Prefer a smaller change
  with a passing test over a larger one with a plausible story.
- **Keep the specification true.** If you change behaviour, update the matching `specs/` document in
  the same commit. If you close a documented gap, strike the entry and say what replaced it; a spec
  that keeps asserting a fixed defect is as wrong as code that keeps the bug.
- **Prefer deletion to accretion.** This code base accumulated a duplicated evaluator, placeholder
  tests (`assert!(true)`), a second Helm chart and a committed `node_modules`. When two things do one
  job, delete one.
- **State limits plainly.** If something is stubbed, partially wired or unverifiable here, write it in
  the `Known gaps` section instead of describing intent as fact.
- **Do not rewrite history** without explicit permission, and never commit secrets.

## What the verification loop taught — keep these true

A 25-round fix-and-verify loop ran on `rRules/`. Almost every defect it found was **a claim nothing
executed**, and the remedies all had the same shape: make the claim executable, then put it in a gate
that runs on every push. If you read one section of this file, read this one.

### The law, and its disguises

*"A claim without a gate behind it is not a claim this project should make."* The claims that turned out
false were rarely code bugs first — they were **statements**:

| Disguise | What was actually wrong |
|---|---|
| Prose | A memory baseline frozen at v6.0 that no release could pass; a README quoting "1005 tests, 38 targets" while the tree ran 996 across 40; four "removed" TODOs still in the file |
| A machine-readable contract | `/api-docs/openapi.json` omitted **21 endpoints**, including all of auth — a generated client could not log in |
| Configuration | `k8s/` set `SERVER_HOST`/`SERVER_PORT` while the server read `RRULES_BIND_ADDRESS`/`PORT`; the chart never set `API_SECRET_KEY`, so deployments ran on a public development string |
| Code nothing compiled | Three `api` modules and a 992-line test suite that **no module declared**, so nothing had ever type-checked them |
| A test that asserted nothing | `assert!(cache_hits >= 0 \|\| cache_misses >= 0)` on unsigned counters; five tests looping over an **empty** engine list |
| A command nobody ran | The README's first command was `cargo run --bin api`; the binary is `rRules-server`, so it failed with `no bin target named api` |

### Two habits that actually catch things

**1. Verify in the mode the gate will run in, not the mode that is convenient.**
A warm working tree hid a flaky test for two rounds: `api/tests/dmn_metrics_extended.rs` shared one
global tracker and passed by scheduling luck until a clean clone ran it and CI failed. Before calling a
suite green, run it the way CI does — `git clone --local . ../ci-repro` and build from cold. Related,
and worse: `git checkout <path>` reverts to **HEAD**, so it silently discards the work-in-progress you
were testing; copy the file to a scratch directory first.

**2. Test the checker against the defect it was written for.**
Five of this workspace's gates were themselves wrong, and each was found by reintroducing the bug and
watching the check *not* fail:

- a README check whose `--bin` matcher also accepted the package name, so `--bin api` passed;
- a `curl` check that sent `POST` to a `GET` route, whose method-mismatch 404 fell through to the
  single-page shell and reported a good endpoint as missing;
- a browser `visit` that waited for *any* text (the navbar supplies it) and then asserted on data a page
  fetches later — so it intermittently "proved" a route blank;
- a memory gate whose baseline was twelve releases old;
- a cross-engine suite that iterated an empty engine list and passed five assertions without testing an
  engine.

A check that has never failed has never been tested. Break the thing, watch it fail, put it back.

### Two implementations of one thing always drift

`main.rs` and the test-state constructor built the same application twice, and the test copy had no
engines registered — so every in-process API test exercised an engine-less server. `agenda.rs` and
`conflict_resolution.rs` each defined their own `ConflictStrategy`: two types with the same name, one
exported. `tailwind.config.ts` declared a class-based dark mode the app never activated, while eighteen
files carried `dark:` utilities that could not match.

The fix is always the same: **one implementation, and a test that its consumers agree.** `build_app` for
the router, `ApiPaths`/`from_paths` for the state, one cross-engine suite driven by
`available_engines()`, `scripts/verify-summary.py` for the verification union.

### Delete, wire, or document — and say which

Dead code documented as a gap is still dead code. Three orphaned `api` modules sat in a gaps table for
months; they went in one commit. A module returning hard-coded zeroes was both unregistered and lying,
so the module went rather than being registered. Placeholders that return `Ok(false)` unconditionally
were deleted after their callers turned out not to exist.

The rule that emerged: **delete** what nothing calls, **wire** what the docs promise and the code can
support, and only then **document** a gap — in the `Known gaps` table, with what it costs a reader. Gaps
are struck through with the release that closed them, never quietly removed.

### Sandbox facts that cost time here

- `$HOME` is read-only. Point tool caches inside the workspace: `CARGO_HOME=$PWD/.cargo-home`,
  `XDG_CACHE_HOME=$PWD/.gh-cache`, `npm_config_cache=…`, `HELM_CACHE_HOME=…`. `make db-test` downloads
  its PostgreSQL into `XDG_CACHE_HOME` too, so run it as
  `XDG_CACHE_HOME=$PWD/.db-cache make db-test`.
- `cc` is a ccache wrapper: **`CCACHE_DISABLE=1`** for anything that compiles C (release builds,
  `libgit2-sys`, fuzzing, CodeWitness), or it fails with `ccache: error: Read-only file system`.
- Never share a port with something already listening. Both the API server and Chromium's DevTools
  endpoint must take an OS-assigned free port: a stale server on a fixed port made every route render
  blank once, and a fixed DevTools port failed CI.
- `cmd | tail` reports *tail's* exit status. Use `cmd > log 2>&1; echo "exit=$?"`.
- Some tools **replace** their child's environment rather than inheriting it. CodeWitness forwards
  `HOME` but not `CARGO_HOME`, so its cargo resolves against `$HOME/.cargo` and fails on a read-only
  home; `make verify` therefore points `HOME` at `.verify-home/` for that one command. Never put git
  in the same recipe as a `HOME` override — it loses `$HOME/.gitconfig` and the author identity.

## When CI cannot run: local evidence, and a debt to repay

GitHub will refuse to start workflow jobs when the account's billing lapses — every job fails in a few
seconds with "recent account payments have failed or your spending limit needs to be increased" and
no step executes. That is not a code result and there is nothing in the repository to fix.

The agreed procedure when that happens:

1. **Keep working**, on local gates only. Do not describe a release as CI-green.
2. **Mark the release in `CHANGELOG.md`**: append `CI/Deploy not confirmed: account billing` to the
   entry's verification line, so the debt is visible where a reader looks.
3. **Keep the exact list of unverified commits.** `gh run list --limit N` reproduces it afterwards.
4. **Repay the debt when billing returns**: `gh run rerun <run-id>` for every workflow run of every
   unverified commit (the commits are already pushed; nothing is lost), then confirm green in `gh run
   view` before resuming normal operation.
5. Do **not** mark the goal complete while any pushed commit is unverified by CI.

## Never commit binaries

**No built or generated binaries in git. Ever.** Not in a commit, not in history, not "just this
once" to unblock a build.

This repository paid for that mistake: compiled test binaries under `target/debug/deps/`
(126–129 MB each) and a committed `frontend/node_modules` inflated `.git` to **5.5 GB**, made the
repository unpushable — GitHub rejects any file over 100 MB with `GH001` — and forced a full history
rewrite on 2026-10-25 in which every commit SHA and every tag changed. The repository is now
~3.4 MB packed.

- Do not weaken `.gitignore`, and never `git add -f` a path it ignores. It covers `target/`,
  `frontend/node_modules/`, `frontend/dist/`, `engine/fuzz/target/`,
  `engine/benches/snapshots/current.json`, `.codewitness/`, `.codebase-memory/` and the DMN
  runtime store `api/dmn/`.
- **Ignore rules never untrack what is already tracked.** Check what you stage:
  `git status --short`, then `git diff --cached --stat`. A surprising file count is a red flag.
- Large **source** assets are fine — TCK XML corpora, fixtures, frozen benchmark snapshots. The rule
  is about build output and dependency trees, not size.
- Generated files that must be published belong in a CI artifact, not in a commit.

Before pushing:

```bash
git count-objects -vH
git cat-file --batch-check --batch-all-objects | awk '$2=="blob" && $3>10000000 {print $3}'
```

Both should be unremarkable. A blob over 100 MB makes the push fail outright; a repository over 1 GB
means something regenerable has slipped in.

## History, and where it came from

Shipped milestones: v1.0 MVP → v2.0 Testing Framework → v3.0 Pluggable Engines → v4.0 Production
Platform → v5.0 Live Execution → v6.0 DMN Level 3 Compliance → v7.0 Performance + Production
Readiness → v8.0 Close TCK Evaluator Gaps → **v9.0 Verified Conformance** (specification,
verification, and a green conformance corpus). The v1.0 scope (core engine, rule-authoring IDE, REST
API, governance, testing/monitoring, DMN compliance) is all delivered; the DMN commitment is measured
rather than asserted, which is what `TCK-COVERAGE-GAPS.md` exists to do.

**Next step:** no milestone is in flight. Scope the next one from `../.planning/TECH-DEBT.md`,
`TCK-COVERAGE-GAPS.md` and the `Known gaps` sections of `specs/`. What is left, in the order it costs a
reader something:

1. **Backward chaining** — `Engine::execute_backward` and `backward_chaining::execute_backward` still
   return a fixed result, and `lib.rs` re-exports the latter as an entry point. Forward chaining works
   (9.0.52); this is the other half of a headline requirement.
2. **LEAPS aggregation** — `recalculate` returns `None` and every fact is fed to every node, so the
   aggregations a LEAPS network is built for never compute.
3. **DMN 1.6 / FEEL 1.6** — COLLECT `distinct`, the one remaining corpus divergence.
4. **`rsa` / `crypto-bigint`** — linked but unreachable (the API verifies HS256 only); see `SECURITY.md`
   for what closing that would take.

The repository is published at github.com/thread3d/rRules (`origin`, default branch `main`).
