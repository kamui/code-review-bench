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
store="$scratch/store"
git clone -q --no-checkout "$repo" "$scratch/repo"
cd "$scratch/repo"
git -c advice.detachedHead=false checkout -q "$commit"
tool="python3 bench/tools/rust_runtime.py"
usage() {
    "$1" --help 2>&1 || true
}
echo "== install version one at $(git rev-parse HEAD)"
$tool install --crate rust/bench-tools --store "$store" --select > "$scratch/one.json"
one=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["pin"]["sha256"])' "$scratch/one.json")
echo "one: $one"
echo "== commit a second version that changes the help summary"
sed -i.bak 's|^/// Deterministic benchmark tools\.$|/// Deterministic benchmark tools (second version).|' rust/bench-tools/src/main.rs
rm rust/bench-tools/src/main.rs.bak
grep -q '(second version)' rust/bench-tools/src/main.rs || { echo "second version edit did not apply"; exit 1; }
git -c user.name=Evidence -c user.email=evidence@example.invalid commit -qam "evidence: second version"
$tool install --crate rust/bench-tools --store "$store" --select > "$scratch/two.json"
two=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["pin"]["sha256"])' "$scratch/two.json")
echo "two: $two"
echo "== selected before rollback"
$tool pin bench-tools --store "$store"
echo "== each retained version, run with --help"
usage "$store/bench-tools/$one/bench-tools"
usage "$store/bench-tools/$two/bench-tools"
usage "$store/bench-tools/$one/bench-tools" | grep -qx 'Deterministic benchmark tools' || { echo "version one lost its summary"; exit 1; }
usage "$store/bench-tools/$two/bench-tools" | grep -qx 'Deterministic benchmark tools (second version)' || { echo "version two lost its summary"; exit 1; }
echo "== rollback"
$tool rollback bench-tools --store "$store" > "$scratch/rollback.json"
python3 -c 'import json, sys; row = json.load(open(sys.argv[1])); print(row["action"], row["pin"]["sha256"], "previous", row["previous"]["sha256"])' "$scratch/rollback.json"
echo "== selected after rollback"
$tool pin bench-tools --store "$store" > "$scratch/pin.json"
cat "$scratch/pin.json"
selected=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["bench-tools"]["sha256"])' "$scratch/pin.json")
test "$selected" = "$one" || { echo "rollback selected $selected, not $one"; exit 1; }
echo "== both versions still verify"
$tool verify bench-tools "$one" --store "$store" > /dev/null
echo "one verifies"
$tool verify bench-tools "$two" --store "$store" > /dev/null
echo "two verifies"
echo "== retained entries"
ls "$store/bench-tools"
