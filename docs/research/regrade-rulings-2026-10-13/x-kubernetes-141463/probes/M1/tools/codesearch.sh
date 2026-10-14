#!/bin/bash
# usage: codesearch.sh <n> <query>   -> saves upstream/M1-code-q<n>.json, retries when rate-limited
n=$1; q=$2; out=M1-code-q$n.json
for attempt in 1 2 3 4; do
  if gh api -X GET search/code -f q="$q" -f per_page=100 > $out 2> $out.err; then
    python3 - "$out" "$q" <<'PY'
import json,sys
d=json.load(open(sys.argv[1]))
print('Q', repr(sys.argv[2]), 'total', d['total_count'], 'returned', len(d['items']), 'incomplete', d.get('incomplete_results'))
for it in d['items'][:60]:
    print('   ', it['repository']['full_name'], it['path'])
PY
    rm -f $out.err; exit 0
  fi
  echo "attempt $attempt failed: $(head -c 300 $out.err)"; sleep 65
done
echo BLOCKED; exit 1
