#!/usr/bin/env bash
# Render docs/diagrams/*.puml to SVG with the PlantUML API server (PLANTUML_API_BASE,
# default https://plantuml.joor.net). `!include _style.iuml` is inlined first, since the
# server cannot read local files. Needs curl and jq.
set -euo pipefail
base="${PLANTUML_API_BASE:-https://plantuml.joor.net}"
dir="$(cd "$(dirname "$0")/../docs/diagrams" && pwd)"
for src in "$dir"/*.puml; do
  out="${src%.puml}.svg"
  body="$(awk -v style="$dir/_style.iuml" '
    /^!include _style\.iuml/ { while ((getline line < style) > 0) print line; close(style); next }
    { print }' "$src")"
  status=$(jq -Rs '{source: ., format: "svg"}' <<<"$body" |
    curl -sS -X POST "$base/api/v1/render" -H 'Content-Type: application/json' \
      --data-binary @- -o "$out.tmp" -w '%{http_code}')
  if [[ "$status" != 200 ]]; then
    echo "error: $(basename "$src"): HTTP $status: $(cat "$out.tmp")" >&2
    rm -f "$out.tmp"
    exit 1
  fi
  mv "$out.tmp" "$out"
  echo "rendered $(basename "$out")"
done
