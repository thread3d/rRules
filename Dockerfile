# Build stage - use nightly for edition 2024 support with musl tools
FROM rustlang/rust:nightly-slim AS builder

WORKDIR /build

# Install musl-tools for static linking on Debian-based image
RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools \
    file \
    && rm -rf /var/lib/apt/lists/*

# Add musl target for cross-compilation
RUN rustup target add x86_64-unknown-linux-musl

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
    --target x86_64-unknown-linux-musl \
    --bin rRules-server \
    --manifest-path api/Cargo.toml \
    --features engine/default && \
    # Verify binary is statically linked
    file target/x86_64-unknown-linux-musl/release/rRules-server && \
    # Strip binary to reduce size
    strip /build/target/x86_64-unknown-linux-musl/release/rRules-server

# Final stage - use alpine for smallest possible image
FROM alpine:3.20

WORKDIR /app

# Install CA certificates and timezone data for TLS/HTTPS support
RUN apk add --no-cache ca-certificates tzdata && \
    # Create non-root user with fixed UID for security
    addgroup -g 1000 rrules && \
    adduser -u 1000 -G rrules -s /bin/sh -D rrules

# Copy the compiled binary from builder (statically linked, no runtime deps)
COPY --from=release-builder /build/target/x86_64-unknown-linux-musl/release/rRules-server /usr/local/bin/rules-server

EXPOSE 8080

USER rrules

ENTRYPOINT ["/usr/local/bin/rules-server"]
