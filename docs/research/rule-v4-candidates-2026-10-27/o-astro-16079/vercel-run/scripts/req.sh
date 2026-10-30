#!/usr/bin/env bash
# req.sh <logfile> <url> then reads request specs from args: "METHOD PATH [Cookie-value]"
set -u
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
U="$1"; shift
ts() { date -u +"%Y-%m-%dT%H:%M:%SZ"; }
run() { # $1 label  $2 path  $3 cookie(optional)
  echo "=============================================="
  echo "## $1  $(ts)"
  if [ -n "${3:-}" ]; then
    echo "\$ curl -sS -D - '$U$2' -H 'Cookie: $3'"
    curl -sS -D - "$U$2" -H "Cookie: $3"
  else
    echo "\$ curl -sS -D - '$U$2'"
    curl -sS -D - "$U$2"
  fi
  echo
}
