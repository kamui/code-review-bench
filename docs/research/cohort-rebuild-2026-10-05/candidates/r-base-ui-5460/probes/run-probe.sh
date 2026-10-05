#!/usr/bin/env bash
# Usage: run-probe.sh <GROUP> [jsdom|chromium] [probe file name, default probe.test.tsx]
# Runs probes/<GROUP>/probe.test.tsx inside the scratch clone at the commit before the
# change and at its head, and saves the observations next to the probe.
set -u
GROUP="$1"
ENVNAME="${2:-jsdom}"
PROBEFILE="${3:-probe.test.tsx}"
HERE="$(cd "$(dirname "$0")" && pwd)"
S=<scratch>
CLONE="$S/head"
BASE=30b8ea2004fa999bed151204208676c6c0a9d261
HEAD=14d39e5d1ad6b7aca2fb067415dba09c6bea219b
export COREPACK_HOME="$S/corepack" COREPACK_ENABLE_DOWNLOAD_PROMPT=0
DEST="$CLONE/packages/react/src/field/control/zzprobe.test.tsx"
SUFFIX=""
[ "$ENVNAME" != "jsdom" ] && SUFFIX="-$ENVNAME"
for pair in "base:$BASE" "head:$HEAD"; do
  label="${pair%%:*}"; sha="${pair##*:}"
  rm -f "$DEST"
  git -C "$CLONE" checkout -q --force "$sha" || exit 1
  cp "$HERE/$GROUP/$PROBEFILE" "$DEST"
  out="$HERE/$GROUP/result-$label$SUFFIX.txt"
  {
    echo "# commit: $(git -C "$CLONE" rev-parse HEAD) ($label)"
    echo "# environment: $ENVNAME"
    echo "# probe: $GROUP/$PROBEFILE (copied to packages/react/src/field/control/zzprobe.test.tsx)"
    echo "# command: VITEST_ENV=$ENVNAME TZ=UTC ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/zzprobe.test.tsx"
    (cd "$CLONE" && PROBE_LABEL="$label" VITEST_ENV="$ENVNAME" TZ=UTC NO_COLOR=1 ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/zzprobe.test.tsx 2>&1) \
      | sed -e "s#$S#<scratch>#g"
  } > "$out"
  rm -f "$DEST"
  echo "== $label =="; grep -E "PROBE|Tests |Test Files|FAIL|Error" "$out" | head -60
done
git -C "$CLONE" checkout -q --force "$HEAD"
