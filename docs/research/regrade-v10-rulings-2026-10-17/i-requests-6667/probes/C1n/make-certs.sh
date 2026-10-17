#!/bin/sh
# Usage: make-certs.sh DIR. Writes a private CA (ca.pem), a server certificate for 127.0.0.1
# signed by it (server.pem, server.key) and an OpenSSL hashed directory holding the CA (cadir/).
set -eu
d=$1
mkdir -p "$d/cadir"
cd "$d"
openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 30 -subj "/CN=probe CA" \
  -addext basicConstraints=critical,CA:TRUE -addext keyUsage=critical,keyCertSign,cRLSign 2>/dev/null
openssl req -newkey rsa:2048 -nodes -keyout server.key -out server.csr -subj "/CN=127.0.0.1" 2>/dev/null
printf 'subjectAltName=IP:127.0.0.1\nbasicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\n' > ext.cnf
openssl x509 -req -in server.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out server.pem -days 30 -extfile ext.cnf 2>/dev/null
cp ca.pem cadir/ca.pem
openssl rehash cadir
