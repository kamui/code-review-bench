#!/bin/sh
# Run probe_proxy.py with the virtual environment of each commit.
#   run.sh <suffix>      output: result-base<suffix>.txt and result-head<suffix>.txt beside this script
set -u
here="$(cd "$(dirname "$0")" && pwd)"
S="$here/scratch"; SUFFIX="${1:-}"
export PROBE_CERTS="$S/certs" PROBE_SCRATCH="$S"
unset REQUESTS_CA_BUNDLE CURL_CA_BUNDLE SSL_CERT_FILE SSL_CERT_DIR SSLKEYLOGFILE PYTHONPATH
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY all_proxy ALL_PROXY no_proxy NO_PROXY
run() { # label venv output
  PROBE_LABEL="$1" "$2/bin/python" -I -W ignore -c "import sys; sys.path.insert(0, '$here'); import runpy; runpy.run_path('$here/probe_proxy.py', run_name='__main__')" 2>&1 |
    sed -E "s/127\.0\.0\.1:[0-9]+/127.0.0.1:<port>/g; s/localhost:[0-9]+/localhost:<port>/g; s/port=[0-9]+/port=<port>/g; s#$S#<scratch>#g" > "$3"
  echo "== $1"; cat "$3"
}
run "base 8dd3b26b (before the change)" "$S/venv-base" "$here/result-base$SUFFIX.txt"
run "head 4089f3dc (the change)" "$S/venv-head" "$here/result-head$SUFFIX.txt"
