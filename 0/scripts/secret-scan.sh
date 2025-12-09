#!/bin/bash
# Fast secret scanning (baseline)
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

VIOLATIONS=0
EXCLUDES=(--exclude-dir=.git --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist --exclude='*.lock' --exclude='package-lock.json')
RG_GLOBS=(
  -g '!.git/**'
  -g '!target/**'
  -g '!node_modules/**'
  -g '!dist/**'
  -g '!**/.env.local'
  -g '!docs/**'
  -g '!deployment/**'
  -g '!_n8n-mcp-reference/**'
  -g '!MCP_REFERENCE.md'
  -g '!**/*.lock'
  -g '!**/package-lock.json'
)

run_rg() {
  if command -v rg >/dev/null 2>&1; then
    rg -n --color=never -uu ${RG_GLOBS[@]} -e "$1" . || true
  else
    grep -rn ${EXCLUDES[@]} -- "$1" . 2>/dev/null || true
  fi
}

echo "🔐 Secret scan (baseline)..."
aws_id=$(run_rg "AKIA[0-9A-Z]{16}")
aws_secret=$(run_rg "(?i)aws(.{0,20})?(secret|access).{0,20}?[A-Za-z0-9/+=]{40}")
priv_keys=$(run_rg "-----BEGIN (RSA|EC|DSA|OPENSSH|PGP) PRIVATE KEY-----")
# Use a PCRE2-free heuristic: look for token/apikey/api_key/secret plus a separator and a 16+ length value,
# then post-filter out common safe placeholders like your_ or YOUR_
tokens_all=$(run_rg "(?i)(token|apikey|api_key|secret)[\s:=]+[A-Za-z0-9\-_\.]{16,}")
tokens=$(echo "$tokens_all" | grep -Ev '(your_|YOUR_)' || true)
bearer=$(run_rg "(?i)authorization:\s*bearer\s+[A-Za-z0-9\-_\.]{16,}")
url_creds=$(run_rg "https?://[^\s/]+:[^\s/]+@[^\s]+")

report() { local name="$1"; local matches="$2"; if [[ -n "$matches" ]]; then echo -e "${RED}❌ Potential secret: $name${NC}"; echo "$matches" | sed 's/^/  - /' | head -20; VIOLATIONS=$((VIOLATIONS+1)); else echo -e "${GREEN}✅ $name: clean${NC}"; fi }

report "AWS_ACCESS_KEY_ID" "$aws_id"
report "AWS_SECRET_ACCESS_KEY" "$aws_secret"
report "Private Keys" "$priv_keys"
report "Generic tokens" "$tokens"
report "Bearer tokens" "$bearer"
report "URL with credentials" "$url_creds"

if [[ $VIOLATIONS -gt 0 ]]; then echo -e "${RED}🚫 Secret scan found $VIOLATIONS potential issues${NC}"; exit 1; else echo -e "${GREEN}🎉 Secret scan passed${NC}"; fi
