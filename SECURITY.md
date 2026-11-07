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

The advisories that remain fall into three groups:

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

`RUSTSEC-2026-0429` (2026-10-22, fixed in `zip` 9.0.0) is the newest member of group 3 and the first `zip`
advisory that is **in RustSec**, so `make audit` sees it where it could not see the GitHub-only ones below.
It cannot be cleared from this repository on either path: `utoipa-swagger-ui` 7 pins `zip` 1.x, and
`pg-embed` 1.0.0 — the newest release — pins `zip` 8.6.0, so even the Swagger UI migration would leave the
advisory reported through the database tests. It is accepted, with the same expiry conditions as the rest of
group 3, and `cargo audit` re-reports it on every release until one of those two dependencies moves.

### The 2026-11-06 HTTP-stack advisories: fixed, not accepted

Five advisories landed against the HTTP stack on **2026-11-06** — after the last CI run that executed and
before the next one — and every one of them is on the request path of the shipped server, so none of them
belongs in `audit.toml`:

| Advisory | Crate | Fixed in |
|---|---|---|
| `RUSTSEC-2026-0446` request smuggling through malformed chunked trailers | `actix-http` 3.20.0 | 3.20.2 → bumped to **3.20.3** |
| `RUSTSEC-2026-0448` unbounded decompression exhausts memory and CPU | `actix-http` 3.20.0 | 3.20.1 → **3.20.3** |
| `RUSTSEC-2026-0455` large-window Brotli bodies bypass decompression limits | `actix-http` 3.20.0 | 3.20.3 |
| `RUSTSEC-2026-0447` compressed request bodies exhaust memory before extractor limits | `actix-web` 4.16.0 | 4.18.2 |
| `RUSTSEC-2026-0443` hidden-file disclosure through Windows short names | `actix-files` 0.6.10 | 0.7.4 |

That they were found locally rather than by CI is the point worth keeping: `make audit` runs in `ci.yml`,
CI could start no job between 2026-11-04 and 2026-11-06 (the account billing lapse recorded in
`CHANGELOG.md` 9.0.88), and the first push after that would have failed the gate. The `actix-files`
requirement moved with the advisory (`0.6` → `0.7`), because 0.7.4 is the first fixed release and the
0.x line does not auto-upgrade across a minor.

`actix-http` 3.20.3 still resolves `h2 0.3.27` and `zstd 0.13.3`, so group 2's ignores are unchanged and
still needed — re-measured by running the gate, not re-asserted.

The distinction in group 3 is checkable, which is why it is written down:

```bash
cargo tree -e normal -i zip        # empty → not in the shipped binary
cargo tree -i zip                  # present → the build and test tree carries it
```

### What `make audit` cannot see

`cargo audit` reports **RustSec** advisories, and the gate is exactly as strong as that database. GitHub's
advisory database knows about more: on 2026-11-02 the default branch carried **11 open Dependabot alerts,
all on `zip`** (2 critical, 7 high, 2 moderate, counted per lockfile in `Cargo.lock` and
`db_tests/Cargo.lock`), and none of them was mirrored into RustSec, so `make audit` was green while the
security page was not. Recorded here rather than left to be rediscovered:

| Path | Version | What would clear it |
|---|---|---|
| `utoipa-swagger-ui` 7 build script — build-time, in the shipped image | `zip` 1.x | `utoipa-swagger-ui` 7 → 10, which requires `utoipa` 4 → 5 and therefore a migration of every `#[utoipa::path]` and `#[derive(ToSchema)]` in `api`. Work, not a patch. |
| `pg-embed` — dev-dependency, `make db-test` | `zip` 8.6.0 | `pg-embed` 1.0.0 is the newest release and pins it; nothing in this repository can move it |

RustSec and GitHub are not disjoint: `RUSTSEC-2026-0429`, published 2026-10-22, is one of these advisories
and *is* mirrored, which is why `make audit` reports it. The others are still invisible to the gate.

The critical one (`GHSA-m665-v7pm-7grj`, patched in `zip` 9.0.5) is reached only through `pg-embed`, which
unpacks the PostgreSQL archive it fetched itself. The build-time path unpacks the Swagger UI release pinned
in `utoipa-swagger-ui`. Both are trusted, fixed inputs — which is why this is recorded as accepted risk
with its expiry condition rather than treated as an incident.

## Hardening already in the code

- Untrusted input never panics: the FEEL parser and the DMN parser return error nodes (FEEL-012,
  SYS-002), and both are fuzzed nightly.
- DMN parsing rejects `DOCTYPE` and processing instructions (XXE). The **FEEL expression** parser is
  bounded in depth (256) and token count; the **DMN XML** parser is not — it has no size, nesting or
  element-count limit, which `specs/dmn-feel.md` records as gap 43 — so `POST /v1/decisions/import`
  accepts an unbounded document from an authenticated caller. This bullet claimed the bound applied to
  "the parser" until 9.0.87; naming which parser is the point.
- Every business route requires a JWT (API-001); rules are isolated per tenant (API-003); the audit trail
  records who changed what.
- The server binds loopback by default, and the chart requires a `API_SECRET_KEY` rather than shipping a
  development string.
