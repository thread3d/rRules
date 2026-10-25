# Documentation

| Document | What it is |
|----------|------------|
| [`API.md`](API.md) | REST API reference: conventions, authentication, every registered endpoint, error model, tenancy. |
| [`user-guide.md`](user-guide.md) | How to write rules, aimed at a rule author rather than a developer. |
| [`../specs/`](../specs/README.md) | **The specification.** Normative requirements with stable ids, plus implementation-derived subsystem specs that CodeWitness executes against. |
| [`../specs/requirements.md`](../specs/requirements.md) | The 35 requirements CodeWitness verifies, in the form it parses. |
| [`../TCK-COVERAGE-GAPS.md`](../TCK-COVERAGE-GAPS.md) | Measured DMN TCK conformance: the baseline the test suite enforces, a digest of failure causes, and a generated ledger of every failing layer (`make tck-ledger`). |
| [`../CHANGELOG.md`](../CHANGELOG.md) | Released behaviour, newest first. |
| [`archive/`](archive/) | Historical audits, kept for provenance. Superseded — do not treat as current. |

## Where to start

- **What the system must do** → `specs/requirements.md`.
- **How a subsystem works** → `specs/domain-model.md`, `specs/rule-engine.md`, `specs/dmn-feel.md`, `specs/http-api.md`.
- **What is verified, and with what evidence** → `specs/README.md` and the CodeWitness report under `<crate>/.codewitness/runs/`.
- **How to run it, and the rules for working here** → `AGENTS.md`, plus the `Makefile` (`make help`).
