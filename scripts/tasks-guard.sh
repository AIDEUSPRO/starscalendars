#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "$0")/.." && pwd)
cd "$root_dir"

fail() { echo "❌ tasks-guard: $*" >&2; exit 1; }

# 1) task.md must have Title and Selected-from marker; no duplicate Next Task Candidate sections
grep -q '^Title:' task.md || fail "task.md: missing Title:"
grep -q 'Selected from tasks-list.md' task.md || echo "⚠️ task.md: no 'Selected from tasks-list.md' marker (optional)"
if [ "$(grep -c '^Next Task Candidate (preview)' task.md || true)" -gt 1 ]; then
  fail "task.md: duplicate 'Next Task Candidate (preview)' sections"
fi

# 2) tasks_checked.md must contain latest Completed marker for last finished task
grep -q '— Completed' tasks_checked.md || echo "⚠️ tasks_checked.md: no '— Completed' marker (informational)"

# 4) Foundation basic checks marked done
grep -q '\[x\].*OpenAPI (utoipa) для core и ko' tasks-list.md || echo "⚠️ tasks-list.md: foundation OpenAPI not marked [x]"
grep -q '\[x\].*security headers' tasks-list.md || echo "⚠️ tasks-list.md: foundation security headers not marked [x]"

echo "✅ tasks-guard: OK"

