#!/bin/sh
# One more throwaway certificate: self-signed, for localhost. Usage: make_selfsigned.sh <certs-dir>
set -eu
cd "$1"
openssl req -x509 -newkey rsa:2048 -nodes -days 30 -keyout selfsigned.key -out selfsigned.pem \
  -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" 2>/dev/null
openssl x509 -in selfsigned.pem -noout -subject -issuer
