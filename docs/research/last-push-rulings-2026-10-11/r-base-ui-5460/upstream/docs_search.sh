#!/usr/bin/env bash
# Usage: docs_search.sh <clone at head> <output file> <rg pattern> <path under the clone>...
# Searches the project's documentation at the pinned head and saves the command, the hit count and the hits.
set -u
CLONE="$1"; OUT="$2"; PATTERN="$3"; shift 3
{
  echo "# commit: $(git -C "$CLONE" rev-parse HEAD)"
  echo "# command (run inside the clone): rg -n -i -e '$PATTERN' -g '*.mdx' -g '*.md' $*"
  hits=$(cd "$CLONE" && rg -n -i -e "$PATTERN" -g '*.mdx' -g '*.md' "$@" | wc -l)
  echo "# hits: $hits"
  (cd "$CLONE" && rg -n -i -e "$PATTERN" -g '*.mdx' -g '*.md' "$@" | cut -c1-700)
  echo
} >> "$OUT"
