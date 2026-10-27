#!/bin/sh
set -eu
export PATH=/home/jack/.cache/rule-v4-candidates-scratch/o-astro-16079/bin:/home/jack/.local/share/mise/installs/node/24.21.0/bin:$PATH
CLONE=/home/jack/.cache/rule-v4-candidates-scratch/o-astro-16079/clone
EVIDENCE=/home/jack/.cache/regrade-v4/docs/research/rule-v4-candidates-2026-10-27/o-astro-16079/probes/M1
cd "$CLONE"
for name in classic edge slash platform; do
 mkdir -p "packages/integrations/vercel/test/fixtures/probe-v10-$name"
 cp -r "$EVIDENCE/fixture-$name/." "packages/integrations/vercel/test/fixtures/probe-v10-$name/"
 test -d "packages/integrations/vercel/test/fixtures/probe-v10-$name/node_modules" || cp -a packages/integrations/vercel/test/fixtures/isr/node_modules "packages/integrations/vercel/test/fixtures/probe-v10-$name/node_modules"
done
cp "$EVIDENCE/lib.mjs" packages/integrations/vercel/test/probe-r4-lib.mjs
cp "$EVIDENCE/cases.mjs" packages/integrations/vercel/test/probe-r4-cases.mjs
git checkout -q -- packages/integrations/vercel/src
git checkout -q 71ae513388df11d7dad6b1e0077c402ad03d0d62
python3 "$EVIDENCE/make-patches.py" "$CLONE" "$EVIDENCE/patches"
for state in base head fix-o1 fix-m1 fix-o1-m1; do
 git checkout -q -- packages/integrations/vercel/src
 if [ "$state" = base ]; then
  git checkout -q be661fb9fd1348ffb038f561f9f053b1c64a3696
 else
  git checkout -q 71ae513388df11d7dad6b1e0077c402ad03d0d62
 fi
 if [ "$state" != base ] && [ "$state" != head ]; then git apply "$EVIDENCE/patches/$state.patch"; fi
 pnpm -r --workspace-concurrency 2 --filter '@astrojs/vercel...' build:ci > "$EVIDENCE/build-$state.txt" 2>&1
 (
  git rev-parse HEAD
  if [ "$state" != base ] && [ "$state" != head ]; then sha256sum "$EVIDENCE/patches/$state.patch"; fi
  cd packages/integrations/vercel/test
  node probe-r4-cases.mjs M1 GT-o1 legit
 ) > "$EVIDENCE/result-$state.txt" 2>&1
 printf '%s complete\n' "$state"
done
