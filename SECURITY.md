# Security

## Reporting

Open an issue at <https://github.com/thread3d/rRules/issues>, or for anything you would rather not post
publicly, contact the repository owner directly. There is no bounty programme.

## Dependency auditing

`make audit` runs [`cargo audit`](https://github.com/rustsec/rustsec) against **every lockfile in the
repository** — `Cargo.lock`, `db_tests/Cargo.lock` and `engine/fuzz/Cargo.lock` — and **fails on any
advisory that is not accounted for**. `.cargo/audit.toml` holds the account: each ignored advisory is
listed with the reason it is acceptable, and an entry is removed by the release that removes the crate,
so the list cannot rot into a silencer.

The three-lockfile sweep is 9.0.90's other lesson. The two excluded workspaces keep their own
`Cargo.lock`, and until that release the gate read only the root one: on 2026-11-07 the root lockfile
audited clean while `db_tests/Cargo.lock` held **five** advisories the gate could not see — the four
actix request-path ones below and one in `libz-sys` — and `engine/fuzz/Cargo.lock` held a sixth, its own
older `libz-sys`. Dependabot had already told us the manifest mattered: **six of its eleven open `zip`
alerts were counted against `db_tests/Cargo.lock`**, because its dependency graph reads every lockfile
where the gate read one. A gate that reads one third of the lockfiles is green on two thirds of the tree.

`npm audit` covers the interface; it reports **0 vulnerabilities** as of 9.0.51.

### What is in the tree today, and why

`cargo audit` was first run in 9.0.51 and found **25 advisories**. Fixing them changed the tree:

| Fixed | How |
|---|---|
| `quick-xml` 0.40 (RUSTSEC-2026-0194, -0195) | bumped to 0.41. **This is the crate that parses untrusted DMN XML**, so it was the one that mattered most. |
| `crossbeam-epoch`, `h2 0.4`, `quinn-proto` | updated within their major versions |
| `protobuf` 2.x (RUSTSEC-2024-0437) | `prometheus` 0.13 → 0.14 |
| `zstd-sys` | dropped by the same actix bump |

Five advisories remain, in three groups, and **9.0.90 closed a fourth** — the `zip` group, carried as
accepted risk since the gate was introduced and described below.

1. **Unreachable at runtime.** `jsonwebtoken` compiles RSA and ECDSA support for every build, so `rsa`
   and `crypto-bigint` are linked. The API verifies **HS256 only** (`api/src/middleware/auth.rs`); no
   request reaches those code paths (`sqlx-mysql`, an optional feature of an optional dependency, names
   `rsa` for the same reason). `RUSTSEC-2023-0071` has no fixed release. Closing this means switching
   `jsonwebtoken`'s crypto backend to `aws_lc_rs` (which brings a C toolchain into the image) or
   replacing the crate for a ~50-line HS256 verifier — recorded as work, not assumed.
2. **Pinned by actix-http.** `actix-http` 3.20.3 — the version that closed the five request-path
   advisories below — still resolves `h2 0.3.27`, which `RUSTSEC-2026-0258` names. Nothing in this
   repository can patch a dependency of a dependency; the entry in `audit.toml` is what the next
   `actix-http` bump has to clear, and `cargo tree -i h2@0.3.27` is the measurement. The two `zstd 0.13`
   entries that used to sit beside it left with `zip` 8 in 9.0.90: `actix-http` 3.20.3 resolves `zstd`
   0.14, and there is nothing to ignore any more.
3. **In a lockfile, absent from every build.** A lockfile is feature-independent, so `rust_decimal`'s
   optional `rkyv` feature and `pg-embed`'s optional `sqlx` feature resolve into it while no feature this
   tree enables compiles them — `cargo tree -i rkyv` and `cargo tree -i webpki-roots` find no parent in
   the resolved graph. They are listed in `audit.toml` rather than suppressed globally, so that enabling
   either feature meets a decision instead of a surprise.

### The `zip` group: closed in 9.0.90

For five releases this document carried `zip` as build-time and test-time risk, because both of its paths
were pinned by someone else's requirement:

| Path | Was | What moved it |
|---|---|---|
| `utoipa-swagger-ui` 7's build script — runs while `api` compiles, ships in the image | `zip` 1.x | `utoipa-swagger-ui` 10.0.2 declares `zip ^9.0`. Reaching it means `utoipa` 4 → 6 and `utoipa-swagger-ui` 7 → 10. |
| `pg-embed` — dev-dependency of `engine_state` and `db_tests`, what `make db-test` starts | `zip` 8.6.0 | 1.0.0 is the newest release and still pins `zip = "8"`; the crate is **vendored with two marked changes** in `vendor/pg-embed`, and `vendor/README.md` records why, what changed, how it was verified and when to delete it. |

The Swagger UI migration was not a version bump. `utoipa` 6 requires every type a path or a body refers
to to implement `ToSchema`, and two api-local types (`TraceResponse`, `ReloadDmnRequest`) plus the
`engine`-owned tenant and simulation types did not — `engine` was still on `utoipa` 4, so its derives
implemented a *different* trait than the one `api` asked for. `engine` now shares `utoipa` 6, so one
version of the trait is in the tree. It also exposed a latent defect worth keeping in the record: `crate::health::live` and `crate::health::ready` were listed
**twice** in `openapi.rs`'s `paths(...)`, which utoipa 4 silently deduplicated; utoipa 6 fails to compile
on it. That duplicate is the reason the document was generated with each health path once rather than
twice, and no gate could have told you which it was.

After both paths:

```bash
cargo tree -i zip        # one zip 9.4.1, build-dependency of utoipa-swagger-ui 10.0.2
cargo audit              # 0 vulnerabilities in all three lockfiles
```

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

`db_tests/Cargo.lock` had the same four actix advisories for the same reason — it pins `api` by path, so
it resolves `actix-web` independently — which is how the missing half of this gate came to light in
9.0.90.

### What `make audit` still cannot see

`cargo audit` reports **RustSec** advisories, and the gate is exactly as strong as that database. GitHub's
advisory database knows about more, and the two are not the same set: on 2026-11-02 the default branch
carried **11 open Dependabot alerts, all on `zip`**, none of which was mirrored into RustSec — so
`make audit` was green while the security page was not. Those eleven are closed by 9.0.90, and the lesson
is kept rather than archived: **when the two disagree, the security page is the one that is telling the
truth**, and a mirror is not a superset. The check is `gh api "/repos/OWNER/REPO/dependabot/alerts?state=open"`,
and Dependabot scans all three lockfiles where `make audit` used to scan one.

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
