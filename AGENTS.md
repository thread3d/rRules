# AGENTS.md — rules for agents working in this repository

The canonical agent rules for this project live one level up, in
[`../AGENTS.md`](../AGENTS.md). Read that first; this file exists so the rule travels with the
repository and is visible to anyone who clones it.

The project's own instructions are in [`CLAUDE.md`](CLAUDE.md): current state, quality gates
(`make ci`, `make tck`, `make verify`), and where the specification lives (`specs/`).

## Never commit binaries

**No built or generated binaries in git. Ever.** Not in a commit, not in history.

This repository paid for that mistake: compiled test binaries under `target/debug/deps/`
(126–129 MB each) and a committed `frontend/node_modules` were in its history, which inflated `.git`
to **5.5 GB** and made the repository unpushable — GitHub rejects any file over 100 MB (`GH001`).
The history was rewritten on 2026-10-25 to remove them; every commit SHA and every tag changed. The
repository is now ~4 MB packed.

In practice:

- Do not weaken `.gitignore`, and never `git add -f` a path it ignores. It covers `target/`,
  `frontend/node_modules/`, `frontend/dist/`, `engine/fuzz/target/`, `engine/benches/snapshots/v6.0-current.json`,
  `.codewitness/`, `.codebase-memory/` and the DMN runtime store `api/dmn/`.
- `.gitignore` does not untrack what is already tracked. Check what you stage:
  `git status --short`, then `git diff --cached --stat`. A surprising file count is a red flag.
- Large **source** assets are fine — TCK XML corpora, fixtures, frozen benchmark snapshots. The rule
  is about build output and dependency trees, not size.
- Generated files that must be published belong in a CI artifact, not in a commit.

Before pushing:

```bash
git count-objects -vH
git cat-file --batch-check --batch-all-objects | awk '$2=="blob" && $3>10000000 {print $3}'
```

Both should report nothing alarming. A blob over 100 MB makes the push fail outright, and a
repository over 1 GB means something regenerable has slipped in.
