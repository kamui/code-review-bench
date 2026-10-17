#!/bin/sh
# Checks out one commit in the scratch clone, applies zero or more patches, rebuilds the adapter and its
# workspace dependencies, and runs cases.mjs. Prints a header that pins the state, then the probe output.
# usage: run-state.sh <clone> <probes/shared directory> <label> <commit> <patch[,patch...] or -> [tags...]
set -eu
CLONE=$1; SHARED=$2; LABEL=$3; COMMIT=$4; PATCHES=$5; shift 5
cd "$CLONE"
git checkout -q -- packages/integrations/vercel/src
git checkout -q "$COMMIT"
if [ "$PATCHES" != "-" ]; then
	for p in $(echo "$PATCHES" | tr ',' ' '); do git apply "$SHARED/patches/$p"; done
fi
pnpm -r --workspace-concurrency 2 --filter '@astrojs/vercel...' build:ci >/dev/null 2>&1
echo "# state: $LABEL"
echo "# checked-out commit: $(git rev-parse HEAD)"
if [ "$PATCHES" != "-" ]; then
	for p in $(echo "$PATCHES" | tr ',' ' '); do echo "# patch applied: $p (sha256 $(sha256sum "$SHARED/patches/$p" | cut -d' ' -f1))"; done
else echo "# patch applied: none"; fi
echo "# entrypoint.ts blob in working tree: $(git hash-object packages/integrations/vercel/src/serverless/entrypoint.ts)"
echo "# index.ts blob in working tree: $(git hash-object packages/integrations/vercel/src/index.ts)"
echo
sh "$SHARED/run.sh" "$CLONE" "$SHARED" "$@"
git -C "$CLONE" checkout -q -- packages/integrations/vercel/src
