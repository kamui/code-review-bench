#!/bin/sh
# Install two versions of usage-pilot into a new store, select the second, roll back, and run the
# behavior corpus with each retained executable. Nothing outside SCRATCH is written.
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
corpus() {
    python3 - "$1" > "$scratch/map.json" <<'PY'
import json, os, sys
adapter = os.path.abspath("tools/usage_pilot/adapter.py")
print(json.dumps({"codex": [sys.executable, adapter, "codex", "--engine", "rust", "--binary", sys.argv[1]],
                  "transcript": [sys.executable, adapter, "transcript"]}))
PY
    python3 tools/behavior_contract.py --implementations "$scratch/map.json" > "$scratch/corpus.log" 2>&1
    echo "corpus: $(grep -c '^PASS' "$scratch/corpus.log") passed, $(grep -c '^FAIL' "$scratch/corpus.log" || true) failed"
}
usage() {
    "$1" 2>&1 || true
}
echo "== install version one at $(git rev-parse HEAD)"
$tool install --crate rust/usage-pilot --store "$store" --select > "$scratch/one.json"
one=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["pin"]["sha256"])' "$scratch/one.json")
echo "one: $one"
echo "== commit a second version that changes the usage message"
sed -i.bak 's/"usage-pilot PATH IN_PRICE/"usage-pilot (second version) PATH IN_PRICE/' rust/usage-pilot/src/main.rs
rm rust/usage-pilot/src/main.rs.bak
git -c user.name=Evidence -c user.email=evidence@example.invalid commit -qam "evidence: second version"
$tool install --crate rust/usage-pilot --store "$store" --select > "$scratch/two.json"
two=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["pin"]["sha256"])' "$scratch/two.json")
echo "two: $two"
echo "== selected before rollback"
$tool pin usage-pilot --store "$store"
echo "== each retained version, run without arguments"
usage "$store/usage-pilot/$one/usage-pilot"
usage "$store/usage-pilot/$two/usage-pilot"
usage "$store/usage-pilot/$one/usage-pilot" | grep -q '"usage-pilot PATH' || { echo "version one lost its message"; exit 1; }
usage "$store/usage-pilot/$two/usage-pilot" | grep -q 'second version' || { echo "version two lost its message"; exit 1; }
echo "== corpus, version one"
corpus "$store/usage-pilot/$one/usage-pilot"
echo "== corpus, version two"
corpus "$store/usage-pilot/$two/usage-pilot"
echo "== rollback"
$tool rollback usage-pilot --store "$store" > "$scratch/rollback.json"
python3 -c 'import json, sys; row = json.load(open(sys.argv[1])); print(row["action"], row["pin"]["sha256"], "previous", row["previous"]["sha256"])' "$scratch/rollback.json"
echo "== selected after rollback"
$tool pin usage-pilot --store "$store" > "$scratch/pin.json"
cat "$scratch/pin.json"
selected=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["usage-pilot"]["sha256"])' "$scratch/pin.json")
test "$selected" = "$one" || { echo "rollback selected $selected, not $one"; exit 1; }
echo "== both versions still verify"
$tool verify usage-pilot "$one" --store "$store" > /dev/null
echo "one verifies"
$tool verify usage-pilot "$two" --store "$store" > /dev/null
echo "two verifies"
echo "== retained entries"
ls "$store/usage-pilot"
