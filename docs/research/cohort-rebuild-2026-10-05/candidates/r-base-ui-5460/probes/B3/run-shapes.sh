#!/usr/bin/env bash
# Usage: run-shapes.sh [jsdom|chromium]
# Runs probe-shapes.test.tsx at the commit before the change and at its head.
# Output: result-shapes-base.txt, result-shapes-head.txt (suffix -chromium for a browser run).
# Vitest reports an unhandled promise rejection as a run error; the flag below keeps the run
# going so every case prints, and the rejection text stays in the output.
set -u
ENVNAME="${1:-jsdom}"
HERE="$(cd "$(dirname "$0")" && pwd)"
S=<scratch>
SUFFIX=""
[ "$ENVNAME" != "jsdom" ] && SUFFIX="-$ENVNAME"
run() {
  label="$1"; clone="$2"; note="$3"
  dest="$clone/packages/react/src/field/control/zzprobe.test.tsx"
  cp "$HERE/probe-shapes.test.tsx" "$dest"
  out="$HERE/result-shapes-$label$SUFFIX.txt"
  {
    echo "# commit: $(git -C "$clone" rev-parse HEAD) ($note)"
    echo "# environment: $ENVNAME"
    echo "# probe: B3/probe-shapes.test.tsx (copied to packages/react/src/field/control/zzprobe.test.tsx)"
    echo "# command: VITEST_ENV=$ENVNAME TZ=UTC ./node_modules/.bin/vitest run --dangerouslyIgnoreUnhandledErrors --project @base-ui/react packages/react/src/field/control/zzprobe.test.tsx"
    (cd "$clone" && VITEST_ENV="$ENVNAME" TZ=UTC NO_COLOR=1 ./node_modules/.bin/vitest run --dangerouslyIgnoreUnhandledErrors --project @base-ui/react packages/react/src/field/control/zzprobe.test.tsx 2>&1) \
      | sed -e "s#$S#<scratch>#g"
  } > "$out"
  rm -f "$dest"
  echo "== $label: $(grep -c '^PROBE\|PROBE ' "$out") PROBE lines; $(grep -E 'Tests ' "$out" | tr -s ' ')"
}
BASE=30b8ea2004fa999bed151204208676c6c0a9d261
HEAD=14d39e5d1ad6b7aca2fb067415dba09c6bea219b
git -C "$S/head" checkout -q --force "$BASE" && run base "$S/head" "base, before the change"
git -C "$S/head" checkout -q --force "$HEAD" && run head "$S/head" "head, the pull request"
if [ -d "$S/master/node_modules" ] && [ "$ENVNAME" = "jsdom" ]; then
  run master "$S/master" "upstream master, fetched 2026-10-05"
fi
