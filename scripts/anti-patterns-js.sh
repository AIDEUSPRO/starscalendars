#!/bin/bash
# JS/TS Anti-pattern scanning (frontend/dioxus-app)
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

VIOLATIONS=0
RG_GLOBS=(
  -g '!.git/**'
  -g '!node_modules/**'
  -g '!dist/**'
  -g '!build/**'
)

has_js_sources() {
  find . \( -name "*.js" -o -name "*.ts" -o -name "*.tsx" \) -not -path "./node_modules/*" -not -path "./dist/*" -not -path "./build/*" | head -n1 | wc -l | tr -d ' '
}

search() {
  local pattern="$1"
  if command -v rg >/dev/null 2>&1; then
    rg -n --color=never -uu ${RG_GLOBS[@]} -e "$pattern" -- '*.js' '*.ts' '*.tsx' || true
  else
    find . \
      -type f \( -name "*.js" -o -name "*.ts" -o -name "*.tsx" \) \
      -not -path "./.git/*" \
      -not -path "./node_modules/*" \
      -not -path "./dist/*" \
      -not -path "./build/*" \
      -print0 2>/dev/null | xargs -0 grep -n -- "$pattern" 2>/dev/null || true
  fi
}

if [ "$(has_js_sources)" -eq 0 ]; then
  echo "${YELLOW}⚠️ No JS/TS sources found — skipping JS anti-pattern scan${NC}"
  exit 0
fi

echo "🔍 JS/TS anti-patterns..."

# 1) eval/new Function
matches=$(search "(^|[^a-zA-Z0-9_])eval\s*\(|new\s+Function\s*\(")
if [[ -n "$matches" ]]; then
  echo -e "${RED}❌ eval()/new Function detected${NC}"
  echo "$matches" | sed 's/^/  - /' | head -30
  ((VIOLATIONS++))
else
  echo -e "${GREEN}✅ No eval()/new Function${NC}"
fi

# 2) document.write
matches=$(search "document\.write\s*\(")
if [[ -n "$matches" ]]; then
  echo -e "${RED}❌ document.write detected${NC}"
  echo "$matches" | sed 's/^/  - /' | head -30
  ((VIOLATIONS++))
else
  echo -e "${GREEN}✅ No document.write${NC}"
fi

# 3) innerHTML / dangerouslySetInnerHTML
matches=$(search "innerHTML\s*=|dangerouslySetInnerHTML")
if [[ -n "$matches" ]]; then
  echo -e "${RED}❌ innerHTML/dangerouslySetInnerHTML detected (potential XSS)${NC}"
  echo "$matches" | sed 's/^/  - /' | head -30
  ((VIOLATIONS++))
else
  echo -e "${GREEN}✅ No innerHTML/dangerouslySetInnerHTML${NC}"
fi

# 4) @ts-ignore without justification (next token is not an explanation)
matches=$(search "@ts-ignore(?!.*reason|.*why|.*ticket|.*issue)")
if [[ -n "$matches" ]]; then
  echo -e "${YELLOW}⚠️ @ts-ignore without justification${NC}"
  echo "$matches" | sed 's/^/  - /' | head -30
  ((VIOLATIONS++))
else
  echo -e "${GREEN}✅ @ts-ignore usage looks justified or absent${NC}"
fi

# 5) any usage (exclude .d.ts)
if command -v rg >/dev/null 2>&1; then
  matches=$(rg -n --color=never -uu ${EXCLUDES[@]} -g '!**/*.d.ts' -e "\bany\b" -- '*.ts' '*.tsx' || true)
else
  matches=$(grep -rn ${EXCLUDES[@]} --include="*.ts" --include="*.tsx" --exclude="*.d.ts" -- "\bany\b" . 2>/dev/null || true)
fi
if [[ -n "$matches" ]]; then
  echo -e "${YELLOW}⚠️ 'any' type usage detected (review for necessity)${NC}"
  echo "$matches" | sed 's/^/  - /' | head -30
  ((VIOLATIONS++))
else
  echo -e "${GREEN}✅ No 'any' usage (excluding d.ts)${NC}"
fi

if [[ $VIOLATIONS -gt 0 ]]; then
  echo -e "${RED}🚫 JS/TS anti-patterns found: $VIOLATIONS${NC}"
  exit 1
else
  echo -e "${GREEN}🎉 JS/TS anti-pattern scan passed${NC}"
fi
