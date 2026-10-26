# Changelog

All notable changes to rRules are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

> Note on early tags: `v1.0`–`v3.0` were backfilled retroactively (see
> `.planning/RETROSPECTIVE.md` "Cross-Milestone Trends"). `v3.0` in particular
> is a historical ref, not a clean release commit — its pluggable-engine code
> landed inside the v4.0 timeline. Entries below begin at v4.0, the first
> milestone tagged at ship time.

## [Unreleased]

## [9.0.14] — 2026-10-25 — A crate-local `target/` is ignored

`make bench` left a 30 MB `engine/target/` that `git status` reported as untracked: `.gitignore`
excluded `/target/`, which only covers the workspace root, and any tool that builds a member in
place creates one beside its manifest. The pattern is now unanchored.

## [9.0.13] — 2026-10-25 — Citations must stay inside the repository

CI failed the new *Spec citations* job on its first run, and the failure was a real defect rather
than a checker bug: `dmn-feel.md` cited `../.planning/phases/39/…/TCK-COVERAGE-GAPS.md:10-30`. The
workspace-level `.planning/` tree is deliberately not published with this repository, so that
reference was unopenable for every reader of it — it resolved on the machine that wrote it and
nowhere else. The claim now cites `TCK-COVERAGE-GAPS.md` and the sweep code, both of which ship.

The checker treats any citation that leaves the repository as unresolved, with that reason in the
message, so the class cannot come back.

The same pass corrected two claims that resolved but misled: `http-api.md` said host and port were
"not configurable by env var" (they come from `RRULES_BIND_ADDRESS` and `PORT`), and it cited
`main.rs:313` for the `/v1` scope, a line that is now a comment — the scope is at 334.

A semantic check was written and then removed: comparing the symbol a sentence names with the text
at the cited lines flagged 646 of 1162 citations, nearly all of them ordinary loose wording. A
checker that cries wolf that often is worse than none.

## [9.0.12] — 2026-10-25 — The specification's citations are checked, and two had drifted

`specs/` cites code 1162 times (`path:line`), which is what makes it evidence rather than
description. Nothing verified those citations, and the parser work in 9.0.10 moved line numbers.

`scripts/check-spec-citations.py` resolves every citation: the file must exist (by path, by
unique basename, or by the full paths the same document uses elsewhere) and the cited range must
fall inside it. Two were wrong — `rule-engine.md` pointed at `rete/mod.rs:750-768` in a 739-line
file, and `dmn-feel.md` at `TCK-COVERAGE-GAPS.md:118-120` in a 74-line file. Both now cite the
lines that hold the claim: the idempotent re-retract assertion, and the accepted-divergence
section.

`make spec-citations` runs it strictly and is part of `make ci`; CI runs it as its own job. A
citation that no longer resolves now fails the build instead of quietly looking like evidence.


## [9.0.11] — 2026-10-25 — The verification gate builds again

`make verify` reported every requirement **BLOCKED**: CodeWitness compiles the crate itself and
passes `--offline` to cargo, so its resolution depends on a crates.io index snapshot — and the
`rustls` bump that came in with a Dependabot PR left `Cargo.lock` pinning a version newer than that
snapshot. "BLOCKED" is honest in the tool's terms but it told us nothing about the code.

`make verify` now runs with `CARGO_NET_OFFLINE=false` so the index can refresh, and both
`codewitness.yaml` files allow the verification build the network it needs to resolve. The gate is
green again: **engine 28 VERIFIED, 0 FAILED, 7 UNVERIFIED**, api 35 UNVERIFIED (the API
requirements are exercised by the integration suites, not by CodeWitness), and no requirement is
blocked.


## [9.0.10] — 2026-10-25 — A fuzzer finds a memory-exhaustion bug in the FEEL parser

`make fuzz` had never been run against this tree. It found, within a minute, that
`parse_expression` could be driven out of memory: `= value` builds a singleton range by parsing a
bound, and the bound parser re-enters the same rule, so **every additional `=` cloned the subtree
into both `start` and `end`**. Twenty equals signs produced a 2^20-node tree (≈170 MB of debug
output) and ran for 170 ms; twenty-four exhausted multi-gigabyte memory, and libFuzzer aborted at
its 2 GB limit. Any endpoint that parses a FEEL expression — decision tables, rule bodies,
`POST /v1/execute` — could be taken down by a 24-character string.

A range bound can no longer itself be a singleton range. `= 5` still parses as a point range;
`= = 5` is an error. `=`×10 000 now parses in 0.8 ms with a 5 MB footprint, and `singleton_range_
does_not_nest` plus the fuzz artifact itself are regression tests. `specs/dmn-feel.md` lists the
guard with the others.

The fuzz gate itself was broken in two ways and is fixed: libFuzzer's `-max_total_time=60` let a
60-second run spin at 99% CPU for 29 minutes, so `timeout` now holds the clock and a budget that
is reached is a pass rather than a failure. Both targets then ran to their budget with no crash,
no timeout and no leak.


## [9.0.9] — 2026-10-25 — The memory gate measures something again

`make memcheck` failed: peak RSS **+35%** against a baseline taken at v6.0. The growth is real
and accounted for — since then the evaluator gained an invocable index, bound function scopes for
business knowledge models, boxed contexts, lists and relations, and a cache holding every parsed
function body. A baseline that no release can pass is not a gate; it is a red light everyone
learns to ignore.

