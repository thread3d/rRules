#!/usr/bin/env bash
# Check the Dockerfile's build logic without a container runtime.
#
# The image build is the one gate that cannot run in every environment (no Docker,
# no podman), so its failure only ever showed up in a Deploy run — where a broken
# COPY set looks like a deployment problem. This script performs the same two
# stages on the host: it assembles the exact files each stage copies, then runs the
# same commands. If the Dockerfile references a file that does not exist, or the
# COPY set is incomplete, this fails here instead.
#
#   make image-check
#
# It is deliberately blunt: `COPY` lines are read from the Dockerfile, so editing
# the image changes what gets checked. Anything the script cannot emulate (apt
# packages, the base image) is out of scope and is listed at the end.
set -euo pipefail

# `cc` here is a ccache wrapper whose home is read-only; C build scripts inside the simulated
# build (libgit2-sys, zstd-sys) fail without this.
export CCACHE_DISABLE=1

cd "$(dirname "$0")/.."
REPO_ROOT=$(pwd)
WORK=$(mktemp -d "${TMPDIR:-/tmp}/rrules-image-check.XXXXXX")
trap 'rm -rf "$WORK"' EXIT

DOCKERFILE="${DOCKERFILE:-Dockerfile}"
[ -f "$DOCKERFILE" ] || { echo "image-check: no $DOCKERFILE" >&2; exit 1; }

echo "==> reading COPY instructions from $DOCKERFILE"
# Paths copied into the image, ignoring anything from another stage or a URL.
mapfile -t COPY_PATHS < <(
    python3 - "$DOCKERFILE" <<'PY'
import re, sys
text = open(sys.argv[1]).read().replace('\\\n', ' ')
seen = set()
for line in text.splitlines():
    line = line.strip()
    if not line.upper().startswith('COPY '):
        continue
    if '--from=' in line:
        continue
    parts = line.split()[1:]
    # The last token is the destination; the rest are sources.
    for source in parts[:-1]:
        source = source.rstrip('/')
        if source in ('.', './') or source.startswith('http'):
            continue
        if source not in seen:
            seen.add(source)
            print(source)
PY
)

missing=0
for path in "${COPY_PATHS[@]}"; do
    if [ -e "$REPO_ROOT/$path" ]; then
        printf '  ok    %s\n' "$path"
    else
        printf '  MISSING %s\n' "$path"
        missing=1
    fi
done
[ "$missing" -eq 0 ] || { echo "image-check: the Dockerfile copies paths that do not exist" >&2; exit 1; }

# Build scripts that shell out to system tools fail only inside the minimal image,
# where those tools are absent — the kind of failure this script exists to catch.
# `utoipa-swagger-ui` downloads its bundle with `curl` (see its build.rs), so the
# builder stage must install curl and CA certificates.
echo "==> build-time system dependencies"
builder_install=$(awk '/^FROM .* AS backend/,/^FROM .* AS runtime/' "$DOCKERFILE")
for tool in curl ca-certificates; do
    if printf '%s' "$builder_install" | grep -qw "$tool"; then
        printf '  ok    builder installs %s\n' "$tool"
    else
        printf '  MISSING builder does not install %s (needed by utoipa-swagger-ui build script)\n' "$tool"
        missing=1
    fi
done
[ "$missing" -eq 0 ] || { echo "image-check: the builder stage is missing a build-time dependency" >&2; exit 1; }

echo "==> stage 1: web interface"
mkdir -p "$WORK/frontend"
cp frontend/package.json frontend/package-lock.json "$WORK/frontend/"
if [ -d frontend/node_modules ]; then
    cp -r frontend/node_modules "$WORK/frontend/"
else
    (cd "$WORK/frontend" && npm ci --no-audit --no-fund >/dev/null)
fi
# `COPY frontend/ ./` — everything except what the build regenerates.
tar -C frontend --exclude=node_modules --exclude=dist -cf - . | tar -C "$WORK/frontend" -xf -
(cd "$WORK/frontend" && npm run build >/dev/null)
[ -f "$WORK/frontend/dist/index.html" ] || { echo "image-check: the interface produced no dist/index.html" >&2; exit 1; }
echo "    dist/index.html built"

echo "==> stage 2: server binary"
mkdir -p "$WORK/api" "$WORK/engine" "$WORK/engine_state"
cp Cargo.toml Cargo.lock "$WORK/"
for crate in engine api engine_state; do
    cp "$crate/Cargo.toml" "$WORK/$crate/"
    tar -C "$crate" -cf - . | tar -C "$WORK/$crate" -xf -
done
(cd "$WORK" && cargo build --release --locked -p api --bin rRules-server)
[ -x "$WORK/target/release/rRules-server" ] || { echo "image-check: no release binary" >&2; exit 1; }
echo "    target/release/rRules-server built ($(du -h "$WORK/target/release/rRules-server" | cut -f1))"

echo
echo "image-check: both stages build from the Dockerfile's COPY set."
echo "Not covered here (container-only): base images, apt packages, the final"
echo "runtime stage, and anything that depends on the container's own toolchain."
