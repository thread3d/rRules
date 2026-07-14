.PHONY: build test lint fmt docker-build run clean frontend-dev frontend-build ci help bench memcheck fuzz fuzz-build

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

# Full CI pipeline (build + test + lint)
ci: build test lint frontend-build
	@echo "CI pipeline complete"

# Run DMN subsystem benchmarks (Criterion, Phase 36+)
bench:
	cargo bench --bench dmn_benchmarks -p engine

# Run memory baseline comparison against v6.0 snapshots (regression gate)
memcheck:
	@echo "Running memory baseline check..."
	@if [ ! -f engine/benches/snapshots/v6.0-baseline.json ]; then \
		echo "ERROR: v6.0 baseline snapshot not found at engine/benches/snapshots/v6.0-baseline.json"; \
		echo "Run 'make bench' first to generate baselines, or update the baseline with new measurements."; \
		exit 1; \
	fi
	cargo run -p engine --bin memory_baseline

# Build cargo-fuzz targets (Phase 38+; requires nightly toolchain)
fuzz-build:
	cd engine && cargo +nightly fuzz build

# Run DMN fuzz targets (Phase 38+; requires nightly + cargo-fuzz).
# Usage: make fuzz TARGET=decision_table   (default: all targets)
fuzz:
	@if [ -z "$$TARGET" ]; then \
		echo "Fuzzing all targets (bounded, ~60s each)..."; \
		cd engine && cargo +nightly fuzz run feel_parser -- -max_total_time=60; \
		cd engine && cargo +nightly fuzz run decision_table -- -max_total_time=60; \
	else \
		echo "Fuzzing target: $$TARGET"; \
		cd engine && cargo +nightly fuzz run $$TARGET -- -max_total_time=60; \
	fi

help:
	@echo "Available targets:"
	@echo "  build           - Build the Rust workspace"
	@echo "  test            - Run all tests (engine + api)"
	@echo "  lint            - Run clippy linter with warnings as errors"
	@echo "  fmt             - Format code with rustfmt"
	@echo "  docker-build    - Build Docker image using existing Dockerfile"
	@echo "  bench           - Run DMN subsystem benchmarks (Criterion)"
	@echo "  memcheck        - Run memory baseline comparison against v6.0 snapshots"
	@echo "  run             - Run the application locally (dev mode)"
	@echo "  clean           - Clean build artifacts"
	@echo "  frontend-dev    - Run frontend dev server"
	@echo "  frontend-build  - Build frontend for production"
	@echo "  ci              - Full CI pipeline (build + test + lint)"
