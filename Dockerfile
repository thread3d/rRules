# rRules server image: the web interface and the API binary in one container.
#
#   docker build -t rrules-server:latest .
#
# The binary serves the IDE from `FRONTEND_DIST`, so the interface is built in its
# own stage and copied in — an API-only image would answer requests but serve no UI.

# ---- 1. Web interface -------------------------------------------------------
FROM node:22-bookworm-slim AS frontend

WORKDIR /src/frontend

# Manifests first, so this layer is cached until the dependencies change.
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --no-audit --no-fund

COPY frontend/ ./
RUN npm run build

# ---- 2. Server binary ------------------------------------------------------
# The same compiler CI tests with. `rust:1-slim` follows stable, so the released
# binary would otherwise be built by a different compiler than the one the gates ran
# on — and the two only disagree at the worst moment.
FROM rust:1.94.1-slim-bookworm AS backend

WORKDIR /src

# Build-time system dependencies:
#   build-essential, pkg-config, zlib1g-dev  libgit2-sys compiles libgit2's C sources
#   cmake                                    some -sys crates generate with it
#   curl, ca-certificates                    `utoipa-swagger-ui`'s build script
#                                            downloads the Swagger UI bundle over
#                                            HTTPS (its `reqwest` feature is off, so
#                                            it shells out to `curl`). Without these
#                                            the build fails with "No such file or
#                                            directory" from that script.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        cmake \
        zlib1g-dev \
        curl \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# The workspace manifests and the lockfile, so resolution matches the repository.
COPY Cargo.toml Cargo.lock ./
COPY engine/Cargo.toml ./engine/
COPY api/Cargo.toml ./api/
COPY engine_state/Cargo.toml ./engine_state/

# The whole member directories, not just their `src`: a crate's manifest declares
# its bench and test targets, and cargo refuses the manifest if those files are
# absent. `.dockerignore` keeps build output and dependencies out of the context.
COPY engine ./engine
COPY api ./api
COPY engine_state ./engine_state

# `--features postgres` compiles the PostgreSQL-backed DMN store, so a deployment can set
# `DMN_PERSISTENCE=postgres` against this image. Without it the flag would be a documented option
# that no shipped artifact could honour.
RUN cargo build --release --locked -p api --bin rRules-server --features postgres

# ---- 3. Runtime ------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g 1000 rrules \
    && useradd -u 1000 -g 1000 -s /usr/sbin/nologin -M rrules

WORKDIR /app

COPY --from=backend /src/target/release/rRules-server /usr/local/bin/rrules-server
COPY --from=frontend /src/frontend/dist /app/frontend/dist

# Every path the server reads at startup is set explicitly, so the container needs
# no working-directory assumptions. Mount a volume over /data to persist rules.
ENV RUST_LOG=info \
    FRONTEND_DIST=/app/frontend/dist \
    RULE_STORAGE_PATH=/data/rules \
    DMN_STORAGE_PATH=/data/dmn \
    WATCH_RULES_DIR=true \
    PORT=8080

RUN mkdir -p /data/rules /data/dmn && chown -R rrules:rrules /data

USER rrules

EXPOSE 8080

# Liveness and readiness probes live in the Helm chart (`/health`, `/ready`), not
# here: an in-image check would need a shell and a client the image does not carry.

ENTRYPOINT ["/usr/local/bin/rrules-server"]
