#!/bin/bash
# Anti-pattern scan (Rust) for AI+KO project
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

EXCLUDES=(--exclude-dir=.git --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist)
RG_GLOBS=(
  -g '!.git/**'
  -g '!target/**'
  -g '!node_modules/**'
  -g '!dist/**'
  -g '!**/tests/**'
  -g '!**/benches/**'
  -g '!**/examples/**'
  -g '!**/*_test.rs'
  -g '!**/*_tests.rs'
  -g '!**/integration_tests.rs'
  -g '*.rs'
)

search() {
  local pattern="$1"
  if command -v rg >/dev/null 2>&1; then
    rg -n --color=never -uu ${RG_GLOBS[@]} -e "$pattern" . || true
  else
    grep -rn ${EXCLUDES[@]} --include="*.rs" -- "$pattern" . 2>/dev/null || true
  fi
}

# Remove matches that are inside test modules within a file.
# Heuristic: if file contains a line "mod tests {" and match line number is AFTER that, drop it.
filter_out_test_hits() {
  while IFS= read -r entry; do
    [[ -z "$entry" ]] && continue
    local file line rest
    file="${entry%%:*}"
    rest="${entry#*:}"
    line="${rest%%:*}"
    # Find first occurrence of a tests module
    local test_start
    test_start=$(rg -n --color=never -uu -e '^\s*mod\s+tests\s*\{' "$file" | head -1 | cut -d: -f1 || true)
    if [[ -n "${test_start:-}" ]]; then
      if [[ "$line" -gt "$test_start" ]]; then
        # Inside tests module → allow all anti-patterns
        continue
      fi
    fi
    echo "$entry"
  done
}

prefix_first_n() {
  # Print first N lines and prefix with two spaces + dash; BSD sed compatible
  local n="$1"
  sed -n "1,${n}p" | sed 's/^/  - /'
}

VIOLATIONS=0

echo "🚨 Core Rust anti-patterns"
for pat in "\\.unwrap\\(\\)" "\\.expect\\(" "panic!\\(" "HashMap::new\\(" "Vec::new\\(" "unimplemented!\\(" "todo!\\("; do
  out=$(search "$pat" | filter_out_test_hits)
  if [[ -n "$out" ]]; then 
    echo -e "${RED}❌ $pat${NC}"
    echo "$out" | prefix_first_n 30
    case "$pat" in
      "\\.unwrap\\(\\)")
        echo "  Guidance: use ? to propagate, or match/if let; define error types with thiserror"
        ;;
      "\\.expect\\(")
        echo "  Guidance: remove expect; use Result + ? or explicit match with proper error"
        ;;
      "panic!\\(")
        echo "  Guidance: never panic in prod paths; return Result and map to RFC7807"
        ;;
    esac
    VIOLATIONS=$((VIOLATIONS+1))
  else 
    echo -e "${GREEN}✅ $pat clean${NC}"
  fi
done

echo "🧵 Std logging/printing macros (use tracing only)"
for pat in "println!\\(" "eprintln!\\(" "dbg!\\(" "log::info!\\(" "log::warn!\\(" "log::error!\\(" "log::debug!\\("; do
  out=$(search "$pat" | filter_out_test_hits)
  if [[ -n "$out" ]]; then 
    echo -e "${RED}❌ Disallowed macro: $pat (use tracing)${NC}"
    echo "$out" | prefix_first_n 30
    echo "  Guidance: replace with tracing::{error!, warn!, info!, debug!, trace!}"
    VIOLATIONS=$((VIOLATIONS+1))
  else 
    echo -e "${GREEN}✅ $pat clean${NC}"
  fi
done

# STRICT BAN: All unwrap* patterns forbidden in production code
echo "🛑 STRICT BAN: All unwrap* patterns (including unwrap_or_else)"
for pat in "\\.unwrap_or_else\\("; do
  out=$(search "$pat" | filter_out_test_hits)
  if [[ -n "$out" ]]; then
    echo -e "${RED}❌ unwrap_or_else forbidden${NC}"
    echo "$out" | prefix_first_n 30
    echo "  Guidance:"
    echo "  - Prefer explicit control flow: match / if let / let-else"
    echo "  - Option<T>: if let Some(v) = opt { v } else { default }"
    echo "  - Result<T,E>: if let Ok(v) = res { v } else { default }  (or use ? to propagate)"
    VIOLATIONS=$((VIOLATIONS+1))
  else
    echo -e "${GREEN}✅ No unwrap_or_else patterns${NC}"
  fi
done

