.PHONY: build test lint fmt docker-build run clean frontend-dev frontend-build ci help

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

help:
	@echo "Available targets:"
	@echo "  build           - Build the Rust workspace"
	@echo "  test            - Run all tests (engine + api)"
	@echo "  lint            - Run clippy linter with warnings as errors"
	@echo "  fmt             - Format code with rustfmt"
	@echo "  docker-build    - Build Docker image using existing Dockerfile"
	@echo "  run             - Run the application locally (dev mode)"
	@echo "  clean           - Clean build artifacts"
	@echo "  frontend-dev    - Run frontend dev server"
	@echo "  frontend-build  - Build frontend for production"
	@echo "  ci              - Full CI pipeline (build + test + lint)"
