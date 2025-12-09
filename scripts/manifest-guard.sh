#!/usr/bin/env bash
set -euo pipefail

fail=0

# 1) Forbid rust-version in any Cargo.toml
if grep -R --line-number --include "Cargo.toml" '^rust-version\s*=' .; then
  echo "❌ rust-version must NOT be specified in Cargo.toml (policy)" >&2
  fail=1
fi

# 2) Require edition = "2024" in each package Cargo.toml
while IFS= read -r -d '' f; do
  case "$f" in
    ./astro-rust/*) continue ;; # skip vendor/readonly astro-rust
  esac
  if grep -q '^[[:space:]]*\[package\]' "$f"; then
    if ! grep -q '^edition\s*=\s*"2024"' "$f"; then
      echo "❌ Missing or wrong edition in $f (expected edition = \"2024\")" >&2
      fail=1
    fi
  fi
done < <(find . -name Cargo.toml -print0)

# 2.5) Forbid legacy crate path in workspace members (kept from reference; harmless if absent)
if grep -q 'mpcrag-service' Cargo.toml; then
  echo "❌ mpcrag-service must be removed from workspace members (legacy)" >&2
  fail=1
fi

# 3) Workspace pin policy: majors only (0.x -> minor) for core deps
root="./Cargo.toml"
if [ -f "$root" ]; then
  if ! grep -qE '^axum\s*=\s*"0\.8"(\s|$)' "$root"; then
    if ! grep -qE '^axum\s*=\s*\{[^}]*version\s*=\s*"0\.8"' "$root"; then
      echo "❌ axum must be pinned to \"0.8\" in workspace" >&2; fail=1;
    fi
  fi
  if ! grep -q '^tokio\s*=\s*{[^}]*version\s*=\s*"1"' "$root"; then echo "❌ tokio must be pinned to \"1\"" >&2; fail=1; fi
  if ! grep -q '^tracing\s*=\s*"0\.1"' "$root"; then echo "❌ tracing must be pinned to \"0.1\"" >&2; fail=1; fi
  if ! grep -q '^tracing-subscriber\s*=\s*{[^}]*version\s*=\s*"0\.3"' "$root"; then echo "❌ tracing-subscriber must be pinned to \"0.3\"" >&2; fail=1; fi
  if ! grep -q '^thiserror\s*=\s*"2"' "$root"; then echo "❌ thiserror must be pinned to \"2\"" >&2; fail=1; fi
  if ! grep -q '^serde\s*=\s*{[^}]*version\s*=\s*"1"' "$root"; then echo "❌ serde must be pinned to \"1\"" >&2; fail=1; fi
  if ! grep -q '^serde_json\s*=\s*"1"' "$root"; then echo "❌ serde_json must be pinned to \"1\"" >&2; fail=1; fi
  if ! grep -q '^uuid\s*=\s*{[^}]*version\s*=\s*"1"' "$root"; then echo "❌ uuid must be pinned to \"1\"" >&2; fail=1; fi
fi

exit $fail

