#!/bin/bash

# 🛡️ Quality Guardian: Enhanced anti-pattern scanning with test code exclusion
# Canonical patterns source: anti.md (plus QUALITY.md and CLAUDE.md)

set -euo pipefail

echo "🛡️ Quality Guardian: Comprehensive anti-pattern scanning..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Centralized excludes
EXCLUDES=(--exclude-dir=.git --exclude-dir=target --exclude-dir=astro-rust --exclude-dir=node_modules --exclude-dir=dist --exclude-dir=frontend/node_modules)

# Search helper (prefers ripgrep if available)
search_content() {
  local pattern="$1"; shift
  if command -v rg >/dev/null 2>&1; then
    rg -n --color=never -uu ${EXCLUDES[@]} -e "$pattern" "$@" || true
  else
    grep -rn ${EXCLUDES[@]} --include="*.rs" -- "$pattern" "$@" 2>/dev/null || true
  fi
}

VIOLATIONS=0
TOTAL_FILES=$(find . -name "*.rs" -not -path "./target/*" -not -path "./astro-rust/*" | wc -l | tr -d ' ')

echo "📊 Scanning $TOTAL_FILES Rust files"

# Function to check if a line is within a test module (ENHANCED: better test detection)
is_in_test_code() {
    local file="$1"
    local line_number="$2"

    # Primary check: #[cfg(test)] module with proper scope detection
    local cfg_test_line=$(awk '/#\[cfg\(test\)\]/ { print NR }' "$file" | tail -1)

    if [[ -n "$cfg_test_line" ]] && [[ $line_number -gt $cfg_test_line ]]; then
        # Check if we're still within the test module by finding the next non-test module
        local next_mod_line=$(awk -v start="$cfg_test_line" '
            NR > start && /^[[:space:]]*#\[cfg\(.*\)\][[:space:]]*$/ && !/cfg\(test\)/ { print NR; exit }
        ' "$file")

        # If no next module found, or we're before it, we're in test code
        if [[ -z "$next_mod_line" ]] || [[ $line_number -lt $next_mod_line ]]; then
            echo "IN_TEST"
            return 0
        fi
    fi

    # Secondary check: individual #[test] functions (works regardless of cfg_test)
    local test_fn_line=$(awk -v target="$line_number" '
        /#\[test\]/ { if (NR < target && target - NR <= 20) print NR }
    ' "$file" | tail -1)

    if [[ -n "$test_fn_line" ]]; then
        echo "IN_TEST"
        return 0
    fi

    # Tertiary check: inside mod tests { } block
    local in_test_mod=$(awk -v target="$line_number" '
        /^[[:space:]]*mod[[:space:]]+tests[[:space:]]*\{/ { test_start = NR }
        /^[[:space:]]*\}[[:space:]]*$/ && test_start && NR > test_start {
            if (target > test_start && target < NR) { print "IN_TEST"; exit }
            test_start = ""
        }
    ' "$file")

    if [[ "$in_test_mod" == "IN_TEST" ]]; then
        echo "IN_TEST"
        return 0
    fi

    # Not in test code
    echo "NOT_IN_TEST"
}

# Function to scan for a pattern with test exclusion
scan_pattern() {
    local pattern="$1"
    local description="$2"
    local suggestion="$3"
    local allow_in_tests="${4:-true}"

    echo "🔍 Scanning for: $pattern"

    local matches=$(search_content "$pattern" .)

    if [[ -z "$matches" ]]; then
        echo "✅ No violations found for: $pattern"
        return 0
    fi

    local production_violations=""
    local test_violations=""
    local total_matches=0

    while IFS= read -r match; do
        if [[ -z "$match" ]]; then
            continue
        fi

        file=$(echo "$match" | cut -d: -f1)
        line_num=$(echo "$match" | cut -d: -f2)
        content=$(echo "$match" | cut -d: -f3-)

        if [[ ! -r "$file" ]]; then
            continue
        fi

        ((total_matches++))

        local test_result=$(is_in_test_code "$file" "$line_num")

        if [[ "$test_result" == "IN_TEST" ]]; then
            test_violations+="  $file:$line_num:${content}"$'\n'
        else
            production_violations+="  $file:$line_num:${content}"$'\n'
        fi
    done <<< "$matches"

    # Report results
    if [[ -n "$production_violations" ]]; then
        echo -e "${RED}❌ CRITICAL: Found forbidden pattern: $pattern${NC}"
        echo -e "${YELLOW}📝 Suggestion: $suggestion${NC}"
        echo "📁 Files:"
        echo "$production_violations" | sed 's/^/  - /'
        ((VIOLATIONS++))
        return 1
    elif [[ -n "$test_violations" ]]; then
        if [[ "$allow_in_tests" == "true" ]]; then
            echo -e "${GREEN}✅ Pattern found only in test code (acceptable): $pattern${NC}"
            echo -e "${BLUE}📍 Test locations:${NC}"
            echo "$test_violations" | sed 's/^/  - /' | head -5
            return 0
        else
            echo -e "${RED}❌ CRITICAL: Found forbidden pattern even in tests: $pattern${NC}"
            echo -e "${YELLOW}📝 Suggestion: $suggestion${NC}"
            echo "📁 Test files:"
            echo "$test_violations" | sed 's/^/  - /'
            ((VIOLATIONS++))
            return 1
        fi
    else
        echo "✅ No violations found for: $pattern"
        return 0
    fi
}

# Core anti-patterns from anti.md, QUALITY.md and CLAUDE.md
echo "🚨 Checking core anti-patterns..."

scan_pattern "HashMap::new()" "HashMap initialization without capacity" "Use HashMap::with_capacity(n) for pre-allocation" "false"
scan_pattern "panic!(" "panic! usage" "Use Result<T, E> with custom error types" "true"
# Absolute ban: any unwrap* (includes unwrap_or*/default/else, unwrap_unchecked, unwrap_err, custom variants). No test exceptions.
scan_pattern "\\.unwrap[[:alnum:]_]*\\(" "unwrap* usage" "Use explicit match/if let/let-else or propagate with ? (no unwrap*)" "false"
# Absolute ban: any expect* (includes expect_unwrap variants). No test exceptions.
scan_pattern "\\.expect[[:alnum:]_]*\\(" "expect* usage" "Use explicit match/if let/let-else or propagate with ? (no expect*)" "false"
scan_pattern "unreachable!(" "unreachable! usage" "Use Result<T, E> with proper error handling" "true"
scan_pattern "unimplemented!(" "unimplemented! usage" "Implement the function or use todo!() during development" "true"
scan_pattern "BTreeMap::new()" "BTreeMap initialization without capacity" "Use BTreeMap::new() with proper sizing consideration" "false"
scan_pattern "todo!(" "todo! usage" "Complete implementation before production" "true"
scan_pattern "HashSet::new()" "HashSet initialization without capacity" "Use HashSet::with_capacity(n) for pre-allocation" "false"
scan_pattern "Vec::new()" "Vec initialization without capacity" "Use Vec::with_capacity(n) for pre-allocation" "false"
scan_pattern "eval(" "eval() function usage" "🚨 CRITICAL SECURITY VULNERABILITY - never use eval() in any context" "false"

echo "📋 Summary of scan results:"
echo "  - Test code exclusions applied per CLAUDE.md"
echo "  - unwrap*/expect* are banned everywhere (tests included)"
echo "  - Production code must use proper error handling"

echo ""
echo "🦀 Rust 1.91.1+ specific pattern validation..."

# Enhanced anti.md patterns (2025-01-08)
echo "🔍 Checking enhanced anti.md patterns..."
# (unwrap*/expect* absolute ban already enforced above)
# Missing error documentation in Result functions
echo "🔍 Checking for missing error documentation..."
result_functions=$(search_content "fn.*-> *Result" .)
if [[ -n "$result_functions" ]]; then
    echo -e "${BLUE}📋 Found $(echo "$result_functions" | wc -l) Result-returning functions${NC}"
    echo "⚠️  Manual review recommended for error documentation completeness"
fi

# Final summary
echo ""
if [[ $VIOLATIONS -eq 0 ]]; then
    echo -e "${GREEN}🎉 ALL ANTI-PATTERN CHECKS PASSED${NC}"
    echo "✅ Code quality standards maintained"
    echo "✅ Test code patterns are acceptable per CLAUDE.md"
else
    echo -e "${RED}🚫 QUALITY ENFORCEMENT FAILED${NC}"
    echo "Found $VIOLATIONS critical violations"
    echo "Fix all critical violations above before proceeding."
    exit 1
fi
