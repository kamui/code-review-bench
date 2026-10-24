#!/usr/bin/env bash
# Usage: probe.sh <scratch> <base|head>
# Probe C1nb. At one commit:
# 1. Runs the project's own bundle-size checker (test/bundle-size, the tool behind the
#    code-infra-dashboard "Bundle size" comment) for the tracked bundle `@base-ui/react` and the
#    entry points `@base-ui/react/field`, `@base-ui/react/input` and `@base-ui/react/form`.
# 2. Bundles each component the performance comment named, and each other benchmarked component,
#    with esbuild and lists whether the bundle includes the changed file
#    packages/react/src/field/control/FieldControl.tsx.
# Prints its results to stdout.
set -u
S="$1"
LABEL="$2"
CLONE="$S/source"
case "$LABEL" in
  base) SHA=30b8ea2004fa999bed151204208676c6c0a9d261 ;;
  head) SHA=14d39e5d1ad6b7aca2fb067415dba09c6bea219b ;;
  *) echo "unknown commit label $LABEL"; exit 1 ;;
esac
git -C "$CLONE" checkout -q --force --detach "$SHA" || exit 1
echo "# commit: $(git -C "$CLONE" rev-parse HEAD) ($LABEL)"
mkdir -p "$S/tmp"
SNAP="$S/tmp/size-$LABEL.json"
rm -f "$SNAP"
echo "## 1. bundle-size-checker"
echo "# command (in test/bundle-size): bundle-size-checker --output <tmp>/size-$LABEL.json --filter '@base-ui/reac[t]' '@base-ui/react/fiel[d]' '@base-ui/react/inpu[t]' '@base-ui/react/for[m]' --concurrency 2"
(cd "$CLONE/test/bundle-size" && env CI= TMPDIR="$S/tmp" NODE_OPTIONS=--max-old-space-size=4096 \
  ./node_modules/.bin/bundle-size-checker --output "$SNAP" \
  --filter '@base-ui/reac[t]' '@base-ui/react/fiel[d]' '@base-ui/react/inpu[t]' '@base-ui/react/for[m]' \
  --concurrency 2 2>&1) | rg -v 'Could not resolve peer dependencies' | sed -e "s#$S#<scratch>#g"
echo "# snapshot:"
python3 -I -c "
import json, sys
snap = json.load(open(sys.argv[1]))
for key in sorted(k for k in snap if not k.startswith('_')):
    print(f'SIZE {key} parsed={snap[key][\"parsed\"]} gzip={snap[key][\"gzip\"]}')
" "$SNAP"
echo "## 2. Does the component's bundle include field/control/FieldControl.tsx?"
ESBUILD="$CLONE/node_modules/.pnpm/esbuild@0.27.7/node_modules/esbuild/bin/esbuild"
for m in menu dialog tooltip popover select checkbox slider tabs scroll-area combobox input field; do
  META="$S/tmp/meta-$m-$LABEL.json"
  (cd "$CLONE" && "$ESBUILD" "packages/react/src/$m/index.ts" --bundle --format=esm \
    --external:react --external:react-dom --external:react/jsx-runtime \
    --metafile="$META" --outfile="$S/tmp/out-$m-$LABEL.js" --log-level=error)
  python3 -I -c "
import json, sys
inputs = json.load(open(sys.argv[1]))['inputs']
hit = any(k.endswith('field/control/FieldControl.tsx') for k in inputs)
print(f'INCLUDES {sys.argv[2]} FieldControl.tsx={hit} inputs={len(inputs)}')
" "$META" "$m"
done
git -C "$CLONE" checkout -q --force --detach 14d39e5d1ad6b7aca2fb067415dba09c6bea219b