# Strict ban on unwrap_or (eager evaluation) — no exceptions
for pat in "\\.unwrap_or\\("; do
  out=$(search "$pat" | filter_out_test_hits)
  if [[ -n "$out" ]]; then
    echo -e "${RED}❌ Forbidden eager unwrap_or: $pat${NC}"
    echo "$out" | prefix_first_n 60
    echo "  Guidance:"
    echo "  - Prefer explicit control flow: match / if let / let-else (no unwrap_*)"
    echo "  - Option<T>: if let Some(v) = opt { v } else { default }"
    echo "  - Result<T,E>: if let Ok(v) = res { v } else { default }  (or use ? to propagate)"
    VIOLATIONS=$((VIOLATIONS+1))
  else
    echo -e "${GREEN}✅ $pat clean${NC}"
  fi
done

# Strict ban on unwrap_or_default — replace with map_or_else(Default::default, |v| v) or propagate with ?
echo "🛑 STRICT BAN: unwrap_or_default (prefer match/if let/let-else)"
for pat in "\\.unwrap_or_default\\("; do
  out=$(search "$pat" | filter_out_test_hits)
  if [[ -n "$out" ]]; then
    echo -e "${RED}❌ unwrap_or_default forbidden${NC}"
    echo "$out" | prefix_first_n 60
    echo "  Guidance:"
    echo "  - Option<T>: if let Some(v) = opt { v } else { Default::default() }"
    echo "  - Result<T,E>: if let Ok(v) = res { v } else { Default::default() } (or use ? to propagate)"
    VIOLATIONS=$((VIOLATIONS+1))
  else
    echo -e "${GREEN}✅ No unwrap_or_default patterns${NC}"
  fi
done

echo "🔍 Eager default patterns"
# unwrap_or/ok_or eager defaults (map_or is allowed for lazy evaluation)
for pat in "\\.unwrap_or\([^)]*[a-zA-Z_][a-zA-Z0-9_]*\s*\(" "\\.ok_or\([^)]*[a-zA-Z_][a-zA-Z0-9_]*\s*\("; do
  out=$(search "$pat" | filter_out_test_hits)
  if [[ -n "$out" ]]; then echo -e "${RED}❌ Eager default detected: $pat${NC}"; echo "$out" | prefix_first_n 30; VIOLATIONS=$((VIOLATIONS+1)); else echo -e "${GREEN}✅ No eager defaults for $pat${NC}"; fi
done

# map_or is allowed for lazy evaluation (not eager)
echo "✅ map_or patterns (lazy evaluation allowed)"
for pat in "\\.map_or\([^)]*[a-zA-Z_][a-zA-Z0-9_]*\s*\("; do
  out=$(search "$pat" | filter_out_test_hits)
  if [[ -n "$out" ]]; then
    echo -e "${YELLOW}ℹ️ map_or found (allowed for lazy evaluation)${NC}"
    echo "$out" | prefix_first_n 20
  else
    echo -e "${GREEN}✅ No map_or patterns${NC}"
  fi
done

echo "🛑 Forbidden unwrap_* variants"
# Disallow common and custom helper variants like unwrap_u64/unwrap_str/etc. (but allow unwrap_or/unwrap_or_default/unwrap_or_else)
for pat in "\\.unwrap_[a-zA-Z0-9_]*\\(" "\\.unwrap_unchecked\\(" "\\.unwrap_err\\("; do
  out=$(search "$pat" | filter_out_test_hits)
  if [[ -n "$out" ]]; then
    out=$(echo "$out" | grep -Ev 'unwrap_or\(|unwrap_or_default\(|unwrap_or_else\(' || true)
  fi
  if [[ -n "$out" ]]; then echo -e "${RED}❌ Forbidden: $pat${NC}"; echo "$out" | prefix_first_n 30; VIOLATIONS=$((VIOLATIONS+1)); else echo -e "${GREEN}✅ $pat clean${NC}"; fi
done

echo "🔒 SQL safety"
out=$(search "format!\\s*\\(\\s*\\\"\\s*(SELECT|INSERT|UPDATE|DELETE)" | filter_out_test_hits)
if [[ -n "$out" ]]; then echo -e "${RED}❌ Potential SQL via format!${NC}"; echo "$out" | prefix_first_n 30; VIOLATIONS=$((VIOLATIONS+1)); else echo -e "${GREEN}✅ No SQL via format!${NC}"; fi

echo "⏳ Blocking calls in async contexts (heuristics)"
# std::thread::sleep is always forbidden in async code paths
sleep_hits=$(search "std::thread::sleep\\(" | filter_out_test_hits)
if [[ -n "$sleep_hits" ]]; then echo -e "${RED}❌ std::thread::sleep detected${NC}"; echo "$sleep_hits" | prefix_first_n 30; VIOLATIONS=$((VIOLATIONS+1)); else echo -e "${GREEN}✅ No std::thread::sleep${NC}"; fi

echo "🔁 .await inside loops (heuristic via ripgrep PCRE)"
if command -v rg >/dev/null 2>&1; then
  loop_await=$(rg -n --color=never -uu -U -P ${RG_GLOBS[@]} -e "for\s+\w+\s+in[\s\S]*?\{[\s\S]*?\.await" . || true)
  if [[ -n "$loop_await" ]]; then echo -e "${YELLOW}⚠️ Potential .await inside loop (review needed)${NC}"; echo "$loop_await" | prefix_first_n 40; else echo -e "${GREEN}✅ No obvious await-in-loop${NC}"; fi