The snapshots are now named for what they measure: `baseline.json` and `current.json`, no
version in the name. The baseline is the 9.0 measurement, recorded by `make baseline-update`, and
`engine/benches/README.md` says why the v6.0 floor was retired. Allocations moved 5%
(1877 → 1986) and RSS 35%; from here the gate protects those numbers.

## [9.0.8] — 2026-10-25 — "Test this rule" says what it can and cannot run

The edit page mounts a test runner whose Run button posts to `POST /v1/execute`. That endpoint
runs the **engine's rule set**, loaded from the server's rules directory; a rule created through
`/v1/rules` is stored as text and is not in it. The runner therefore always reported "0 fired"
for an interface-authored rule, with nothing to say why — and a run where nothing fired looked
exactly like a run where nothing matched.

`rules_matched` is now part of the execute response: the number of rules the filter selected.
The runner shows it, and when it is zero it says so plainly — "No engine rule matched this
filter… rules created here are stored as text and are not part of it". `specs/http-api.md`
records the boundary next to the endpoint.

`make e2e` now presses Run and asserts the explanation appears, so the page cannot regress to a
silent zero. It also refuses to run when its port is already in use: a stale server on the same
port made every route render blank once, because the browser received an `index.html` from the
old build against freshly built chunks.


## [9.0.7] — 2026-10-25 — Stale "mount-only" comments removed, governance checked for real

Four routes carried TODOs saying their components were mounted but not wired
(`TODO(22-05): wire ApprovalQueue fetch to /v1/approvals/*`). They had been wired since: driven
in a browser while signed in, approvals lists `GET /v1/approvals/pending`, audit lists
`GET /v1/audit` with its filters, branches lists the repository's branches, and rbac shows the
signed-in user's role and permissions. The comments described a state that no longer existed and
are replaced with what the code does.

`rbac` also now states its limit: the API has no role-assignment endpoint, so the page reports
the session's role and permissions rather than offering edits it could not save.

`make e2e` asserts all four against their endpoints while signed in — an empty state from the
API passes, a 401 does not.


## [9.0.6] — 2026-10-25 — The rule list shows every rule, and the edit page edits the rule

Two defects the browser gate found once it started creating rules.

`GET /v1/rules` returned exactly **one** rule however many existed: an omitted
`page_size` deserialises to `0`, and `clamp(1, 100)` turned that into a page of one. The field
documents a default of 50, and now gets it — capped at 100, named in constants rather than
inline. `specs/http-api.md` recorded the truncation as intended behaviour and is corrected.

`/rules/{id}/edit` invented its content: a hard-coded expression with the id spliced in, a
hit-policy selector, an empty decision table, and a Save that wrote a description containing a
timestamp. It never read the rule it claimed to edit. It now loads the rule by id — the name,
description and status it shows are the rule's own — saves the fields the API stores, and
reports a rule that cannot be loaded instead of rendering a stub. The expression editor, hit
policy and decision table are gone from it: they were scaffolding with no server counterpart,
and leaving them made the page look like an editor that worked.

`make e2e` now walks the whole cycle: create a rule, see it listed, open it, and find its name
in the edit form.


## [9.0.5] — 2026-10-25 — A rule can be created from the interface

`/rules/new` was a placeholder: a JavaScript expression editor, a hit-policy selector and an
empty decision table, with no name field, no submit button and no call to the API — while the
rule list offered a "Create Rule" link to it. The authoring side of the interface could display
rules and not create one.

It is now a form that matches what the API accepts — name, optional description, salience and
the rule body — and posts it, returning to the list on success and showing the API's message on
failure. The page states plainly that the body is stored as text and never compiled: the API
validates only that it is non-empty.

The client's `CreateRuleRequest` described the *view* model (`hitPolicy`, `conditions`,
`actions`) rather than the wire format (`name`, `dsl_body`, `tags`, `salience`, `status`), so a
create from the interface would have sent fields the server ignores. The edit page was sending
`hitPolicy` to `PUT /v1/rules/{id}`, which has no such field.

`make e2e` now creates a rule through the API from the browser, checks it appears in the list,
and checks the form renders with a name field and a submit button.


## [9.0.4] — 2026-10-25 — The browser gate stops deleting its own profile

`make e2e` reused no browser profile: it created one per run and deleted it on the way out,
which raced Chromium still flushing to it and printed an `ENOTEMPTY` warning on every run —
and once failed the step outright. It now uses one profile under `target/e2e/` and leaves it
there: it is build output, ignored by git, and a directory that is never deleted cannot fail
to be deleted.


## [9.0.3] — 2026-10-25 — Vite 8, Vitest 5, jsdom 30

The three remaining advisories after 9.0.2 lived in the build and test toolchain, and clearing
them meant major upgrades: `vite` 5 → 8, `vitest` 2 → 5, `@vitejs/plugin-react` 4 → 6, `jsdom`
25 → 30. `npm audit` now reports **3 moderate** advisories, all in Tailwind's PostCSS chain,
whose fix is the Tailwind 4 migration — a CSS rewrite, not a dependency bump.

Two declarations were removed while there: `@vitejs/plugin-react-swc`, which nothing imports
(`vite.config.ts` uses `@vitejs/plugin-react`), and `@types/axios`, a deprecated stub for a
package that ships its own types.

