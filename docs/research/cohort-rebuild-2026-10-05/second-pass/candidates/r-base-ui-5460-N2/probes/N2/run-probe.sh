#!/usr/bin/env bash
# Usage: run-probe.sh <scratch> [jsdom|chromium] [probe file, default probe.test.tsx] [result prefix, default result]
# Runs the probe inside <scratch>/source at the commit before the change and at its head,
# in the project's own test setup, and saves what it printed beside the probe.
set -u
S="$1"
ENVNAME="${2:-jsdom}"
PROBEFILE="${3:-probe.test.tsx}"
PREFIX="${4:-result}"
HERE="$(cd "$(dirname "$0")" && pwd)"
CLONE="$S/source"
BASE=30b8ea2004fa999bed151204208676c6c0a9d261
HEAD=14d39e5d1ad6b7aca2fb067415dba09c6bea219b
DEST="$CLONE/packages/react/src/field/control/zzprobe.test.tsx"
SUFFIX=""
[ "$ENVNAME" != "jsdom" ] && SUFFIX="-$ENVNAME"
mkdir -p "$S/tmp"
for pair in "base:$BASE" "head:$HEAD"; do
  label="${pair%%:*}"; sha="${pair##*:}"
  rm -f "$DEST"
  git -C "$CLONE" checkout -q --force --detach "$sha" || exit 1
  cp "$HERE/$PROBEFILE" "$DEST"
  out="$HERE/$PREFIX-$label$SUFFIX.txt"
  {
    echo "# commit: $(git -C "$CLONE" rev-parse HEAD) ($label)"
    echo "# environment: $ENVNAME"
    echo "# probe: probes/N2/$PROBEFILE (copied to packages/react/src/field/control/zzprobe.test.tsx)"
    echo "# command: VITEST_ENV=$ENVNAME TZ=UTC ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/zzprobe.test.tsx"
    (cd "$CLONE" && VITEST_ENV="$ENVNAME" TZ=UTC NO_COLOR=1 TMPDIR="$S/tmp" \
      PLAYWRIGHT_BROWSERS_PATH="$HOME/.cache/ms-playwright" \
      ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/zzprobe.test.tsx 2>&1) \
      | sed -e "s#$S#<scratch>#g" -e "s#$HOME#<home>#g"
  } > "$out"
  rm -f "$DEST"
  echo "== $label ($ENVNAME): $(grep -c 'PROBE ' "$out") PROBE lines; $(grep -E 'Tests ' "$out" | tr -s ' ')"
done
git -C "$CLONE" checkout -q --force --detach "$HEAD"