else
  echo -e "${YELLOW}⚠️ rg not available; skip await-in-loop heuristic${NC}"
fi

if [[ $VIOLATIONS -gt 0 ]]; then
  echo -e "${RED}🚫 Anti-pattern scan found $VIOLATIONS issues${NC}"; exit 1
else
  echo -e "${GREEN}🎉 Anti-pattern scan passed${NC}"; fi

# KO-service specific guards (MCP rmcp-only, Postgres-only, no direct Qdrant)
echo "🛡️ KO-service guards (rmcp-only, Postgres-only, no Qdrant)"

# Only warn by default; enforce when ENFORCE_KO_GUARDS=1
ENFORCE_KO_GUARDS=${ENFORCE_KO_GUARDS:-0}

# 1) Forbid direct Qdrant usage from ko-service
qdrant_hits=$(search "qdrant" | grep "^ko-service/src/" || true)
if [[ -n "$qdrant_hits" ]]; then 
  echo -e "${RED}❌ ko-service must not reference Qdrant directly (use n8n workflows)${NC}"
  echo "$qdrant_hits" | prefix_first_n 40
  if [[ "$ENFORCE_KO_GUARDS" == "1" ]]; then VIOLATIONS=$((VIOLATIONS+1)); fi
else
  echo -e "${GREEN}✅ No direct Qdrant references in ko-service${NC}"
fi

# 2) Forbid SQLite leftovers in ko-service after Postgres-only migration
sqlite_hits=$( (search "\bSqlite\b|rusqlite|sqlite::" | grep "^ko-service/src/" ) || true)
if [[ -n "$sqlite_hits" ]]; then 
  echo -e "${RED}❌ ko-service must be PostgreSQL-only: SQLite remnants detected${NC}"
  echo "$sqlite_hits" | prefix_first_n 40
  if [[ "$ENFORCE_KO_GUARDS" == "1" ]]; then VIOLATIONS=$((VIOLATIONS+1)); fi
else
  echo -e "${GREEN}✅ No SQLite remnants in ko-service source${NC}"
fi

# 3) Enforce rmcp-only (no custom MCP transports/registries)
# a) Flag suspicious 'transport'/'registry' in ko-service/src/mcp (heuristic)
mcp_sus=$( (rg -n --color=never -uu -g '!target/**' -g '!node_modules/**' -g '!dist/**' -g 'ko-service/src/mcp/**' -e '\btransport\b|\bregistry\b' || true) )
if [[ -n "$mcp_sus" ]]; then
  echo -e "${YELLOW}⚠️ Review ko-service mcp code for custom transport/registry (rmcp-only policy)${NC}"
  echo "$mcp_sus" | prefix_first_n 40
  # Only enforce if explicit non-rmcp imports found
fi
# b) Fail on external MCP crates whose first segment starts with 'mcp' (rmcp is allowed)
non_rmcp_imports=$(rg -n --color=never -uu -g '!target/**' -g '!node_modules/**' -g '!dist/**' -g 'ko-service/src/**' -e '^\s*use\s+(mcp[[:alnum:]_]*)::' || true)
if [[ -n "$non_rmcp_imports" ]]; then
  # Drop allowed rmcp
  non_rmcp_imports=$(echo "$non_rmcp_imports" | grep -vE '^\s*use\s+rmcp(::|\s)' || true)
fi
if [[ -n "$non_rmcp_imports" ]]; then
  echo -e "${RED}❌ rmcp-only violation: external MCP crate import detected (only rmcp allowed)${NC}"
  echo "$non_rmcp_imports" | prefix_first_n 40
  if [[ "$ENFORCE_KO_GUARDS" == "1" ]]; then VIOLATIONS=$((VIOLATIONS+1)); fi
else
  echo -e "${GREEN}✅ rmcp-only imports policy respected${NC}"
fi

# Rust 1.90 module system hygiene
echo "📦 Module system hygiene (Rust 1.90)"
mod_rs=$(search "/mod\.rs$")
if [[ -n "$mod_rs" ]]; then echo -e "${YELLOW}⚠️ mod.rs files present (prefer flat modules)${NC}"; echo "$mod_rs" | head -20 | sed 's/^/  - /'; fi
extern_crate=$(search "^extern crate ")
if [[ -n "$extern_crate" ]]; then echo -e "${RED}❌ extern crate usage is disallowed (2018+ edition)${NC}"; echo "$extern_crate" | head -20 | sed 's/^/  - /'; VIOLATIONS=$((VIOLATIONS+1)); else echo -e "${GREEN}✅ No extern crate${NC}"; fi

# Non-blocking reminder: research gate
echo "🔎 Reminder: Run 'technical-researcher' and attach a Research Brief before implementation tasks."
