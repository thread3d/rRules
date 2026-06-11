# Build stage - use nightly for edition 2024 support with musl tools
FROM rustlang/rust:nightly-slim AS builder

WORKDIR /build

# Install musl-tools for static linking on Debian-based image
RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

# Stage 1: Cache dependencies only (leverages Docker layer caching)
FROM builder AS deps-cached

WORKDIR /build

# Copy only Cargo.toml and Cargo.lock files to cache dependencies
COPY api/Cargo.lock api/Cargo.toml ./
COPY engine/Cargo.toml ./engine/
COPY shared/Cargo.toml ./shared/ 2>/dev/null || true

# Pre-fetch dependencies (cached unless Cargo.toml changes)
RUN cargo fetch --manifest-path api/Cargo.toml

# Stage 2: Build release binary with musl for static linking
FROM builder AS release-builder

WORKDIR /build

COPY . .

# Build with musl for static linking (no glibc dependency)
RUN cargo build --release \
    --bin rRules-server \
    --manifest-path api/Cargo.toml \
    --features engine/default

# Final stage - use alpine for smallest possible image
FROM alpine:3.20

WORKDIR /app

# Install CA certificates for TLS/HTTPS support
RUN apk add --no-cache ca-certificates

# Use nobody user which exists in alpine
USER nobody

# Copy the compiled binary from builder
COPY --from=release-builder /build/target/release/rRules-server /app/rules-server

EXPOSE 8080

ENTRYPOINT ["/app/rules-server"]
