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
TypeScript + Vite + TanStack). Storage is hybrid — Git for rule versioning, PostgreSQL for runtime
metadata. Ships as a Docker image and a Helm chart (`helm/rrules-server`, with dev/staging/prod
overlays; `k8s/` holds the equivalent raw manifests for `kubectl apply -k`).

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
  requirements — **28 VERIFIED, 0 FAILED, 7 UNVERIFIED** for `engine`; the 7 are the HTTP API
  requirements, covered from `api` where their integration suites pass.
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
```

`make help` lists the rest (benchmarks, memory-regression gate, fuzzing, frontend dev server).
`make bench`, `make memcheck` and `make fuzz` need a nightly toolchain or a frozen baseline snapshot.

`make ci` includes the browser check, which drives the real bundle in headless
Chromium: every route must render, a deep link must load, and the app must work once
signed in. Unit tests in jsdom cannot see a bundle that throws at load or a route the
server answers with 404 — both of which shipped before this gate existed.

**CI runs exactly these gates.** `.github/workflows/ci.yml` invokes `make ci` and `make tck` on
pushes and pull requests to `master`/`main`, so a green check means the same thing as a green run
locally. `bench.yml` runs the benchmark and memory-regression gates nightly, `fuzz.yml` fuzzes the
parser and decision-table loader nightly, and `deploy.yml` publishes the image on every push and
deploys only when `KUBE_CONFIG` is configured. Run the gates yourself anyway — do not push something
you have not seen pass.

## Where things live

| Artifact | Location |
|----------|----------|
| Specification (start here) | `specs/README.md` |
| Normative requirements | `specs/requirements.md` |
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
`TCK-COVERAGE-GAPS.md` and the `Known gaps` sections of `specs/` — candidates: DMN 1.6 / FEEL 1.6
(COLLECT `distinct`, the one remaining corpus divergence), `<functionDefinition>`/`<invocation>`
decision bodies, non-literal business knowledge model bodies, real `PostgresDmnStore` queries, and
PostgreSQL-backed tenants. The repository is published at
github.com/thread3d/rRules (`origin`, default branch `main`).
