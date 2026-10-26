.PHONY: build test lint fmt fmt-check tck tck-ledger spec-citations verify e2e image-check bump-version docker-build run clean frontend-dev frontend-build frontend-test ci help bench memcheck baseline-update fuzz fuzz-build

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

# Run the application locally (dev mode)
run:
	cargo run --bin api

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
	@# CodeWitness builds the crate itself and passes `--offline` to cargo, so its
	@# resolution relies on a crates.io index snapshot. A `Cargo.lock` pinning a version
	@# newer than that snapshot fails to resolve and the whole run is reported BLOCKED —
	@# a tooling limit, not a code defect. `CARGO_NET_OFFLINE=false` lets the index refresh.
	CARGO_NET_OFFLINE=false codewitness verify engine --requirements specs/requirements.md --in-place --no-mutation
	CARGO_NET_OFFLINE=false codewitness verify api --requirements specs/requirements.md --config api/codewitness.yaml --in-place --no-mutation
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
ci: build test lint fmt-check frontend-build frontend-test spec-citations e2e
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

# Build cargo-fuzz targets (Phase 38+; requires nightly toolchain)
fuzz-build:
	cd engine && cargo +nightly fuzz build

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
		(cd engine && timeout --signal=INT --kill-after=30 $(FUZZ_MAX_TOTAL_TIME) \
			cargo +nightly fuzz run $$1 -- \
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
	@echo "  spec-citations  - Check that spec citations still resolve to code"
	@echo "  bump-version    - Write VERSION to every file that carries it"
	@echo "  ci              - Full CI pipeline (build + test + lint + fmt-check + frontend)"
