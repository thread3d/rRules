#!/usr/bin/env python3
"""Bump every version source in one place.

The version lives in seven places and they drifted badly once (crates at 0.1.0,
frontend 4.0.0, chart 1.0.0). This script keeps the bump honest.
"""
import json
import pathlib
import re
import sys

new = sys.argv[1]
root = pathlib.Path('.')

touched = []

# `db_tests` is a separate workspace (excluded from the root so `pg-embed` never reaches `api`), but it
# is still a crate in this repository whose version is part of the release: 9.0.85 shipped with it at
# 9.0.58, which is the kind of "every version lives in one place" claim this script exists to keep.
for crate in ('engine', 'api', 'engine_state', 'engine/fuzz', 'db_tests'):
    p = root / crate / 'Cargo.toml'
    s = p.read_text()
    s2 = re.sub(r'(?m)^version = "[^"]+"', f'version = "{new}"', s, count=1)
    if s2 != s:
        p.write_text(s2)
        touched.append(str(p))

p = root / 'frontend' / 'package.json'
data = json.loads(p.read_text())
data['version'] = new
p.write_text(json.dumps(data, indent=2) + '\n')
touched.append(str(p))

p = root / 'frontend' / 'package-lock.json'
data = json.loads(p.read_text())
data['version'] = new
if '' in data.get('packages', {}):
    data['packages']['']['version'] = new
p.write_text(json.dumps(data, indent=2) + '\n')
touched.append(str(p))

p = root / 'helm' / 'rrules-server' / 'Chart.yaml'
s = p.read_text()
s = re.sub(r'(?m)^version: .*$', f'version: {new}', s)
s = re.sub(r'(?m)^appVersion: .*$', f'appVersion: "{new}"', s)
p.write_text(s)
touched.append(str(p))

# The README opens with the release it documents; a bump that leaves it behind is the
# first thing a visitor reads.
for path, pattern, replacement in [
    ('README.md', r'\*\*[0-9]+\.[0-9]+\.[0-9]+\*\* — ', f'**{new}** — '),
    ('api/src/openapi.rs', r'version = "[^"]+"', f'version = "{new}"'),
    ('api/src/main.rs', r'rRules REST API Server v[0-9.]+', f'rRules REST API Server v{new}'),
    ('api/src/metrics.rs', r'rRules v[0-9.]+', f'rRules v{new}'),
    ('specs/http-api.md', r'version `[0-9.]+`', f'version `{new}`'),
]:
    p = root / path
    s = p.read_text()
    s2 = re.sub(pattern, replacement, s)
    if s2 != s:
        p.write_text(s2)
        touched.append(path)

print(f"version {new} written to:")
for path in touched:
    print(f"  {path}")
