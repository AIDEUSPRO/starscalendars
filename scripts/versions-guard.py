#!/usr/bin/env python3
import sys, os, re, json, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CARGO_TOML = os.path.join(ROOT, 'Cargo.toml')

print('🔎 Versions guard (crates.io max_version vs [workspace.dependencies])')

try:
    with open(CARGO_TOML, 'r', encoding='utf-8') as f:
        txt = f.read()
except Exception as e:
    print(f'❌ cannot read Cargo.toml: {e}', file=sys.stderr)
    sys.exit(1)

# Extract [workspace.dependencies] block
ws_deps_re = re.compile(r"^\[workspace\.dependencies\]\s*(.*?)^\[", re.M | re.S)
m = ws_deps_re.search(txt + "\n[")  # sentinel
if not m:
    print('⚠️ no [workspace.dependencies] found')
    sys.exit(0)
block = m.group(1)

pins = {}
for line in block.splitlines():
    line = line.split('#', 1)[0].strip()
    if not line:
        continue
    # name = "0.8"
    m1 = re.match(r"^([A-Za-z0-9_-]+)\s*=\s*\"([^\"]+)\"", line)
    if m1:
        pins[m1.group(1)] = m1.group(2)
        continue
    # name = { version = "0.8", ... }
    m2 = re.match(r"^([A-Za-z0-9_-]+)\s*=\s*\{[^}]*version\s*=\s*\"([^\"]+)\"", line)
    if m2:
        pins[m2.group(1)] = m2.group(2)

if not pins:
    print('⚠️ no pins parsed')
    sys.exit(0)

def crates_io_latest(crate: str) -> str:
    url = f'https://crates.io/api/v1/crates/{crate}'
    with urllib.request.urlopen(url, timeout=10) as r:
        d = json.load(r)
        return d.get('crate', {}).get('max_version', 'unknown')

def core_ver(v: str) -> str:
    return v.split('-', 1)[0]

def verdict(pin: str, latest: str) -> str:
    p = core_ver(pin).split('.')
    l = core_ver(latest).split('.')
    # If user pins only MAJOR (e.g., "1"), treat as OK when latest MAJOR matches
    if len(p) == 1 and p[0].isdigit() and len(l) >= 1:
        return 'OK' if p[0] == l[0] else 'OUTDATED_MAJOR'
    if len(p) < 2 or len(l) < 2:
        return 'UNKNOWN'
    pmaj, pmin = p[0], p[1]
    lmaj, lmin = l[0], l[1]
    if pmaj != '0':
        return 'OK' if pmaj == lmaj else 'OUTDATED_MAJOR'
    else:
        return 'OK' if pmin == lmin else 'OUTDATED_MINOR'

outdated = []
for name, pin in sorted(pins.items()):
    try:
        latest = crates_io_latest(name)
    except Exception as e:
        print(f'{name} pin={pin} latest=error: {e}')
        continue
    v = verdict(pin, latest)
    print(f'{name} pin={pin} latest={latest} => {v}')
    if v.startswith('OUTDATED'):
        outdated.append(f'{name}:{pin}->{latest}')

if outdated:
    print('❌ Outdated workspace dependency pins detected:', file=sys.stderr)
    for o in outdated:
        print(f' - {o}', file=sys.stderr)
    sys.exit(2)

print('✅ Versions guard passed')

