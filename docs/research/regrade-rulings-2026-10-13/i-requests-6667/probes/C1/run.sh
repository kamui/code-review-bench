#!/bin/sh
# Run probe.py against the commit before the change and its head.
#   run.sh <scratch> <python> <suffix>
# <scratch>/base and <scratch>/head are git worktrees of the two commits; <scratch>/certs holds the
# throwaway certificates of make_certs.sh. <python> is the interpreter with the dependencies.
# Output: result-base<suffix>.txt and result-head<suffix>.txt beside this script.
set -u
S="$1"; PY="$2"; SUFFIX="${3:-}"
here="$(cd "$(dirname "$0")" && pwd)"
export PROBE_CERTS="$S/certs" PROBE_SCRATCH="$S"
unset REQUESTS_CA_BUNDLE CURL_CA_BUNDLE SSL_CERT_FILE SSL_CERT_DIR SSLKEYLOGFILE
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY all_proxy ALL_PROXY no_proxy NO_PROXY
run() { # label source-dir output
  PROBE_LABEL="$1" PYTHONPATH="$2:$here" "$PY" -W ignore "$here/probe.py" 2>&1 |
    sed -E "s/port=[0-9]+/port=<port>/g; s#$S#<scratch>#g" > "$3"
  echo "== $1"; cat "$3"
}
run "base 8dd3b26b (before the change)" "$S/base/src" "$here/result-base$SUFFIX.txt"
run "head 4089f3dc (the change)" "$S/head/src" "$here/result-head$SUFFIX.txt"
