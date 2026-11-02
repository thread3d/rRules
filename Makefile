.PHONY: build test lint fmt fmt-check tck tck-ledger spec-citations docs-snippets readme singleton-tests frontend-types verification-cost preflight audit deploy-env verify e2e image-check chart-lint db-test bump-version docker-build run clean frontend-dev frontend-build frontend-test ci help bench memcheck baseline-update fuzz fuzz-build fuzz-invocation

# Build the Rust workspace
build:
	cargo build --workspace

# Run all tests (engine + api)
test:
	cargo test --workspace

# Run clippy linter with warnings as errors
# `--all-targets` covers test and bench code too: without it, unit tests inside modules,
# integration tests and benchmarks were never linted, and 60 findings had accumulated there
# — including a test that asserted nothing (`x >= 0 || y >= 0` on unsigned counters).
lint:
	cargo clippy --workspace --all-targets -- -D warnings
	@# The `postgres` feature is off by default, so a plain workspace lint never compiles it.
	cargo clippy -p engine_state --features postgres --all-targets -- -D warnings
	cargo clippy -p api --features postgres --all-targets -- -D warnings
	cargo clippy --manifest-path db_tests/Cargo.toml --all-targets -- -D warnings

# Format code with rustfmt
fmt:
	cargo fmt --all

# Fail if any file is not rustfmt-clean (CI gate)
fmt-check:
	cargo fmt --all -- --check

# Build Docker image using existing Dockerfile
docker-build:
	docker build -t rrules-server:latest .

# Every `path:line` citation in specs/ must still resolve: the specification is only
# evidence while its references point at real code.
spec-citations:
	./scripts/check-spec-citations.py --strict

# Every variable the deployment sets must be one the server reads, and every variable whose
# default is unsafe must be set by each deployment path.
deploy-env:
	./scripts/check-deploy-env.py

# The PostgreSQL store, against a real PostgreSQL that pg-embed downloads and runs in userspace: no
# Docker, no root, no preinstalled server. Ignored by default because the first run downloads ~30 MB,
# and a gate every `cargo test` pays for is a gate people stop running.
#
# The binary cache honours XDG_CACHE_HOME, which this sandbox's read-only $HOME makes necessary:
# `XDG_CACHE_HOME=$PWD/.db-cache make db-test`.
db-test:
	@# Two suites: the store itself, and the API's decision endpoints persisting through it.
	@XDG_CACHE_HOME="$${XDG_CACHE_HOME:-$$PWD/.db-cache}" \
		cargo test -p engine_state --features postgres --test postgres_store -- --ignored --test-threads=1
	@# The API's database tests live in their own crate so `pg-embed` is not an `api` dev-dependency:
	@# see `db_tests/Cargo.toml` for why that mattered.
	@XDG_CACHE_HOME="$${XDG_CACHE_HOME:-$$PWD/.db-cache}" \
		cargo test --manifest-path db_tests/Cargo.toml -- --ignored --test-threads=1

# Every dependency advisory must be either fixed or accounted for in `.cargo/audit.toml`, whose entries
# carry their reason and are checked for staleness by the tool itself. The advisory database needs the
# network on first run; it is cached afterwards.
audit:
	@command -v cargo-audit >/dev/null 2>&1 || { \
		echo "audit: cargo-audit is not installed." >&2; \
		echo "       Install it with: cargo install cargo-audit --locked" >&2; \
		echo "       The gate is enforced in CI, which installs it for the job." >&2; \
		exit 1; }
	cargo audit

# Measure the machine before trusting a gate: the 862 GB `target/` made `make verify` report BLOCKED
# twice for reasons that had nothing to do with the code.
preflight:
	./scripts/preflight.py

# The interface's declared response shapes must match the keys the API sends: a type that invents a field
# makes every caller silently wrong, and no compiler can see it.
frontend-types:
	./scripts/check-frontend-types.py

# The requirement verification builds and measures `api` and `engine`; a heavy dev-dependency there
# starves its budget and reports every requirement BLOCKED (9.0.45-9.0.57). This keeps that true.
verification-cost:
	./scripts/check-verification-cost.py

# A test that observes a process-wide tracker must serialise itself: SYS-003 says the trackers are
# resettable, and resettability buys nothing when two tests run at once.
singleton-tests:
	./scripts/check-singleton-tests.py

