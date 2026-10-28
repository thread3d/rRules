# rRules

A business rules management system in Rust: define business decisions as reusable rules, deploy
them without rebuilding, and watch their impact.

**9.0.66** — released from a verified tree: everything in the table below was measured by a gate,
not summarised. See [`CHANGELOG.md`](CHANGELOG.md) for what shipped.

## What it does

- **Rules and decisions.** A Rete-family engine with runtime-selectable algorithms (Rete, TREAT,
  LEAPS, PHREAK), salience ordering, agenda groups, and fact retraction with truth maintenance.
- **DMN 1.5.** A canonical IR, a FEEL implementation (Decimal arithmetic, not floating point),
  decision tables with all seven hit policies, and a DRD evaluator with topological levels and
  parallel execution.
- **HTTP API.** REST endpoints for rule and decision management, live execution, governance
  (approval workflows, audit), and metrics.
- **Authoring.** A React 19 web IDE for rules, decision tables, and test runs.
- **Deployment.** A Docker image and a Helm chart with dev/staging/prod overlays.

## What is verified

Claims in this repository come with a gate attached. All of these run locally; CI runs them all
except the requirement verification, which needs a tool that is not published (see the note below):

```bash
make ci              # build, test, clippy -D warnings, fmt --check, frontend build + tests
                     # … plus the browser gate: every route, a deep link, and a signed-in flow
make tck             # OMG DMN TCK conformance sweep, baseline-gated
make verify          # CodeWitness: execute specs/requirements.md
make spec-citations  # every `path:line` in specs/ still resolves to the code it names
make docs-snippets   # every Rust example in the user guide compiles and its assertions pass
make readme          # every command the README tells you to run resolves
make deploy-env      # every variable the chart and manifests set is one the server reads
make image-check     # the Dockerfile's stages build from its COPY set, without a container
make memcheck        # memory-regression gate against a committed baseline
make fuzz            # bounded fuzzing of the FEEL parser and the decision-table loader
```

The browser gate, the citation check, the guide's examples, the README's commands and the deployment
variables are all part of `make ci`, which is what CI runs on every push.

`make verify` executes the specification with [CodeWitness](https://github.com/codewitness/codewitness),
which is a jar installed from a source checkout rather than a published package. CI runs it only
when the repository variable `CODEWITNESS_ENABLED` is `true` and the jar is supplied as a secret;
otherwise that job is skipped, which is the honest state and not a pass.

| | Measured |
|---|---|
| Tests | **996 Rust tests across 40 targets**, 6 ignored, plus 28 frontend tests — all green |
| DMN TCK | **170 layer results pass, 0 fail, 4 skipped** across 51 vectors |
| Requirements | **35 verified, 0 failed, 0 unverified** (CodeWitness, over the engine and API crates) |
| Lints | `cargo clippy --workspace -- -D warnings` clean, rustfmt clean |

The four TCK skips are one recorded divergence — a vector asserting a `round up` scale limit that
DMN 1.5 does not state — kept in `engine/tests/tck_sweep.rs` with its expected and actual behaviour
rather than hidden. Every remaining failure was fixed in the evaluator, not by loosening the harness.

## Where to start

| Read | For |
|---|---|
| [`specs/README.md`](specs/README.md) | The specification, and how to verify it |
| [`specs/requirements.md`](specs/requirements.md) | 35 normative requirements with stable ids |
| [`TCK-COVERAGE-GAPS.md`](TCK-COVERAGE-GAPS.md) | Measured conformance, and the one divergence |
| [`docs/user-guide.md`](docs/user-guide.md) | Using the system |
| [`docs/API.md`](docs/API.md) | The REST API |
| [`AGENTS.md`](AGENTS.md) | Working on this repository: conventions, gates, limits |
| [`CHANGELOG.md`](CHANGELOG.md) | Release history |

The specification documents each end with a **Known gaps** section that states what the code does
*not* do. That is deliberate: a specification that quietly omits its gaps is worth less than one that
names them.

## Quick start

```bash
cargo build --workspace          # engine, api, engine_state
make run                         # API on :8080 (RRULES_BIND_ADDRESS / PORT)
npm --prefix frontend install    # once, before the interface
make frontend-dev                # IDE on :5173, proxying the API
make tck && make verify          # the conformance and specification gates
```

Storage today: rule and decision *documents* live on disk, and rule versioning is a git repository
(`<RULE_STORAGE_PATH>/.git`, libgit2). Everything the API manages at runtime — rules, categories,
tags, tenants, approvals, audit events, sessions — is in memory, so a restart starts empty.

Decision documents can also be kept in **PostgreSQL**: build with `--features postgres` and set
`DMN_PERSISTENCE=postgres` with a `DATABASE_URL`, and the `/v1/decisions` endpoints persist there
instead of to `DMN_STORAGE_PATH`. The default is unchanged — the filesystem store, which is what git
tracks — and a deployment that asks for Postgres without the feature or the URL fails at startup
rather than silently writing files. The tenant and audit stores remain in memory;
`migrations/` holds their schema.

The **audit trail** can be persisted the same way: `AUDIT_PERSISTENCE=postgres` records every
governance event into a table instead of process memory, so the trail survives a restart — which is the
property an audit trail is for. It is append-only, and the in-memory default is unchanged.

The **tenant registry** too: `TENANT_PERSISTENCE=postgres` keeps the organisations that own rules in a
table instead of process memory, so a restart no longer forgets every tenant and orphan the rules
scoped to them.

All of them are exercised against a real database: `make db-test` downloads PostgreSQL with `pg-embed`
and runs the DMN store's tests, an HTTP-level test that imports, lists, reads, exports and deletes a
decision, and two that record governance events and read them back — including one that reopens the
database from a second connection to prove the trail outlived the writer. `specs/http-api.md` records this under its known gaps rather than
implying otherwise.

## Licence

MIT — see [`LICENSE`](LICENSE).