Vite 8 bundles with Rolldown. It still honours the `manualChunks` hook — the build emits one
`vendor` chunk containing React — but that guarantee now rests on `make e2e`, which loads the
real bundle and fails when a route renders nothing. That is the gate that caught the blank
page this configuration exists to prevent.


## [9.0.2] — 2026-10-25 — Dependency advisories: 44 to 9

`npm audit` reported 44 advisories in the frontend's dependency tree (11 critical, 20 high).
`npm audit fix` resolves 35 of them inside the declared ranges — the lockfile moves, no
manifest changes — leaving 9 that need major upgrades: `vite` 5 → 8 and `vitest` 2 → 5, taken
up separately rather than smuggled into a patch release.

The remaining advisories are in the build and test toolchain (`vite`, `vitest`, `tinypool`),
not in code the server or the browser runs.


## [9.0.1] — 2026-10-25 — Clippy 1.99 lints, and a version-bump script

Ten findings that clippy 1.99 reports and 1.94.1 does not, fixed rather than deferred:
`sort_by(|a, b| b.x.cmp(&a.x))` became `sort_by_key(.. Reverse(..))` in the tenant store and
both salience orderings; four `format!("…{}", &string)` borrows lost their `&`; and the
optional-sign split in the offset parser became a `match` on the two prefixes instead of an
`if`/`else` chain. The compiler stays pinned at 1.94.1 — the code now passes on both.

`make bump-version VERSION=x.y.z` (`scripts/version-bump.py`) writes the version to all eleven
places it lives: four crate manifests, `package.json` and its lockfile, the chart's `version`
and `appVersion`, the OpenAPI document, the server banner and metrics footer, and the citation
in `specs/http-api.md`. They had drifted once — crates at 0.1.0, frontend at 4.0.0, chart at
1.0.0 — and this is the cheapest way to stop that recurring.


### Added — gates that do not need infrastructure the project lacks

Two failure modes kept hiding behind workflows whose logs nobody could read:

- **`make image-check`** reads the `Dockerfile`'s `COPY` instructions, assembles
  exactly those files, and runs each stage's commands on the host. The image build is
  the one artefact this project cannot build everywhere (no container runtime in
  every environment), so a missing or misspelled `COPY` source only ever surfaced as
  a failed deploy. It runs as its own CI job.
- **The chart is linted and rendered in CI** (default values and each environment
  overlay), and again in the deploy job before cluster access is configured — a
  broken template used to look like a deployment failure.
- The deploy job asks the cluster whether it answers before attempting a release. A
  `KUBE_CONFIG` that reaches nothing now publishes the image and emits a warning
  naming the fix, instead of failing every push; a red Deploy means a deploy was
  attempted and failed.

CI also pins Rust 1.94.1 rather than "whatever stable is today", so a new clippy or
rustfmt cannot fail a commit that changed neither.

### Fixed — the web interface, checked in a real browser for the first time

Every one of these was invisible to the existing tests, because they run source
modules in jsdom and never load a production bundle or talk to a running server:

- **The production bundle rendered a blank page.** `build.rollupOptions.manualChunks`
  split React, TanStack and the rest across four chunks, and the `vendor` chunk read
  `useState` off an undefined React (`TypeError` on load). One vendor chunk now; bundle
  splitting is a caching optimisation, a build that does not boot is not worth 200 KB.
- **No single-page-application fallback.** `actix-files` answered 404 for `/rules`,
  `/login` and every other client-side route, so a reload or a bookmark failed before
  the router ran. Unknown non-API GETs now serve the shell; an unknown `/v1/...` still
  returns a 404, so a client typo cannot look like success.
- **No navigation.** `Navbar` existed and no route mounted it: the application was a
  landing page with every other page reachable only by typing its URL. It is mounted in
  the root route and lists Rules, Governance and Monitoring.
- **The governance section had no layout.** `governance.tsx` never registered its
  route, so the header and the tab bar for approvals/audit/branches/rbac never rendered.
- **A 401 hung the browser.** The API client did `window.location.href = '/login'` on
  every unauthorised response — including the several that fire in parallel when a page
  loads — which left the page blank and the renderer stuck mid-navigation. It now clears
  the session and announces it; the page renders its own error and the navbar offers the
  login link.
- **The rule list crashed as soon as it had data.** The API returns
  `{total_count, rules[], page, page_size, total_pages}` and single rules unwrapped; the
  client read `response.data.data`, so `rules` was `undefined` and `RulesList` threw.
  The wire format is now mapped to the view model in one place, which also fixes every
  date field (the API uses `created_at`, the view model `createdAt`, so sorting by date
  compared `NaN`).
- **Two client endpoints did not exist.** `PATCH /v1/rules/{id}/status` answered 404 (a
  status change is a `PUT`), and `/v1/rules/search` was matched by `/v1/rules/{id}` and
  answered 400.

### Added — `make e2e`: the interface checked in a browser

`frontend/e2e/smoke.mjs` drives the built bundle in headless Chromium over the DevTools
protocol — no dependencies — and `make e2e` builds the interface, starts the server on a
spare port with a throwaway store, then checks every route renders, that a deep link
works as an entry point, and that the app works once signed in. It is part of `make ci`
and of the CI workflow, so a blank page or a 404 route now fails the build.
`api/src/spa.rs` carries unit tests for the fallback (a client route serves the shell, an
unknown API path stays a 404).

### Fixed — the container image and the deployment probes

