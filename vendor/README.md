# Vendored dependencies

One crate is vendored here, for one reason, and it is meant to leave.

## `pg-embed` 1.0.0 — patched to `zip` 9

`pg-embed` downloads the PostgreSQL binaries that `make db-test` needs (a JAR from Maven whose single
`.txz` entry is the server) and starts them. It is a **dev-dependency** of `engine_state` and of
`db_tests`, which is why `zip` appeared in both `Cargo.lock` files and why Dependabot reported the same
advisories twice.

Upstream 1.0.0 — the newest release, published 2026-02-24 and unchanged since — pins `zip = "8"`. Every
known `zip` advisory is fixed in 9.0.0 (`GHSA-m665-v7pm-7grj`, the critical one, in 9.0.5), so the
dependency cannot be fixed by resolving within the requirement it declares:

```text
RUSTSEC-2026-0443 / -0446 / -0447 / -0448 / -0455   fixed in zip 9.0.0 … 9.0.5
GHSA-m665-v7pm-7grj (critical)                      fixed in zip 9.0.5
```

### What was changed

Exactly two things, both marked `PATCHED BY THIS REPOSITORY` where they appear:

1. `Cargo.toml` — `[dependencies.zip] version = "8"` → `"9"`.
2. `src/pg_unpack.rs` — zip 9's `ZipFile::name()` returns `Result<Cow<str>, ZipError>` where zip 8
   returned `&str`, so the entry name is read once, checked, and released before the entry is read.

Nothing else in the crate was touched. `assets/` was dropped because it is documentation images that
nothing compiles, and so were the two files crates.io adds to a published copy (`.cargo_vcs_info.json`,
`.gitignore`). Every remaining file is as published, so re-vendoring a future release is a `diff` and not
an archaeology exercise.

### Why this is safe

`pg-embed` uses `zip` in one place, read-only: it opens the downloaded JAR, iterates its entries, and
copies the `.txz` payload out. `ZipArchive::new`, `len`, `by_index`, `read_to_end` are unchanged in
zip 9, and the compiler proves the rest — the patch does not compile against zip 8.

The one line that changed is the one that matters for the advisories: with zip 9 the JAR is parsed by the
version that fixed the path-traversal, decompression and decoder advisories.

### How it is verified

```bash
cargo tree -i zip                    # one zip, 9.x, reached only through this crate
cargo audit                          # 0 vulnerabilities
XDG_CACHE_HOME=$PWD/.db-cache make db-test   # from a cleared cache → exercises the patched unpack
```

`make db-test` was run from an **emptied** `pg-embed` cache when this was introduced, so the download and
unpack path really executes rather than being skipped by an extraction that already existed.

### How it goes away

Delete this directory and the `[patch.crates-io]` block in the root `Cargo.toml` **and** in
`db_tests/Cargo.toml` (it is its own workspace) as soon as `pg-embed` publishes a release that allows
`zip` 9 — then `cargo update -p pg-embed` and the two lockfiles pick it up. Check with:

```bash
curl -s https://crates.io/api/v1/crates/pg-embed/1.0.0/dependencies | grep -A2 '"zip"'
```

### How it was produced

```bash
cp -r ~/.cargo/registry/src/*/pg-embed-1.0.0 vendor/pg-embed
rm -rf vendor/pg-embed/assets vendor/pg-embed/.cargo-ok vendor/pg-embed/Cargo.lock
# then the two changes above
```
