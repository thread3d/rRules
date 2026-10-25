.PHONY: build test lint fmt fmt-check tck tck-ledger verify docker-build run clean frontend-dev frontend-build frontend-test ci help bench memcheck baseline-update fuzz fuzz-build

# Build the Rust workspace
build:
	cargo build --workspace

# Run all tests (engine + api)
test:
	cargo test --workspace

# Run clippy linter with warnings as errors
lint:
	cargo clippy --workspace -- -D warnings

# Format code with rustfmt
fmt:
	cargo fmt --all

# Fail if any file is not rustfmt-clean (CI gate)
fmt-check:
	cargo fmt --all -- --check

# Build Docker image using existing Dockerfile
docker-build:
	docker build -t rrules-server:latest .

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
	codewitness verify engine --requirements specs/requirements.md --in-place --no-mutation
	codewitness verify api --requirements specs/requirements.md --config api/codewitness.yaml --in-place --no-mutation

# Run the frontend test suite
frontend-test:
	cd frontend && npx vitest run

# Full CI pipeline (build + test + lint + format check + frontend)
ci: build test lint fmt-check frontend-build frontend-test
	@echo "CI pipeline complete"

# Run DMN subsystem benchmarks (Criterion, Phase 36+)
bench:
	cargo bench --bench dmn_benchmarks -p engine

# Run memory baseline comparison against the frozen v6.0 snapshot (regression
# gate, PERF-04). Writes a CURRENT snapshot (engine/benches/snapshots/v6.0-current.json)
# and compares it against the committed v6.0-baseline.json via the gate script.
# Exits non-zero on RSS/allocation regression beyond threshold.
memcheck:
	@echo "Running memory baseline check..."
	@if [ ! -f engine/benches/snapshots/v6.0-baseline.json ]; then \
		echo "ERROR: frozen v6.0 baseline snapshot not found at engine/benches/snapshots/v6.0-baseline.json"; \
		echo "Run 'make baseline-update' to capture a baseline, then commit it."; \
		exit 1; \
	fi
	cargo run -p engine --bin memory_baseline
	@./scripts/memcheck-gate.sh

# Refresh the FROZEN v6.0 baseline snapshot (PERF-04). Use only when intentionally
# accepting new memory numbers (e.g. after an optimization that legitimately
# changes the baseline). Commit the resulting v6.0-baseline.json.
baseline-update:
	@echo "Refreshing frozen v6.0 baseline snapshot..."
	cargo run -p engine --bin memory_baseline -- --update-baseline
	@echo "Frozen baseline refreshed. Commit engine/benches/snapshots/v6.0-baseline.json."

# Build cargo-fuzz targets (Phase 38+; requires nightly toolchain)
fuzz-build:
	cd engine && cargo +nightly fuzz build

# Run DMN fuzz targets (Phase 38+; requires nightly + cargo-fuzz).
# Usage: make fuzz TARGET=decision_table FUZZ_MAX_TOTAL_TIME=300
#        (no TARGET runs every target; defaults bound each run to 60 s)
FUZZ_MAX_TOTAL_TIME ?= 60
FUZZ_RUNS ?= 600000
fuzz:
	@set -e; \
	run() { \
		echo "Fuzzing $$1 ($(FUZZ_RUNS) runs, max $(FUZZ_MAX_TOTAL_TIME) s)..."; \
		cd engine && cargo +nightly fuzz run $$1 -- \
			-runs=$(FUZZ_RUNS) -max_total_time=$(FUZZ_MAX_TOTAL_TIME); \
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
	@echo "  ci              - Full CI pipeline (build + test + lint + fmt-check + frontend)"
