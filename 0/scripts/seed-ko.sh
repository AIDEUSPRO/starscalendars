#!/usr/bin/env bash
set -euo pipefail

KO_BASE=${KO_BASE:-"http://127.0.0.1:8082"}

echo "Seeding KO sources via ${KO_BASE}..."
curl -s -X POST "${KO_BASE}/admin/ingest/batch" -H 'content-type: application/json' -d '[
  {"type":"url","url":"https://kb.ai1c.pro/1c/bsl/handlers","title":"1C BSL: Обработчики событий","tags":["BSL"]},
  {"type":"url","url":"https://kb.ai1c.pro/rust/anti-patterns","title":"Rust Anti-patterns 2025","tags":["Dev:Rust","Dev:AntiPatterns"]}
]' | cat
echo
echo "Done. Reindex via KO→n8n webhook if needed."