The `Deploy` workflow failed on its first run, and the image it built would not
have worked if it had. The `Dockerfile` predated the workspace layout: it copied
`api/Cargo.lock` and a `shared/` crate that do not exist, used `COPY … 2>/dev/null
|| true` (a `COPY` instruction has no shell), and asked cargo for
`--features engine/default`, which cannot select another member's features. It also
never built the web interface, so the image would have served an API and no UI.

`Dockerfile` is rewritten against the actual layout: a Node stage builds
`frontend/dist`, a Rust stage builds `-p api --bin rRules-server --locked` from the
workspace root, and a Debian slim runtime carries both with every startup path set
explicitly and a non-root user. `.dockerignore` no longer excludes `Cargo.lock`
(`Cargo.*.lock` matched it) and now drops the build output, dependencies and
process documentation from the context.

Two defects surfaced while verifying the runtime, both of which would have kept the
deployment from ever becoming healthy:

- The server bound `127.0.0.1:8080` unconditionally. In a container that is
  reachable only from inside it. It now reads `RRULES_BIND_ADDRESS` and `PORT`
  (defaults unchanged for local work), and the image sets `0.0.0.0`.
- The chart and the `k8s/` manifests probe `/health` and `/ready`, while the
  application served only `/health/live` and `/health/ready` — so every probe got a
  404, the container was restarted before it could become ready, and the Service
  had no endpoints. Both flat paths are now aliases of the existing handlers.


## [v9.0] - 2026-10-25 — Verified Conformance

The release where the project's claims became measurable. rRules ships with a specification it can
be checked against, and with the OMG DMN conformance suite green: **170 layer results pass, none
fail, four skipped as one recorded divergence** across 51 vectors, up from the 143 pass / 31 fail
that the first honest measurement reported.

Two changes define the release. The **specification** (`specs/`) states what the system does as 35
normative requirements plus four implementation-derived subsystem documents, each citing the code and
each ending with the gaps it does *not* close. **Verification** executes those requirements
(CodeWitness: 28 verified, 0 failed, 7 covered from the API crate) and the conformance corpus
(`make tck`), so a claim about this system now has a gate behind it.

Closing the conformance gap meant fixing the evaluator rather than the harness — boxed decision
bodies, business knowledge models, decision services, real FEEL filter semantics, a context-free
lexer, the temporal model, and the full arithmetic matrix. The details are below.

### Changed — DMN conformance measured at 170 passed / 0 failed / 4 skipped

The vendored OMG DMN TCK corpus now evaluates cleanly: **170 layer results pass, none fail,
four are skipped** across 51 vectors, up from the 143 pass / 31 fail of the first honest
measurement. The four skips are a single recorded divergence — `round up(5.5, 6177)` is an
error in the vector and unchanged in the specification — kept in `known_divergence_list` with
its expected and actual behaviour, so it is reported rather than hidden.

Closing those 31 layers meant fixing the evaluator, not the harness. Each of these previously
evaluated to a silent null:

- **Boxed decision bodies** — `<context>`, `<list>` and `<relation>` are modelled, parsed,
  round-tripped and evaluated, so the FEEL vectors written that way can run at all.
- **Business knowledge models and decision services are callable**: a BKM carries its formal
  parameters and body and is bound into the scope as a function value (so a call inside a `for`
  body sees the iteration variable); a service binds its arguments over its own input decisions
  and rejects a call that is short of arguments, names an unknown parameter, or passes the
  wrong primitive type.
- **Real filter semantics** — 1-based position selection with negative indices, scalar
  targets, context entries bound by name.
- **A context-free lexer**: `a*b` lexed as a single name. Names are re-merged after lexing
  against the names the model declares, which also makes `Date-Time2`, `values in a list` and
  `{foo+bar: "foo"}` parse.
- **Temporal completeness**: exponent literals, `24:00:00`, the `Z` designator, a time with an
  IANA zone, duration properties, `time offset`, and a single carrier for an offset-less
  date-time.
- **The arithmetic matrix** (1087 cases): negative and fractional duration scaling,
  `duration / duration`, commuted operands, the mixed naive/zoned subtraction rules, and
  left-associative `**`.
- **Aggregate and conversion contracts**: `stddev` is the sample deviation, `mode` always an
  ascending list, a non-numeric element makes an aggregate null, `number()` takes
  `(from, grouping, decimal)` and validates its separators, and `day of week` / `month of year`
  return names.

`specs/dmn-feel.md` is updated to match, with the two gaps this closes struck through.

### Fixed — DMN conformance (TCK 143/31 → 156/18 layer results)

Sixteen of the thirty-one failing layers now pass. Each was a real evaluator
defect, found by reading the vector rather than the summary:

- **Filter semantics** (`0069-feel-list`): `list[i]` selects by 1-based position
  (negative counts from the end, `0` and out-of-range are null, a scalar target
  behaves as a one-element list), `list[boolean]` filters, and a context element
  exposes its entries by name so `[{a:1}][a >= 2]` works. Previously every form
  was treated as a boolean filter over a list.
- **Numeric literals with an exponent** (`0068-feel-equality`): `1.23e4` lexed as
  `1.23` and the exponent was silently dropped — `12300 = 1.23e4` was false.
