#!/bin/sh
# Usage: run_all.sh <django checkout> <python> <sock dir> <port>   (writes to stdout)
HERE=$(cd "$(dirname "$0")" && pwd)
echo "===== PART 1: fork_probe.py (a process that used the database forks workers) ====="
for mode in closed left-open pool; do
  PYTHONPATH="$1" timeout 150 "$2" "$HERE/fork_probe.py" $mode "$3" "$4" 2>&1 | grep -v -e DeprecationWarning -e 'pid = os.fork()' | cut -c1-300
  echo
done
echo "===== PART 2: run_parallel.sh (manage.py test --parallel 2 on a small project) ====="
sh "$HERE/run_parallel.sh" "$1" "$2" "$3" "$4" 2>&1
