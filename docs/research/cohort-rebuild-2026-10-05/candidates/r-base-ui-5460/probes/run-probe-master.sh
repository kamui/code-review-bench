#!/usr/bin/env bash
# Usage: run-probe-master.sh <GROUP> [jsdom|chromium] [probe file name, default probe.test.tsx]
# Runs the same probe at upstream master as fetched on 2026-10-05, to record what the
# maintainers have left in place since. Output: probes/<GROUP>/result-master.txt
set -u
GROUP="$1"
ENVNAME="${2:-jsdom}"
PROBEFILE="${3:-probe.test.tsx}"
HERE="$(cd "$(dirname "$0")" && pwd)"
S=<scratch>
CLONE="$S/master"
DEST="$CLONE/packages/react/src/field/control/zzprobe.test.tsx"
SUFFIX=""
[ "$ENVNAME" != "jsdom" ] && SUFFIX="-$ENVNAME"
cp "$HERE/$GROUP/$PROBEFILE" "$DEST"
out="$HERE/$GROUP/result-master$SUFFIX.txt"
{
  echo "# commit: $(git -C "$CLONE" rev-parse HEAD) (upstream master, fetched 2026-10-05)"
  echo "# environment: $ENVNAME"
  echo "# probe: $GROUP/$PROBEFILE (copied to packages/react/src/field/control/zzprobe.test.tsx)"
  echo "# command: VITEST_ENV=$ENVNAME TZ=UTC ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/zzprobe.test.tsx"
  (cd "$CLONE" && VITEST_ENV="$ENVNAME" TZ=UTC NO_COLOR=1 ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/zzprobe.test.tsx 2>&1) \
    | sed -e "s#$S#<scratch>#g"
} > "$out"
rm -f "$DEST"
echo "== master =="; grep -E "PROBE|Tests |Test Files|FAIL|Error" "$out" | head -60