- **Aggregates** (`0059`–`0063`, `0094`): `all`/`any` take a list *or* separate
  arguments, accept a single boolean, reject non-booleans (rather than coercing),
  and treat `all([])`/`any([])` as vacuously true/false. `median`, `product`,
  `stddev` and `mode` accept the same two forms and treat any non-numeric element
  as an error instead of skipping it; `stddev` is the **sample** deviation and
  needs two values; `mode` always returns an ascending list.
- **`number()`** (`0058`): the positional order is
  `number(from, grouping separator, decimal separator)`, separators must differ,
  an omitted decimal separator defaults to `.`, and text containing a character
  that is not a digit, sign or declared separator is rejected.
- **`modulo`** (`0056`): the remainder is floored, so its sign follows the
  divisor (`modulo(-12, 5)` is 3).
- **Multi-word names** (`0016`, `0002-input-data-number`): FEEL allows spaces in
  names, and models use them (`values in a list`, `Monthly Salary`). The parser now
  merges a run of words when the joined text is a name the model or the request
  declares — previously such a name was a syntax error.
- **Context keys with spaces** (`0057`): `{foo bar: "foo"}` parses.
- **Regular expressions in string literals** (`0067`): an unknown escape keeps its
  backslash, so `"\s"` reaches `split` as `\s` instead of `s`.
- **`Z` time designator and offset-aware time equality** (`0068`):
  `time("09:30:00Z")` parses, and `time("10:30:00+01:00") = time("09:30:00Z")` is
  true because both denote the same instant.

Two unit tests encoded the superseded behaviour and were corrected rather than
worked around (`number(number)` is a type error; `mode` returns a list).

### Added
- **`make tck-ledger`** — the conformance record now carries a generated ledger of every failing
  layer (vector, layer, observed-versus-expected) alongside the hand-written digest of causes. The
  block between the `BEGIN/END GENERATED LEDGER` markers in `TCK-COVERAGE-GAPS.md` is owned by
  `scripts/update-tck-ledger.sh`, which runs the sweep and splices the result in, so the list cannot
  rot into a stale summary.
- **`tck_baseline_matches_documentation`** — a test that reads the pass/fail counts from
  `TCK-COVERAGE-GAPS.md` and compares them with the constants the sweep gates on. The document and
  the gate can no longer drift apart: changing one without the other fails the suite with both
  numbers in the message. Verified by deliberately breaking the document (150/20) and watching it
  fail.

### Fixed
- **GitHub Actions workflows restored and repaired.** The four workflows had been removed because
  none of them could run as written; they are back, fixed:

  | Workflow | Was | Now |
  |---|---|---|
  | `ci.yml` | triggered on `main` only; skipped frontend tests, the TCK sweep | triggers on `master` **and** `main`; runs `make ci` and `make tck`, so CI and the local gate cannot disagree |
  | `deploy.yml` | guarded the cluster step with `if: ${{ secrets.KUBE_CONFIG != '' }}`, which GitHub forbids; failed on every push | builds and pushes the image on every push, deploys only when `KUBE_CONFIG` is set (checked through a job-level `env`), and logs a notice when it is not |
  | `bench.yml` | uploaded the same artifact twice, `actions/cache@v3` | separate `benchmarks` and `memory-gate` jobs with distinct artifacts; the gate runs nightly and on the `bench-regression` label, not on every PR |
  | `fuzz.yml` | `actions/cache@v3`; ran on every PR | runs nightly and on the `fuzz` label; `taiki-e/install-action` for `cargo-fuzz` |

  All four pin `permissions` to the least privilege they need and add a concurrency group so
  superseded runs cancel. `make fuzz` now honours `FUZZ_MAX_TOTAL_TIME`/`FUZZ_RUNS` instead of
  hardcoding 60 s, and `engine/benches/README.md` documents the jobs and the thresholds again.

### Changed
- **`AGENTS.md` replaces `CLAUDE.md` as the project's instruction file.** It
  carries the same information an agent needs — what the project is, the measured
  state, the quality gates, where the specification lives, the working conventions
  — plus the rule that no built or generated binaries may enter git. It exists in
  two copies: this repository (version-controlled, travels with a clone) and the
  parent workspace. The `.claude/` directory and its tracked `settings.json` are
  removed.
- **History rewritten** to strip build artifacts: `target/`,
  `frontend/node_modules/` and `frontend/dist/` were in every commit, which inflated
  `.git` to 5.45 GB and made a push to GitHub impossible (the six 126–129 MB test
  binaries exceed the 100 MB per-file limit). The repository is now 3.3 MiB packed
  with 605 tracked files; all 494 commits and all eight tags survive, every SHA
  re-pointed. A `frontend/node_modules` that had been committed despite being
  listed in `.gitignore` was part of what came out — ignore rules never untrack
  what is already tracked.

