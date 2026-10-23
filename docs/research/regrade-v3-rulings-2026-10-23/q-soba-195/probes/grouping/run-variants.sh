#!/usr/bin/env bash
# Usage: run-variants.sh SCRATCH
# Builds each variant made by make_variants.py from a fresh copy of the head, runs the failing
# cases of every group and every admitted pre-existing problem it is tested against, saves the
# outputs under results/ and the patches under patches/, and writes results/table.md.
set -eu
S=$1
HERE=$(cd "$(dirname "$0")" && pwd)
H="$HERE/../harness"
CASES=(P1-bb-email-only-with-github P1-bb-oauth-partial-with-github P1-control-github-only
       P2-newline P2-control-plain PEq2-file-only PEq4-gitea-url-only PEq1-file C1n-skip-file)
OUTV="$S/variants.$(date +%s%N)"
mkdir -p "$HERE/results" "$HERE/patches"
python3 -I "$HERE/make_variants.py" "$S/head" "$OUTV" > /dev/null
for dir in "$OUTV"/*; do
  v=$(basename "$dir")
  (cd "$S" && diff -ru --color=never --exclude=.git "head" "$(basename "$OUTV")/$v" || true) \
    | sed -E 's/^(---|\+\+\+) ([^\t]*)\t.*/\1 \2/; s#variants\.[0-9]+/#variants/#g' > "$HERE/patches/$v.patch"
  bash "$H/drive.sh" "$S" "$v" "$dir" "$HERE/results/$v.txt" "${CASES[@]}"
done
bash "$H/drive.sh" "$S" head "$S/head" "$HERE/results/head.txt" "${CASES[@]}"
python3 -I "$HERE/table.py" "$HERE/results" > "$HERE/results/table.md"
