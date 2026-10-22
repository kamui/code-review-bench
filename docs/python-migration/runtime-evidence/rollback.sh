#!/bin/sh
# Install two versions of bench-tools into a new store, select the second, roll back, and
# verify both retained executables. Nothing outside SCRATCH is written.
# Usage: docs/python-migration/runtime-evidence/rollback.sh SCRATCH COMMIT
# SCRATCH must not exist. COMMIT is the revision to clone; dependencies must already be fetched.
set -eu
scratch=$1
commit=$2
repo=$(git rev-parse --show-toplevel)
mkdir "$scratch"
BENCH_RUNTIME_STORE="$scratch/store"
export BENCH_RUNTIME_STORE
store=$BENCH_RUNTIME_STORE
git clone -q --no-checkout "$repo" "$scratch/repo"
cd "$scratch/repo"
git -c advice.detachedHead=false checkout -q "$commit"
tool=bench/tools/bench-tools
usage() {
    "$1" --help 2>&1 || true
}
# The first "sha256" a command prints: the selected or installed pin, whose keys are sorted.
sha256() {
    sed -n 's/^ *"sha256": "\([0-9a-f]\{64\}\)",\{0,1\}$/\1/p' "$1" | head -n 1
}
echo "== install version one at $(git rev-parse HEAD)"
$tool runtime install --select > "$scratch/one.json"
one=$(sha256 "$scratch/one.json")
echo "one: $one"
echo "== commit a second version that changes the help summary"
sed -i.bak 's|^/// Deterministic benchmark tools\.$|/// Deterministic benchmark tools (second version).|' rust/bench-tools/src/main.rs
rm rust/bench-tools/src/main.rs.bak
grep -q '(second version)' rust/bench-tools/src/main.rs || { echo "second version edit did not apply"; exit 1; }
git -c user.name=Evidence -c user.email=evidence@example.invalid commit -qam "evidence: second version"
$tool runtime install --select > "$scratch/two.json"
two=$(sha256 "$scratch/two.json")
echo "two: $two"
echo "== selected before rollback"
$tool runtime pin
echo "== each retained version, run with --help"
usage "$store/bench-tools/$one/bench-tools"
usage "$store/bench-tools/$two/bench-tools"
usage "$store/bench-tools/$one/bench-tools" | grep -qx 'Deterministic benchmark tools' || { echo "version one lost its summary"; exit 1; }
usage "$store/bench-tools/$two/bench-tools" | grep -qx 'Deterministic benchmark tools (second version)' || { echo "version two lost its summary"; exit 1; }
echo "== rollback"
$tool runtime rollback > "$scratch/rollback.json"
grep -q '"action": "rollback"' "$scratch/rollback.json" || { echo "rollback recorded no rollback"; exit 1; }
echo "rollback to $(sha256 "$scratch/rollback.json")"
echo "== selected after rollback"
$tool runtime pin > "$scratch/pin.json"
cat "$scratch/pin.json"
selected=$(sha256 "$scratch/pin.json")
test "$selected" = "$one" || { echo "rollback selected $selected, not $one"; exit 1; }
echo "== the selection runs only from the commit it was built from"
if $tool roster > /dev/null 2> "$scratch/stale.txt"; then
    echo "version one ran at a HEAD it was not built from"; exit 1
fi
grep -q 'was built from crate tree' "$scratch/stale.txt" || { cat "$scratch/stale.txt"; exit 1; }
echo "refused as expected"
echo "== both versions still verify"
$tool runtime verify "$one" > /dev/null
echo "one verifies"
$tool runtime verify "$two" > /dev/null
echo "two verifies"
echo "== retained entries"
ls "$store/bench-tools"
