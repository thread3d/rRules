#!/usr/bin/env python3
"""Compare what the deployment configuration sets against what the server reads.

A deployment manifest is a claim about the program it runs: that this variable exists, that this
port is where the server listens. The claim rots silently — the image sets `PORT` while a ConfigMap
sets `SERVER_PORT`, and nothing complains because a container with an unread variable starts
exactly like one with a read variable.

Two failures this catches:

- **dead configuration**: a variable the deployment sets and the server never reads. It looks like
  configuration, does nothing, and misleads whoever debugs the deployment next;
- **unset secrets**: a variable the server reads, that the deployment never sets, whose built-in
  default is unsafe. `API_SECRET_KEY` is the one that matters — the default is a public string.

Usage:
  scripts/check-deploy-env.py [--verbose]

Exit codes: 0 configuration and code agree, 1 dead configuration was found.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# Variables whose built-in default is unsafe to run with, so a deployment that does not set them
# is a defect rather than a configuration choice.
SECURITY_RELEVANT = {"API_SECRET_KEY"}

ENV_READ = re.compile(r'env::var\(\s*"([A-Z][A-Z0-9_]*)"')

# Variables a dependency reads, which `env::var` in this crate will never show. Each entry says
# who consumes it, so the allowlist cannot quietly grow into "anything we cannot explain".
CONSUMED_BY_DEPENDENCY = {
    "RUST_LOG": "tracing-subscriber's EnvFilter sets the log level",
}
DOCKER_ENV = re.compile(r"^\s{0,4}ENV\s+([A-Z][A-Z0-9_]*)=|^\s{4,}([A-Z][A-Z0-9_]*)=", re.M)
CONFIG_KEY = re.compile(r"^\s*([A-Z][A-Z0-9_]*):", re.M)
ENV_NAME = re.compile(r"- name:\s*([A-Z][A-Z0-9_]*)\s*$", re.M)
TEMPLATE_VAR = re.compile(r"\{\{\s*\.Values\.config\.([A-Z][A-Z0-9_]*)")


def read_by_server() -> set[str]:
    found: set[str] = set()
    for path in (REPO / "api" / "src").rglob("*.rs"):
        found.update(ENV_READ.findall(path.read_text(encoding="utf-8", errors="replace")))
    return found


def set_by_docker() -> dict[str, set[str]]:
    out: dict[str, set[str]] = {}
    path = REPO / "Dockerfile"
    if path.is_file():
        for match in DOCKER_ENV.finditer(path.read_text(encoding="utf-8")):
            name = match.group(1) or match.group(2)
            out.setdefault(name, set()).add("Dockerfile")
    return out


def set_by_helm() -> dict[str, set[str]]:
    out: dict[str, set[str]] = {}
    for path in (REPO / "helm").rglob("*.yaml"):
        text = path.read_text(encoding="utf-8", errors="replace")
        # Only what the templates actually put in the container environment, plus the values the
        # `config:` block declares. A blanket scan for upper-case words picks up prose.
        names = set(ENV_NAME.findall(text)) | set(TEMPLATE_VAR.findall(text))
        for name in names:
            out.setdefault(name, set()).add(str(path.relative_to(REPO)))
    # Values files also declare the keys the template interpolates.
    values = REPO / "helm" / "rrules-server" / "values.yaml"
    if values.is_file():
        for name in CONFIG_KEY.findall(values.read_text(encoding="utf-8")):
            out.setdefault(name, set()).add(str(values.relative_to(REPO)))
    return out


def set_by_k8s() -> dict[str, set[str]]:
    out: dict[str, set[str]] = {}
    for path in (REPO / "k8s").rglob("*.yaml"):
        text = path.read_text(encoding="utf-8", errors="replace")
        for name in set(CONFIG_KEY.findall(text)) | set(ENV_NAME.findall(text)):
            out.setdefault(name, set()).add(str(path.relative_to(REPO)))
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    read = read_by_server() | set(CONSUMED_BY_DEPENDENCY)
    docker, helm, k8s = set_by_docker(), set_by_helm(), set_by_k8s()

    # Each deployment path is judged on its own: a chart deployed without the raw manifests must
    # still set the variables that matter, and vice versa.
    failures = 0
    for label, sources in (("docker image + helm", (docker, helm)), ("docker image + k8s", (docker, k8s))):
        configured: dict[str, set[str]] = {}
        for source in sources:
            for name, files in source.items():
                configured.setdefault(name, set()).update(files)
        failures += report(label, read, configured, args.verbose)

    return 1 if failures else 0


def report(label: str, read: set[str], configured: dict[str, set[str]], verbose: bool) -> int:


    dead = {name: files for name, files in configured.items() if name not in read}
    unset_secrets = sorted(SECURITY_RELEVANT - set(configured))
    unset_optional = sorted(read - set(configured) - SECURITY_RELEVANT)

    print(f"deployment environment — {label}: {len(read)} variables read by the server")
    print(f"  set by configuration : {len(read & set(configured))}")
    print(f"  set but never read   : {len(dead)}")
    print(f"  never set            : {len(unset_optional)} optional, {len(unset_secrets)} security-relevant")

    for name in sorted(dead):
        print(f"  DEAD      {name}: set by {', '.join(sorted(dead[name]))}, read nowhere in api/src")
    for name in unset_secrets:
        print(f"  UNSET     {name}: the server falls back to an unsafe default")
    if verbose:
        for name in unset_optional:
            note = CONSUMED_BY_DEPENDENCY.get(name)
            print(f"  optional  {name}{f' ({note})' if note else ' (has a documented default)'}")

    return 1 if (dead or unset_secrets) else 0


if __name__ == "__main__":
    sys.exit(main())
