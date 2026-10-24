#!/usr/bin/env bash
# Usage: drive.sh SCRATCH LABEL SRC_DIR OUT_FILE [SCENARIO...]
# Builds soba from SRC_DIR (a checkout of base, head, or head with a patch) offline from the
# unpacked module cache, then, inside a private user and network namespace (unshare -rnm) whose
# only network is loopback, points bitbucket.org and api.bitbucket.org at 127.0.0.1, starts
# fake_api.py and fake_bitbucket.py, runs run.sh and writes the sanitized output. No request
# can leave the machine.
set -eu
S=$1 LABEL=$2 SRC=$3 OUT=$4
shift 4
HERE=$(cd "$(dirname "$0")" && pwd)
PORT=18795
export GOMODCACHE="$S/gomodcache" GOCACHE="$S/gocache" GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local

mkdir -p "$S/bin"
(cd "$SRC" && go build -o "$S/bin/soba-$LABEL" .)

if [ ! -d "$S/fake/git/me/demo.git" ]; then
  mkdir -p "$S/fake/git/me" "$S/fake/seed"
  git -C "$S/fake/seed" init -q
  printf 'hello\n' > "$S/fake/seed/README"
  git -C "$S/fake/seed" add README
  git -C "$S/fake/seed" -c user.name=probe -c user.email=probe@example.invalid commit -q -m seed
  git clone -q --bare "$S/fake/seed" "$S/fake/git/me/demo.git"
  git -C "$S/fake/git/me/demo.git" update-server-info
fi

if [ ! -f "$S/tls/cert.pem" ]; then
  mkdir -p "$S/tls"
  openssl req -x509 -newkey rsa:2048 -nodes -days 2 -subj "/CN=bitbucket.org" \
    -addext "subjectAltName=DNS:bitbucket.org,DNS:api.bitbucket.org" \
    -keyout "$S/tls/key.pem" -out "$S/tls/cert.pem" 2>/dev/null
fi

W="$S/work/$LABEL.$(date +%s%N)"
mkdir -p "$W"
printf '127.0.0.1 localhost\n127.0.0.1 bitbucket.org api.bitbucket.org\n' > "$W/hosts"

unshare -rnm bash -s "$HERE" "$S" "$W" "$PORT" "$S/bin/soba-$LABEL" "$@" <<'NS' | python3 -I "$HERE/sanitize.py" "$W" "$PORT" > "$OUT"
set -eu
HERE=$1 S=$2 W=$3 PORT=$4 BIN=$5
shift 5
ip link set lo up
mount --bind "$W/hosts" /etc/hosts
python3 -I "$HERE/fake_api.py" "$PORT" "$S/fake" "$W/server.log" &
A=$!
python3 -I "$HERE/fake_bitbucket.py" "$S/tls/cert.pem" "$S/tls/key.pem" "$S/fake" "$W/server.log" &
B=$!
trap 'kill $A $B 2>/dev/null || true' EXIT
sleep 0.7
bash "$HERE/run.sh" "$BIN" "$W" "$PORT" "$S/tls/cert.pem" "$@"
NS