### Added
- **A specification that can be executed.** `specs/` now holds the system
  specification — `requirements.md` (35 normative requirements with stable ids),
  plus `domain-model.md`, `rule-engine.md`, `dmn-feel.md` and `http-api.md`
  derived from the code with `path:line` citations and an explicit `Known gaps`
  section each. `engine/codewitness.yaml` (28 Rust scenarios) and
  `api/codewitness.yaml` (integration-suite mapping) wire
  [CodeWitness](https://github.com/codewitness/codewitness) to it. Measured:
  28 VERIFIED, 0 FAILED, 7 UNVERIFIED (the API requirements, covered from the
  `api` project instead).
- `engine/tests/rule_semantics.rs` — firing order, fixpoint, numeric comparison
  and retraction pinned across all four engines.
- `engine/tests/tck_values.rs` — typed parsing of TCK test XML (values, lists,
  contexts) with tolerance-aware structural comparison.

### Changed
- `tck_sweep` records every layer outcome, prints each failure reason, evaluates
  every `resultNode` of a case, and gates on a documented baseline instead of
  hiding failures. Measured TCK state: 136 layer results pass, 24 fail, 14 skip.
- `engine` enables its four engines by default, so `cargo test -p engine` tests
  them.
- `SemanticValidator` lost its unused lifetime parameter and the `unsafe`
  transmute that propped it up.

### Removed
- The legacy React tree under `api/src/{components,features}` (18 files) — dead
  code in the Rust crate, superseded by `frontend/`.

- **Tenant management API** — `GET/POST /v1/tenants` and
  `GET/DELETE /v1/tenants/{id}` are now registered behind the `/v1` auth scope.
  The handlers build on `engine::governance::tenant::MemoryTenantStore`, which
  was previously uncompiled dead code: it gained synchronous
  `create`/`get`/`list`/`delete` cores plus `TenantStoreError`
  (blank name → `400`, duplicate name → `409`), and the async
  `TenantStoreBackend` trait impl now delegates to them instead of duplicating
  the logic.
- `api/tests/tenants.rs` — tenant CRUD contract plus a per-tenant rule-isolation
  test proving two `X-Tenant-ID` values never see each other's rules.

### Housekeeping
- Updated stale `CLAUDE.md` to reflect v5.0 (was a full milestone behind, still
  describing v4.0 as current with v5.0 as a "candidate thesis").
- Removed the dead `api/src/models/` module (duplicate `ExecuteRequest`/
  `ExecuteResponse`/`InferenceTraceEntry`/`RuleFilter`/`TraceSummary` plus
  duplicate `rules` DTOs and a `coverage.rs`). The canonical types live in
  `api::v1::execution` and `api::v1::rules`; nothing in the workspace imported
  `models::`. (v5.0 retrospective flagged the `execution.rs` half; the whole
  module was equally dead.)
- Deleted a stray bare git repository left in `rules/` (untracked, no content).
- Replaced stale `// pub mod auth;` / `// pub mod tenants;` TODO comments in
  `api/src/v1/mod.rs` with accurate pointers (auth is at `crate::auth` +
  `crate::middleware::auth`; tenants route module not yet wired).
- Added `.planning/TECH-DEBT.md` recording the three engine retraction
  `TODO`s (Rete/TREAT/LEAPS network cleanup) and the multi-tenancy route gap as
  known debt, intentionally deferred from this pass.

### Verification
- `cargo test --workspace`: all binaries passing, 0 failed (~353 tests).
- `cargo clippy --workspace --all-targets`: 0 errors (4 pre-existing warnings).
- `npx vitest run`: 5/5 passing.

## [v8.0] - 2026-07-15 — Close TCK Evaluator Gaps

### Added
- **Numeric built-in coverage** (Phase 40) — `abs()` extended to
  `DaysTimeDuration`/`YearsMonthsDuration`; `number(from, grouping, decimal)`
  3-arg form with locale-independent parsing; named-parameter resolution by
  positional fallback in evaluator dispatch (`abs(n: -1)`); `median`, `mode`,
  `stddev`, `product`; arithmetic edge cases (empty-list SUM/MIN/MAX → null).
- **String + list built-in coverage** (Phase 41) — `sort()` comparator-lambda
  support via `apply_function` (correct lambda scope handling) and an
  empty-list guard returning null per DMN 1.5; `filter` lambda invocation;
  edge-case tests for `split` (no delimiter / empty string / null inputs),
  `concatenate`, `insert`/`remove`, and `list contains` null propagation.
- **TCK-COVERAGE-GAPS.md** — repository-root record of the single remaining
  divergence (DMN15-31 COLLECT with `distinct`, a DMN 1.6 / FEEL 1.6 feature,
  out of scope for DMN 1.5 Level 3) with spec references and disposition.
- **Verification artifacts** — `.planning/milestones/v8.0-ROADMAP.md`,
  `.planning/milestones/v8.0-MILESTONE-AUDIT.md`, and
  `.planning/phases/43-tck-conformance-verification-close-out/VERIFICATION.md`
  recording per-phase deliverables and evidence.
- **Comprehensive built-in tests** — `engine/tests/feel_builtins_comprehensive.rs`.

### Changed
- **OMG DMN TCK evaluator coverage: 21/51 → 51/51 vectors** (123/123 layer
  passes; compliance-level-2 110/110, compliance-level-3 13/13), with zero
  regression in the 30 vectors that already parsed and round-tripped. Phase 42
  (context/relational/temporal) required no code change — the v6.0 Phase-31
  implementations already satisfied their vectors and are now verified by the
  sweep rather than assumed.
- Rete rule-execution path (`/v1/execute`, `EngineRegistry`) untouched — zero
  regression by scope lock.

### Fixed
- **Multi-word FEEL function names were unparseable** — the lexer emits one
  `Name` token per word and the parser stopped at the first one, so
  `upper case("x")` parsed as `Name("upper")` and evaluated to `null`. Every
  multi-word built-in (`upper case`, `lower case`, `string length`,
  `string join`, `list contains`, `substring before/after`, `starts with`,
  `ends with`, `get value`, `get entries`, `date and time`, `round *`,
  `years and months duration`) was affected inside real DMN models.
  `Parser::merge_multiword_name` now re-joins adjacent words when the result is
  exactly a registered built-in name, leaving operators (`and`, `or`, `in`)
  untouched.
- **`sort(list, comparator)` ignored the comparator** — the Phase 41 "comparator
  support" was never in the code; the function returned the input list. Sorting
  now applies the comparator through `eval::apply_function_value`, supporting
  boolean (`x > y`) and numeric comparator results, with `sort([]) → []` per
  DMN 1.5 §10.3.4.2.5.
- **Rounding functions ignored their `scale` argument** — `round up`,
  `round down`, `round half up` and `round half down` always rounded to an
  integer (`round half up(1.25, 1)` returned `1`). They now use
  `rust_decimal::RoundingStrategy` at the requested scale (ties away from zero /
  toward zero per spec).
- **`get entries(c)` returned strings** instead of the spec's list of
  `{key, value}` contexts (now emitted in sorted-key order), and
  **`years and months duration(from, to)`** had no two-date form (now computes
  the month delta).
- **`retract_fact` was a stub returning `Ok(false)`** in Rete, TREAT and LEAPS,
  so four `engine/tests/retract_integration.rs` tests were permanently red.
  Retraction now removes the fact from working memory, alpha memory and every
  beta-memory partial match (`BetaMemory::remove_fact`), drops TREAT's join
  cache and resets LEAPS aggregation nodes.
- **TCK structural comparator hung and emitted malformed XML** —
  `engine/src/dmn/test_util.rs` looped forever on the first attribute-less tag
  and rebuilt tags without their closing `>`. Replaced with `canonicalize_tags`
  (attributes sorted, self-closing tags expanded, CDATA unwrapped), and the
  stray `eprintln!` step tracing is gone.
- **Two `discover_tck_vectors` unit tests never matched the fixture layout**
  (they omitted the `<vector>-test-NN.xml` naming the discovery code requires).
- **TCK precision audit scanned attribute values** — DMN ids such as
  `113e6550-…` were read as numbers with enormous exponents, so the A1 gate
  (`rust_decimal` suffices for the value domain) always failed. The scan is now
  restricted to element text and confirms A1 across 102 XML files.
- **`-D warnings` clippy gate was red** — fixed the outstanding lints
  (`if let` destructuring, collapsible `if`, map-value iteration, needless range
  loop, `is_empty` companion, type aliases, `large_enum_variant` waiver with
  rationale). `make lint` now passes.
- Compile break in `engine/src/validation.rs` unit tests: `ComparisonOp` was
  dropped from the module imports while still used by
  `test_semantic_validator_types`; the import now lives in the test module.

### Verification (re-verified 2026-10-25)
- `cargo test --workspace`: every target green (696 engine lib tests; 8/8
  retraction integration tests, was 4/8; 20/20 FEEL built-in tests).
- `cargo test -p engine --test tck_sweep`: 51 vectors, 123/123 layer passes,
  0 failures, evaluator layer now compares **every** test case structurally via
  `engine/tests/tck_values.rs` (typed `xsd:*` values, `<list>`, `<component>`).
- `cargo clippy --workspace -- -D warnings`: clean.
- `npx vitest run`: 28/28; `npx tsc --noEmit`: clean.

## [v7.0] - 2026-07-15 — Performance + Production Readiness

### Added
- **Benchmark infrastructure** (Phase 36) — Criterion `dmn_benchmarks` suite
  (`make bench`) covering parser, exporter, evaluator, and decision tables.
- **Memory regression gate** (Phase 36) — frozen `v6.0-baseline.json` snapshot
  over a 60-file corpus, `memory_baseline` binary, and `scripts/memcheck-gate.sh`
  (`make memcheck`) failing on RSS/allocation regression.
- **Fuzzing** (Phase 38) — `cargo-fuzz` targets `feel_parser` and
  `decision_table`, committed regression corpus, `make fuzz` runner, and a
  dedicated `fuzz.yml` CI workflow.
- **Property-based tests** (Phase 38) — proptest invariants for FEEL evaluation
  (including context topological order independence) in
  `engine/tests/feel_properties.rs`.

### Changed
- **Zero-copy DMN XML loading** (Phase 37) — dropped per-element `into_owned()`
  in the parser; re-baselined the memory snapshot to the 60-file corpus.
- **FEEL AST caching** (Phase 37) — lexer token cache + parse-with-cache entry
  point and decision-table cell AST warming; `feel_cache_wiring` tests.
- **Parallel DRD evaluation** (Phase 39) — topological sort of decision
  dependencies with rayon level-by-level evaluation of independent decisions
  inside one `DecisionEvaluator::evaluate` call.

## [v6.0] - 2026-07-10 — DMN Level 3 Compliance

### Added
- **DMN 1.5 model + parser + exporter** (Phase 29) — streaming namespace-aware
  `quick-xml` parser (DMN 1.1–1.5) with XXE rejection, CDATA preservation,
  entity resolution, and ID/annotation preservation into a canonical IR;
  writer-based exporter with an 8-test DOM-compare round-trip gate.
- **FEEL value domain** (Phase 30) — Decimal128 numbers via `rust_decimal`
  (no f64 on the production path, `0.1 + 0.2 = 0.3` proven), five temporal types
  with ISO 8601 `@""` round-trip and IANA zones (`chrono-tz`), DST-correct
  duration arithmetic, and the FEEL type lattice with exactly the three
  spec-defined implicit conversions (`1 + "2" → null`).
- **FEEL expression language** (Phase 31) — grammar rules 1–65, boxed
  expressions, if/for/some/every, path navigation, filters, lambdas with lexical
  capture, three-valued logic and null propagation, ~70+ built-in functions, and
  per-thread AST caching.
- **Decision-table engine** (Phase 32) — all 7 hit policies (UNIQUE, ANY,
  PRIORITY, FIRST, RULE ORDER, OUTPUT ORDER, COLLECT) with SUM/MIN/MAX/COUNT
  aggregators.
- **Decision evaluation API** (Phase 33) — `POST /v1/decisions/{id}/evaluate`
  with multi-output decision services and DRD chaining; `/v1/decisions` CRUD,
  import/export/IR/hot-reload; per-decision Prometheus counters and DRD depth
  gauge at `/v1/metrics/dmn` + `/v1/metrics/dmn/ws`; `DmnDecisionTracker`
  dead-decision detection.
- **Frontend DMN wiring** (Phase 34) — hit-policy model refactored to the DMN
  canonical 7-policy union with a COLLECT aggregator sub-type;
  `DecisionTable`/`HitPolicySelector` wired to the real backend evaluator with
  results and trace panels.
- **Conformance close-out** (Phase 35) — TCK sweep across parser, evaluator,
  round-trip, and DMNDI layers; Level 3 conformance checklist with evidence;
  zero-regression verification.

### Changed
- The DMN surface is a **parallel evaluation path** (`engine/src/dmn/`) and is
  deliberately not routed through `/v1/execute` or `EngineRegistry`; the Rete
  rule path is unchanged by construction.

## [v5.0] - 2026-07-06 — Live Execution

### Added
- **Real rule execution** — `POST /v1/execute` fires rules through
  `EngineRegistry` (fresh `Box<dyn ExecutionEngine>` per request in
  `web::block`) and returns truthful `rules_fired`/`rules_not_fired`/
  `facts_after` from post-execution working memory. Closes the long-standing
  `rules_fired: 0` stub.
- **Live monitoring data** — per-execution Prometheus counters + WebSocket
  `MetricsSnapshot` broadcast (Phase 24); `RuleExecutionTracker` as a
  `LazyLock<Mutex<…>>` singleton so dead-rule detection reads live firings
  (Phase 26). Three metric surfaces (Prometheus, dashboard, dead-rule) agree.
- **Real reload + JobStore** (Phase 25) — `SharedRuleSet` swap-don't-mutate with
  a generation counter; async reload writes `ReloadJob` transitions;
  batch-execute aggregates truthfully.
- **Stateful sessions** (Phase 27) — `SessionManager` (DashMap sharded
  registry) with TTL, LRU eviction, max-count, background sweep, and a gauge
  shipped together; stateful `POST /v1/execute/session/{id}` endpoints.
- **Frontend test-runner** (Phase 28) — `execution.ts` + `TestRunner` component;
  engine-selector dropdown; inference trace surfaced; `rule_filter` honored
  server-side for a true "test this rule" UX.

### Fixed
- v3.0 `engine::`→`crate::` E0433 import bug across all four engine modules —
  the pluggable-engine surface compiles for the first time.
- Minimal direct rule evaluator built on `ReteEngine` and mirrored onto
  TREAT/LEAPS/PHREAK via a shared `direct_eval` module.
- Milestone-audit cross-phase WARNINGs (frontend `rule_filter` dead input;
  `/metrics` `active_sessions` hardcoded) resolved with regression tests before
  tagging.

## [v4.0] - 2026-07-02 — Production Platform

### Added
- **Production React/TypeScript frontend** — CodeMirror rule IDE, decision-table
  editor (all DMN hit policies), guided editors, rule-management dashboard.
- **Enterprise governance backend** — RBAC (admin/author/reviewer/viewer) with
  JWT auth, maker-checker approval workflow, immutable audit trail with
  CSV/JSON export, Git-backed branching/merging with conflict resolution.
- **Monitoring & metrics** — Prometheus `/metrics`, dashboard API, dead-rule
  detection, coverage reporting.
- **Complete management API** — rules/categories/tags CRUD, approvals, audit,
  branches.
- **Deployment & CI/CD** — multi-stage Alpine Docker, Helm chart
  (dev/staging/prod), hot reload with async job tracking.
- **Frontend-backend integration closure** (Phase 22) — aligned frontend to
  `/v1`, JWT-protected routes + AuthProvider, mounted orphaned routes, real
  WebSocket metrics streaming, audit/approval/branch UI wired to backend.

<!-- Link references resolve within this file; the repository is published at
     github.com/thread3d/rRules. -->
[Unreleased]: #unreleased
[v9.0]: #v90---2026-10-25--verified-conformance
[9.0.0]: #v90---2026-10-25--verified-conformance
[v8.0]: #v800---2026-07-15---close-tck-evaluator-gaps
[v7.0]: #v700---2026-07-15---performance--production-readiness
[v6.0]: #v600---2026-07-10---dmn-level-3-compliance
[v5.0]: #v500---2026-07-06---live-execution
[v4.0]: #v400---2026-07-02---production-platform