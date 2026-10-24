#!/usr/bin/env bash
# Usage: run-probe.sh <scratch> <GROUP> [jsdom|chromium] [probe file, default probe.test.tsx] [result prefix, default result] [patch file, optional] [commits, default "base head"]
# Runs probes/<GROUP>/<probe file> inside <scratch>/source, in the project's own test setup, at the
# commit before the change (base) and at the reviewed head. With a patch file, the patch is applied
# to the checked-out commit before the run and removed after it. Saves what each run printed beside
# the probe. Copied from docs/research/regrade-v10-rulings-2026-10-17/r-base-ui-5460/probes/run-probe.sh.
set -u
S="$1"
GROUP="$2"
ENVNAME="${3:-jsdom}"
PROBEFILE="${4:-probe.test.tsx}"
PREFIX="${5:-result}"
PATCH="${6:-}"
COMMITS="${7:-base head}"
HERE="$(cd "$(dirname "$0")" && pwd)"
CLONE="$S/source"
BASE=30b8ea2004fa999bed151204208676c6c0a9d261
HEAD=14d39e5d1ad6b7aca2fb067415dba09c6bea219b
DEST="$CLONE/packages/react/src/field/control/zzprobe.test.tsx"
SUFFIX=""
[ "$ENVNAME" != "jsdom" ] && SUFFIX="-$ENVNAME"
mkdir -p "$S/tmp"
for label in $COMMITS; do
  case "$label" in
    base) sha="$BASE" ;;
    head) sha="$HEAD" ;;
    *) echo "unknown commit label $label"; exit 1 ;;
  esac
  rm -f "$DEST"
  git -C "$CLONE" checkout -q --force --detach "$sha" || exit 1
  if [ -n "$PATCH" ]; then
    git -C "$CLONE" apply "$HERE/$GROUP/$PATCH" || { echo "patch $PATCH does not apply at $label"; exit 1; }
  fi
  cp "$HERE/$GROUP/$PROBEFILE" "$DEST"
  out="$HERE/$GROUP/$PREFIX-$label$SUFFIX.txt"
  {
    echo "# commit: $(git -C "$CLONE" rev-parse HEAD) ($label)"
    [ -n "$PATCH" ] && echo "# patch applied on top: probes/$GROUP/$PATCH"
    echo "# environment: $ENVNAME"
    echo "# probe: probes/$GROUP/$PROBEFILE (copied to packages/react/src/field/control/zzprobe.test.tsx)"
    echo "# command: VITEST_ENV=$ENVNAME TZ=UTC ./node_modules/.bin/vitest run --project @base-ui/react --maxWorkers=1 packages/react/src/field/control/zzprobe.test.tsx"
    (cd "$CLONE" && VITEST_ENV="$ENVNAME" TZ=UTC NO_COLOR=1 TMPDIR="$S/tmp" \
      PLAYWRIGHT_BROWSERS_PATH="$HOME/.cache/ms-playwright" \
      ./node_modules/.bin/vitest run --project @base-ui/react --maxWorkers=1 packages/react/src/field/control/zzprobe.test.tsx 2>&1) \
      | sed -e "s#$S#<scratch>#g" -e "s#$HOME#<home>#g"
  } > "$out"
  rm -f "$DEST"
  git -C "$CLONE" checkout -q --force --detach "$sha"
  echo "== $GROUP $PROBEFILE $label ($ENVNAME${PATCH:+, $PATCH}): $(grep -c 'PROBE ' "$out") PROBE lines; $(grep -E 'Tests ' "$out" | tr -s ' ')"
done
git -C "$CLONE" checkout -q --force --detach "$HEAD"
