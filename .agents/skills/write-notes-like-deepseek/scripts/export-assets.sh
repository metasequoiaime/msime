#!/usr/bin/env bash
# Export every assets/*.svg to a @2x PNG next to it (SVG is the source of truth).
# Usage: bash scripts/export-assets.sh            # all SVGs
#        bash scripts/export-assets.sh hero       # just hero.svg
set -euo pipefail

cd "$(dirname "$0")/.."

EXPORTER=""
if command -v rsvg-convert >/dev/null 2>&1; then
  EXPORTER="rsvg-convert"
elif command -v inkscape >/dev/null 2>&1; then
  EXPORTER="inkscape"
else
  echo "error: need rsvg-convert (brew install librsvg) or inkscape to export PNGs" >&2
  exit 1
fi

filter="${1:-}"
failed=0
for svg in assets/*.svg; do
  base="$(basename "$svg" .svg)"
  if [ -n "$filter" ] && [ "$base" != "$filter" ]; then
    continue
  fi
  png="assets/${base}.png"
  case "$EXPORTER" in
    rsvg-convert)
      rsvg-convert -w 2800 -o "$png" "$svg" ;;
    inkscape)
      inkscape "$svg" -w 2800 -o "$png" >/dev/null ;;
  esac
  if [ -s "$png" ]; then
    echo "ok: $svg -> $png"
  else
    echo "FAILED: $svg" >&2
    failed=1
  fi
done
exit "$failed"
