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
3. **Development-only.** `zip` and `webpki-roots` arrive through test-only dependencies (`pg-embed`
   downloads and extracts a zip; a test client pulls the roots). `cargo tree -e normal -i <crate>`
   reports both as absent from the shipped dependency set.

The distinction in group 3 is checkable, which is why it is written down:

```bash
cargo tree -e normal -i zip        # empty → not in the shipped binary
cargo tree -i zip                  # present → the test build carries it
```

## Hardening already in the code

- Untrusted input never panics: the FEEL parser and the DMN parser return error nodes (FEEL-012,
  SYS-002), and both are fuzzed nightly.
- DMN parsing rejects `DOCTYPE` and processing instructions (XXE), and the parser is bounded in depth and
  token count.
- Every business route requires a JWT (API-001); rules are isolated per tenant (API-003); the audit trail
  records who changed what.
- The server binds loopback by default, and the chart requires a `API_SECRET_KEY` rather than shipping a
  development string.
