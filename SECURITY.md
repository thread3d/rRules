# Security

## Reporting

Open an issue at <https://github.com/thread3d/rRules/issues>, or for anything you would rather not post
publicly, contact the repository owner directly. There is no bounty programme.

## Dependency auditing

`make audit` runs [`cargo audit`](https://github.com/rustsec/rustsec) against `Cargo.lock` and **fails on
any advisory that is not accounted for**. `.cargo/audit.toml` holds the account: each ignored advisory is
listed with the reason it is acceptable, and an ignore that no longer matches anything is reported, so
the list cannot rot into a silencer.

`npm audit` covers the interface; it reports **0 vulnerabilities** as of 9.0.51.

### What is in the tree today, and why

`cargo audit` was first run in 9.0.51 and found **25 advisories**. Fixing them changed the tree:

| Fixed | How |
|---|---|
| `quick-xml` 0.40 (RUSTSEC-2026-0194, -0195) | bumped to 0.41. **This is the crate that parses untrusted DMN XML**, so it was the one that mattered most. |
| `crossbeam-epoch`, `h2 0.4`, `quinn-proto` | updated within their major versions |
| `protobuf` 2.x (RUSTSEC-2024-0437) | `prometheus` 0.13 → 0.14 |
| `zstd-sys` | dropped by the same actix bump |

Fourteen advisories remain, in three groups:

1. **Unreachable at runtime.** `jsonwebtoken` compiles RSA and ECDSA support for every build, so `rsa`
   and `crypto-bigint` are linked. The API verifies **HS256 only** (`api/src/middleware/auth.rs`); no
   request reaches those code paths. `RUSTSEC-2023-0071` has no fixed release. Closing this means
   switching `jsonwebtoken`'s crypto backend to `aws_lc_rs` (which brings a C toolchain into the image)
   or replacing the crate for a ~50-line HS256 verifier — recorded as work, not assumed.
2. **Pinned by actix-http.** Every release of our HTTP server up to 3.20 depends on `h2 0.3.x` and
   `zstd 0.13.x`, the versions the advisories name. Nothing in this repository can patch a dependency of
   a dependency; the entries in `audit.toml` are what the next actix-http bump must clear.
3. **Build- and test-time only.** `zip` arrives on two paths, neither of which reads an archive this
   project's *users* control: `utoipa-swagger-ui`'s build script unpacks the pinned Swagger UI release
   while `api` is compiled, and `pg-embed` — a dev-dependency of `engine_state` — unpacks the PostgreSQL
   build that `make db-test` downloads. `webpki-roots` comes with a test-only TLS client. The exposure is
   a supply-chain one at build or test time, not a request path. `cargo tree -e normal -i <crate>`
   reports all three as absent from the shipped runtime graph.

The distinction in group 3 is checkable, which is why it is written down:

```bash
cargo tree -e normal -i zip        # empty → not in the shipped binary
cargo tree -i zip                  # present → the build and test tree carries it
```

### What `make audit` cannot see

`cargo audit` reports **RustSec** advisories, and the gate is exactly as strong as that database. GitHub's
advisory database knows about more: on 2026-11-02 the default branch carried **11 open Dependabot alerts,
all on `zip`** (2 critical, 7 high, 2 moderate, counted per lockfile in `Cargo.lock` and
`db_tests/Cargo.lock`), and none of them is mirrored into RustSec, so `make audit` is green while the
security page is not. Recorded here rather than left to be rediscovered:

| Path | Version | What would clear it |
|---|---|---|
| `utoipa-swagger-ui` 7 build script — build-time, in the shipped image | `zip` 1.x | `utoipa-swagger-ui` 7 → 10, which requires `utoipa` 4 → 5 and therefore a migration of every `#[utoipa::path]` and `#[derive(ToSchema)]` in `api`. Work, not a patch. |
| `pg-embed` — dev-dependency, `make db-test` | `zip` 8.6.0 | `pg-embed` 1.0.0 is the newest release and pins it; nothing in this repository can move it |

The critical one (`GHSA-m665-v7pm-7grj`, patched in `zip` 9.0.5) is reached only through `pg-embed`, which
unpacks the PostgreSQL archive it fetched itself. The build-time path unpacks the Swagger UI release pinned
in `utoipa-swagger-ui`. Both are trusted, fixed inputs — which is why this is recorded as accepted risk
with its expiry condition rather than treated as an incident.

## Hardening already in the code

- Untrusted input never panics: the FEEL parser and the DMN parser return error nodes (FEEL-012,
  SYS-002), and both are fuzzed nightly.
- DMN parsing rejects `DOCTYPE` and processing instructions (XXE), and the parser is bounded in depth and
  token count.
- Every business route requires a JWT (API-001); rules are isolated per tenant (API-003); the audit trail
  records who changed what.
- The server binds loopback by default, and the chart requires a `API_SECRET_KEY` rather than shipping a
  development string.
