#!/bin/sh
# Throwaway certificates for the probes. Usage: make_certs.sh <out-dir> <certifi-cacert.pem>
set -eu
out="$1"; certifi="$2"
mkdir -p "$out"; cd "$out"
mkca() { # name
  openssl req -x509 -newkey rsa:2048 -nodes -days 30 -keyout "$1.key" -out "$1.pem" \
    -subj "/CN=probe $1" -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign" 2>/dev/null
}
mkleaf() { # name ca san
  openssl req -newkey rsa:2048 -nodes -keyout "$1.key" -out "$1.csr" -subj "/CN=$1" 2>/dev/null
  printf 'subjectAltName=%s\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid\n' "$3" > "$1.ext"
  openssl x509 -req -in "$1.csr" -CA "$2.pem" -CAkey "$2.key" -CAcreateserial -days 30 \
    -extfile "$1.ext" -out "$1.pem" 2>/dev/null
}
mkca ca-default      # stands for a public CA that the default bundle trusts
mkca ca-private      # stands for a private CA that the default bundle does not trust
mkleaf srv-default  ca-default "DNS:localhost,IP:127.0.0.1"
mkleaf srv-private  ca-private "DNS:localhost,IP:127.0.0.1"
mkleaf srv-wronghost ca-default "DNS:other.example"
# The default bundle used by every probe: the real certifi bundle plus ca-default.
cat "$certifi" ca-default.pem > bundle-default.pem
# A hashed CA directory holding the same CA (for the directory probe).
mkdir -p cadir; cp ca-default.pem cadir/; openssl rehash cadir >/dev/null 2>&1 || c_rehash cadir >/dev/null
ls
