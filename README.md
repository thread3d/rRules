# rRules

A business rules management system in Rust: define business decisions as reusable rules, deploy
them without rebuilding, and watch their impact.

**9.0.15** — released from a verified tree: everything in the table below was measured by a gate,
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

Claims in this repository come with a gate attached, and every one of these runs in CI:

```bash
make ci              # build, test, clippy -D warnings, fmt --check, frontend build + tests
                     # … plus the browser gate: every route, a deep link, and a signed-in flow
make tck             # OMG DMN TCK conformance sweep, baseline-gated
make verify          # CodeWitness: execute specs/requirements.md
make spec-citations  # every `path:line` in specs/ still resolves to the code it names
make image-check     # the Dockerfile's stages build from its COPY set, without a container
make memcheck        # memory-regression gate against a committed baseline
make fuzz            # bounded fuzzing of the FEEL parser and the decision-table loader
```

| | Measured |
|---|---|
| Tests | 1005 Rust tests across 38 targets, plus 28 frontend tests — all green |
| DMN TCK | **170 layer results pass, 0 fail, 4 skipped** across 51 vectors |
| Requirements | **28 verified, 0 failed** (CodeWitness); 7 more covered from the API crate |
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
make run                         # API on :8080
make frontend-dev                # IDE on :5173
make tck && make verify          # the conformance and spec gates
```

Storage is hybrid: Git for rule versioning, PostgreSQL for runtime metadata (`migrations/`).

## Licence

MIT — see [`LICENSE`](LICENSE).