# The two fuzz recipes must give cargo-fuzz the same `--target`, before the `--` that hands the rest to
# libFuzzer: cargo-fuzz's own default is the triple it was compiled for, not the host it runs on, and
# the prebuilt musl binary makes that musl — no std, no sanitizer. `make fuzz` had it after the `--`.
fuzz-invocation:
	./scripts/check-fuzz-invocation.py

# The commands the README tells a reader to run must exist: a target, a binary, a crate, a directory.
readme:
	./scripts/check-readme.py

# Every Rust block in the user guide must compile: a guide that shows code is making a claim
# about the public API, and a snippet the reader copies should build.
docs-snippets:
	./scripts/check-doc-snippets.py

# Write a version to every place it lives: the crate manifests, package.json and its
# lockfile, the chart, the OpenAPI document, the server banner and metrics footer, and
# the spec that cites it. Usage: make bump-version VERSION=9.0.2
bump-version:
	@test -n "$(VERSION)" || { echo "usage: make bump-version VERSION=x.y.z" >&2; exit 1; }
	python3 scripts/version-bump.py "$(VERSION)"
	@# The workspace crates are path dependencies, so their versions live in
	@# Cargo.lock too. Regenerating it here keeps `--locked` builds working; a bump
	@# that skips this breaks every gate that builds with `--locked`. Deliberately
	@# not `--offline`: resolving a lock that needs a crate this machine has never
	@# fetched fails offline, and the failure is easy to miss.
	cargo check --workspace --quiet
	@# `engine/fuzz` is a separate workspace with its own tracked lockfile, so it has to
	@# be resolved too — otherwise it drifts until someone runs the fuzzer.
	cargo metadata --manifest-path engine/fuzz/Cargo.toml --format-version 1 > /dev/null
	@echo "Cargo.lock files refreshed for $(VERSION)"

# Check the Dockerfile's build logic without a container runtime: read its COPY
# instructions, assemble those files, and run each stage's commands on the host.
# The image build is the one gate that cannot run everywhere, and its failures
# otherwise only surface in a deploy.
image-check:
	./scripts/image-context-check.sh

# The chart's gate, exactly as CI runs it (`.github/workflows/ci.yml`, job `chart`): lint
# the chart, render the default values, then render each environment overlay. CI installs
# Helm v3.14.0 in that job and nowhere else, so this target reports the version it found —
# a local Helm that lints differently from CI is then visible instead of silent.
#
# Deliberately not part of `ci`: the job that runs `make ci` has no Helm, and a gate that
# cannot run on the machine that runs it is worse than no gate. Pass HELM= to override.
#
# Helm's own cache/config/data directories are redirected into the tree when $HOME is not
# writable (this sandbox's is read-only); `.helm/` is gitignored.
CHART ?= helm/rrules-server
HELM ?= $(firstword $(shell command -v helm 2>/dev/null) ../.tools/linux-amd64/helm)

