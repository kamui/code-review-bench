#!/bin/sh
# Usage: run-all.sh <scratch dir>
# Regenerates result-head.txt, result-base.txt and result-fixes.txt. <scratch dir> holds the worktrees
# base, head and fix-*, the environments venv-*, the socket directory sock and the file empty-ld-cache.
set -u
P=$(cd "$(dirname "$0")" && pwd)
S=$1
cd "$P"
{
echo "M1 at head fad334e1a9b54ea1acb8cce02a25934c5acfe99f. Run on 2026-10-12. See environment.txt."
echo
echo "================ Environment A: psycopg2-binary 2.9.9 only (no psycopg 3, no psycopg-pool) ================"
./run.sh $S/head $S/venv-pg2/bin/python $S/sock 54471
echo
echo "================ Environment B: psycopg2-binary 2.9.9 and the psycopg-pool 3.2.1 package, no psycopg 3 ================"
./run-short.sh $S/head $S/venv-pg2-poolpkg/bin/python $S/sock 54471
echo
echo "================ Environment C: psycopg2-binary 2.9.9 and a stand-in module named psycopg_pool (stub/psycopg_pool.py); not a real install ================"
./run-short.sh $S/head $S/venv-pg2/bin/python $S/sock 54471 $P/stub
echo
echo "================ Environment D: psycopg2-binary 2.9.9 together with psycopg[binary] 3.1.18 and psycopg-pool 3.2.1 ================"
./run-short.sh $S/head $S/venv-both/bin/python $S/sock 54471
echo
echo "================ Environment E: environment A after following the message: pip install \"psycopg[pool]\" (psycopg 3.1.18 pure Python, psycopg-pool 3.2.1); the system has libpq ================"
./run-short.sh $S/head $S/venv-advice/bin/python $S/sock 54471
echo
echo "================ Environment F: environment E with libpq hidden from the library search (bwrap, an empty /etc/ld.so.cache), as on a machine with no system libpq ================"
bwrap --dev-bind / / --ro-bind $S/empty-ld-cache /etc/ld.so.cache ./run-short.sh $S/head $S/venv-advice/bin/python $S/sock 54471
echo
echo "================ Environment G (control): psycopg[binary] 3.1.18 and psycopg-pool 3.2.1, no psycopg2 ================"
./run-short.sh $S/head $S/venv-pg3/bin/python $S/sock 54471
echo
echo "================ Environment H: psycopg 3.1.18 with psycopg-binary 3.1.18, no psycopg-pool, no psycopg2 ================"
./run-short.sh $S/head $S/venv-pg3-nopool/bin/python $S/sock 54471
} > result-head.txt 2>&1
{
echo "M1 at base bcccea3ef31c777b73cba41a6255cd866bf87237. Run on 2026-10-12. See environment.txt."
echo
echo "================ Environment A: psycopg2-binary 2.9.9 only ================"
./run.sh $S/base $S/venv-pg2/bin/python $S/sock 54471
echo
echo "================ Environment G: psycopg[binary] 3.1.18 and psycopg-pool 3.2.1, no psycopg2 ================"
./run-short.sh $S/base $S/venv-pg3/bin/python $S/sock 54471
} > result-base.txt 2>&1
./run-fixes.sh $S $S/sock 54471 > result-fixes.txt 2>&1
