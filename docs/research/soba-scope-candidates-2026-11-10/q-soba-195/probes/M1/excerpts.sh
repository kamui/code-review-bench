#!/bin/sh
# Usage: excerpts.sh <soba-clone>
# Prints, with line numbers, the code the M1 dossier and case quote at both commits.
set -eu
R=$1
B=c77f548cbd340d2e744da7c6d92a54372b4b900b
H=136a4850df9aec8cf8813b27ba1533d4a17bc642
show() { echo "### $1 $2 lines $3-$4"; git -C "$R" show "$1:$2" | awk -v a="$3" -v b="$4" 'NR>=a && NR<=b {printf "%4d  %s\n", NR, $0}'; echo; }
show $B internal/backup.go 179 288   # displayStartupConfig, base
show $B internal/backup.go 316 368   # checkProvider, base
show $H internal/backup.go 92 138    # collectProviderBackupResults (table) and BitBucket helpers, head
show $H internal/backup.go 190 294   # displayStartupConfig, helpers and the five per-provider functions, head
show $H internal/backup.go 322 395   # checkProvider, checkJustTokenProvider, checkUserAndPasswordProvider, head
show $H internal/constants.go 120 159  # provider tables, identical at both commits
echo "### constants.go identical at both commits:"
if git -C "$R" diff --quiet $B $H -- internal/constants.go; then echo yes; else echo no; fi
echo "### git diff $B $H -- internal/backup.go (hunks for displayStartupConfig and checkProvider)"
git -C "$R" diff --no-color $B $H -- internal/backup.go | awk '/^@@ -181,110/{p=1} /^@@ -378,6/{p=0} p'