chart-lint:
	@set -e; \
	helm_bin="$(HELM)"; \
	if [ -z "$$helm_bin" ] || [ ! -x "$$helm_bin" ]; then \
		echo "helm not found: not on PATH and not at ../.tools/linux-amd64/helm." >&2; \
		echo "CI pins v3.14.0; install it or pass HELM=/path/to/helm." >&2; \
		exit 1; \
	fi; \
	case "$$helm_bin" in /*) ;; *) helm_bin="$$PWD/$$helm_bin" ;; esac; \
	if [ ! -w "$${HOME:-/nonexistent}" ]; then \
		export HELM_CACHE_HOME="$$PWD/.helm/cache" \
			HELM_CONFIG_HOME="$$PWD/.helm/config" \
			HELM_DATA_HOME="$$PWD/.helm/data"; \
	fi; \
	echo "helm: $$("$$helm_bin" version --short) (CI pins v3.14.0)"; \
	echo "==> lint $(CHART)"; \
	"$$helm_bin" lint "$(CHART)"; \
	echo "==> render $(CHART) with the default values"; \
	"$$helm_bin" template rrules "$(CHART)" > /dev/null; \
	for overlay in $(CHART)/values-*.yaml; do \
		[ -e "$$overlay" ] || continue; \
		echo "==> render $(CHART) with $$overlay"; \
		"$$helm_bin" template rrules "$(CHART)" -f "$$overlay" > /dev/null; \
	done; \
	echo "chart lint complete"

# Run the application locally (dev mode)
run:
	cargo run --bin rRules-server

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/

# Run frontend dev server
frontend-dev:
	cd frontend && npm run dev

# Build frontend for production
frontend-build:
	cd frontend && npm ci && npm run build

# Run the DMN TCK sweep and print the per-layer report. Green at the documented
# baseline (143 pass / 31 fail / 0 skip); fails on any regression.
tck:
	cargo test -p engine --test tck_sweep -- --nocapture

# Regenerate the failure ledger inside TCK-COVERAGE-GAPS.md from a real sweep.
# Run after closing a gap (and bump the baseline constants) or after accepting one.
tck-ledger:
	./scripts/update-tck-ledger.sh

# Verify the specification with CodeWitness (requires the `codewitness` CLI).
# Reports land under engine/.codewitness/runs/ and api/.codewitness/runs/.
verify:
	@# The tool is not published, so this gate is local-only unless it is installed; say so
	@# rather than failing with "command not found" from inside a recipe.
	@command -v codewitness >/dev/null 2>&1 || { \
		echo "verify: codewitness is not installed." >&2; \
		echo "        The requirement verification cannot run here; CI runs it only when" >&2; \
		echo "        CODEWITNESS_JAR_B64 is configured. See AGENTS.md." >&2; \
		exit 1; }
	@# CodeWitness builds the crate itself and passes `--offline` to cargo, so its
	@# resolution relies on a crates.io index snapshot. A `Cargo.lock` pinning a version
	@# newer than that snapshot fails to resolve and the whole run is reported BLOCKED —
	@# a tooling limit, not a code defect. `CARGO_NET_OFFLINE=false` lets the index refresh.
	@# CodeWitness replaces its child environment with an allow-list that forwards `HOME` but not
	@# `CARGO_HOME`, so its cargo resolves against `$HOME/.cargo` — read-only in this sandbox, and it
	@# passes `--offline`, so the cache it finds must already be populated. Give it a HOME inside the
	@# tree whose `.cargo` is the workspace's own cache. (Never with git in the same recipe: git reads
	@# `$HOME/.gitconfig` and would lose the author identity.)
	@mkdir -p "$${VERIFY_HOME:-$$PWD/.verify-home}"; \
		[ -e "$${VERIFY_HOME:-$$PWD/.verify-home}/.cargo" ] || \
		ln -s "$$PWD/.cargo-home" "$${VERIFY_HOME:-$$PWD/.verify-home}/.cargo"
	@# CodeWitness is a jar installed into a HOME; this gate points HOME at .verify-home, so the jar has to be
	@# reachable there. If it is missing, copy it from the system installation rather than failing with
	@# "Unable to access jarfile" — a message that says nothing about what to do.
	@if [ ! -f "$${VERIFY_HOME:-$$PWD/.verify-home}/.local/lib/codewitness/codewitness.jar" ]; then \
		mkdir -p "$${VERIFY_HOME:-$$PWD/.verify-home}/.local/lib/codewitness"; \
		if [ -f "$$HOME/.local/lib/codewitness/codewitness.jar" ]; then \
			cp "$$HOME/.local/lib/codewitness/codewitness.jar" \
				"$${VERIFY_HOME:-$$PWD/.verify-home}/.local/lib/codewitness/"; \
		else \
			echo "verify: the CodeWitness jar is not installed; see AGENTS.md" >&2; exit 1; \
		fi; \
	fi
	HOME="$${VERIFY_HOME:-$$PWD/.verify-home}" CARGO_NET_OFFLINE=false \
		codewitness verify engine --requirements specs/requirements.md --in-place --no-mutation
	HOME="$${VERIFY_HOME:-$$PWD/.verify-home}" CARGO_NET_OFFLINE=false \
		codewitness verify api --requirements specs/requirements.md --config api/codewitness.yaml \
		--in-place --no-mutation
	@# Neither crate can execute the other's requirements, so each reports the other half
	@# as UNVERIFIED. The union is the result a reader wants, and a requirement that
	@# neither half verifies is a gap rather than a division of labour.
	@./scripts/verify-summary.py

# Run the frontend test suite
frontend-test:
	cd frontend && npx vitest run

# Drive the built interface in a real browser against a real server: every route
# must render, deep links must work, and the app must work once signed in.
# Catches what jsdom unit tests cannot (a bundle that throws, a route the server 404s).
e2e:
	./scripts/e2e-smoke.sh

# Full CI pipeline (build + test + lint + format check + frontend)
ci: preflight build test lint fmt-check frontend-build frontend-test spec-citations docs-snippets readme singleton-tests fuzz-invocation frontend-types verification-cost deploy-env e2e
	@echo "CI pipeline complete"

# Run DMN subsystem benchmarks (Criterion, Phase 36+)
bench:
	cargo bench --bench dmn_benchmarks -p engine

# Run memory baseline comparison against the frozen v6.0 snapshot (regression
# gate, PERF-04). Writes a CURRENT snapshot (engine/benches/snapshots/current.json)
# and compares it against the committed baseline.json via the gate script. The baselines
# are named by release: baseline.json is the 9.0 measurement, taken when the evaluator
# grew an invocable index and bound function scopes and no longer matched the v6.0 one.
# Exits non-zero on RSS/allocation regression beyond threshold.
memcheck:
	@echo "Running memory baseline check..."
	@if [ ! -f engine/benches/snapshots/baseline.json ]; then \
		echo "ERROR: frozen v6.0 baseline snapshot not found at engine/benches/snapshots/baseline.json"; \
		echo "Run 'make baseline-update' to capture a baseline, then commit it."; \
		exit 1; \
	fi
	cargo run -p engine --bin memory_baseline
	@./scripts/memcheck-gate.sh

# Refresh the FROZEN v6.0 baseline snapshot (PERF-04). Use only when intentionally
# accepting new memory numbers (e.g. after an optimization that legitimately
# changes the baseline). Commit the resulting baseline.json.
baseline-update:
	@echo "Refreshing frozen v6.0 baseline snapshot..."
	cargo run -p engine --bin memory_baseline -- --update-baseline
	@echo "Frozen baseline refreshed. Commit engine/benches/snapshots/baseline.json."

# `cargo fuzz` builds a second workspace (`engine/fuzz`) and pulls registry crates for it, so it
# needs a writable CARGO_HOME. On a machine whose $HOME is read-only (this sandbox's is) that
# build fails outright with `failed to open .../registry/cache/...: Read-only file system`, so
# fall back to the gitignored `.cargo-home/` the workspace already carries. An explicit
# CARGO_HOME from the environment still wins; where $HOME is writable this is cargo's default.
CARGO_HOME ?= $(if $(shell test -w "$$HOME" 2>/dev/null && echo y),$(HOME)/.cargo,$(CURDIR)/.cargo-home)
# Exported, not just defined: `preflight` is a script that decides whether the machine is trustworthy by
# looking at `$CARGO_HOME`, and a make variable it cannot see is a make variable that does not exist to
# it — so `make ci` in a clean shell here failed at its first gate on the very condition it had already
# accounted for, telling the reader to do what the Makefile was doing.
export CARGO_HOME

# The triple cargo-fuzz builds and runs for — and it has to be given to cargo-fuzz **itself**.
#
# `cargo fuzz` defaults `--target` to the triple *it* was compiled for, not the host it runs on
# (`utils::default_target()` is `current_platform::CURRENT_PLATFORM`, a compile-time constant). The
# prebuilt musl binary that `taiki-e/install-action` installs therefore asks cargo for
# `x86_64-unknown-linux-musl`, which fails twice on a runner: that target's std is not installed, and
# a sanitizer build refuses a statically linked libc ("sanitizer is incompatible with statically
# linked libc"). That is what failed the nightly Fuzz job on f84fcc1, after `fuzz-build` had
# succeeded — because `fuzz-build` passed `--target` to cargo-fuzz and `fuzz` passed it to libFuzzer,
# which ignored it. One variable, passed in one place, before the `--` that hands the rest to
# libFuzzer; `scripts/check-fuzz-invocation.py` fails the build if they ever disagree again.
FUZZ_TARGET_TRIPLE ?= $(or $(shell rustc -vV 2>/dev/null | sed -n 's/^host: //p'),x86_64-unknown-linux-gnu)

# Build cargo-fuzz targets (Phase 38+; requires nightly toolchain)
fuzz-build:
	cd engine && CARGO_HOME="$(CARGO_HOME)" cargo +nightly fuzz build --target $(FUZZ_TARGET_TRIPLE)

# Run DMN fuzz targets (Phase 38+; requires nightly + cargo-fuzz).
# Usage: make fuzz TARGET=decision_table FUZZ_MAX_TOTAL_TIME=300
#        (no TARGET runs every target; defaults bound each run to 60 s)
#
# The budget is enforced with `timeout`, not only with libFuzzer's `-max_total_time`:
# that flag's own accounting let a 60-second run spin at full CPU for 29 minutes, so
# the OS now holds the clock. Exit 124/130 means the budget was reached, which is how
# a bounded run is supposed to end; any other status is a crash or a build failure.
FUZZ_MAX_TOTAL_TIME ?= 60
FUZZ_RUNS ?= 600000
fuzz:
	@set -e; \
	run() { \
		echo "Fuzzing $$1 ($(FUZZ_RUNS) runs, max $(FUZZ_MAX_TOTAL_TIME) s)..."; \
		status=0; \
		(cd engine && CARGO_HOME="$(CARGO_HOME)" timeout --signal=INT --kill-after=30 $(FUZZ_MAX_TOTAL_TIME) \
			cargo +nightly fuzz run --target $(FUZZ_TARGET_TRIPLE) $$1 -- \
				-runs=$(FUZZ_RUNS) -max_total_time=$(FUZZ_MAX_TOTAL_TIME)) || status=$$?; \
		if [ $$status -eq 124 ] || [ $$status -eq 130 ]; then \
			echo "  $$1: reached its $(FUZZ_MAX_TOTAL_TIME)s budget"; \
		elif [ $$status -ne 0 ]; then \
			echo "  $$1: FAILED (exit $$status)"; \
			exit $$status; \
		fi; \
	}; \
	if [ -z "$(TARGET)" ]; then \
		run feel_parser; \
		run decision_table; \
	else \
		run "$(TARGET)"; \
	fi

help:
	@echo "Available targets:"
	@echo "  build           - Build the Rust workspace"
	@echo "  test            - Run all tests (engine + api)"
	@echo "  lint            - Run clippy linter with warnings as errors"
	@echo "  fmt             - Format code with rustfmt"
	@echo "  fmt-check       - Fail if any file is not rustfmt-clean"
	@echo "  tck             - Run the DMN TCK sweep (baseline-gated)"
	@echo "  tck-ledger      - Regenerate the failure ledger in TCK-COVERAGE-GAPS.md"
	@echo "  verify          - Verify specs/requirements.md with CodeWitness"
	@echo "  docker-build    - Build Docker image using existing Dockerfile"
	@echo "  bench           - Run DMN subsystem benchmarks (Criterion)"
	@echo "  memcheck        - Run memory baseline comparison against v6.0 snapshots"
	@echo "  run             - Run the application locally (dev mode)"
	@echo "  clean           - Clean build artifacts"
	@echo "  frontend-dev    - Run frontend dev server"
	@echo "  frontend-build  - Build frontend for production"
	@echo "  frontend-test   - Run frontend tests (vitest)"
	@echo "  e2e             - Browser smoke test of the built UI against a live server"
	@echo "  image-check     - Check the Dockerfile's build logic without Docker"
	@echo "  chart-lint      - Lint and render the Helm chart (needs helm; CI's chart job)"
	@echo "  db-test         - The PostgreSQL store against a real embedded PostgreSQL"
	@echo "  spec-citations  - Check that spec citations still resolve to code"
	@echo "  docs-snippets   - Compile every Rust example in docs/user-guide.md"
	@echo "  readme          - Check the README's commands against the Makefile and crates"
	@echo "  singleton-tests - Tests observing a process-wide tracker must be #[serial]"
	@echo "  fuzz-invocation - The fuzz recipes give cargo-fuzz the same --target, before the --"
	@echo "  preflight       - Disk, cache and cargo-home sanity: a gate's verdict is only worth reading on a healthy machine"
	@echo "  frontend-types  - The interface's response shapes match the keys the API sends"
	@echo "  verification-cost - The verified crates stay light enough for the verification to finish"
	@echo "  audit           - Dependency advisories: none unaccounted for (needs cargo-audit)"
	@echo "  deploy-env      - Check chart/manifest variables against the server's"
	@echo "  bump-version    - Write VERSION to every file that carries it"
	@echo "  ci              - Full CI pipeline (build + test + lint + fmt-check + frontend)"
