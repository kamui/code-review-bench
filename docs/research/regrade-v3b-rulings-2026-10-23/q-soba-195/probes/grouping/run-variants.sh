#!/usr/bin/env bash
# Usage: run-variants.sh SCRATCH
# Builds each fix variant from a fresh copy of the head, runs the failing cases of P1 and of each
# admitted pre-existing problem against it, and writes results/<variant>.txt and results/table.md.
#
# patches/fix-p1.patch is this group's smallest fix: the log's BitBucket gate uses the same two
# helpers as the backup run. The other patches are copied unchanged from the 2026-10-23 round
# (regrade-v3-rulings-2026-10-23/q-soba-195/probes/grouping/patches): fix-peq1-envtrue,
# fix-peq1-lfslines, fix-peq2, fix-peq3, fix-peq4 and fix-wide by their names, and fix-peq5.patch
# is that round's fix-p1.patch (PE-q5 was prepared there as P1).
set -eu
S=$1
HERE=$(cd "$(dirname "$0")" && pwd)
H="$HERE/../harness"
CASES=(P1-oauth-complete P1-email-only-with-github P1-email-only-alone P1-apitoken-complete
       PEq5-token-misnamed-with-github PEq5-oauth-partial-with-github
       PEq1-file PEq2-file-only PEq3-compare-file PEq4-gitea-url-only)
V="$S/variants.$(date +%s%N)"
mkdir -p "$V" "$HERE/results"

fresh() { mkdir "$V/$1"; (cd "$S/head" && git archive HEAD) | tar -x -C "$V/$1"; }
apply() { patch -s -p1 -d "$V/$1" < "$HERE/patches/$2.patch"; }

for p in fix-p1 fix-peq5 fix-peq1-envtrue fix-peq1-lfslines fix-peq2 fix-peq3 fix-peq4 fix-wide; do
  fresh "$p"; apply "$p" "$p"
done
fresh both-peq5+p1; apply both-peq5+p1 fix-peq5; apply both-peq5+p1 fix-p1

for v in fix-p1 fix-peq5 both-peq5+p1 fix-peq1-envtrue fix-peq1-lfslines fix-peq2 fix-peq3 fix-peq4 fix-wide; do
  bash "$H/drive.sh" "$S" "$v" "$V/$v" "$HERE/results/$v.txt" "${CASES[@]}"
done
bash "$H/drive.sh" "$S" head "$S/head" "$HERE/results/head.txt" "${CASES[@]}"
python3 -I "$HERE/table.py" "$HERE/results" > "$HERE/results/table.md"
