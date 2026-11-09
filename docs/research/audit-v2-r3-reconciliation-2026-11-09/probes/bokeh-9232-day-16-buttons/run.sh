#!/usr/bin/env bash
# Usage: ./run.sh > output.txt   (needs node; no other dependency)
cd "$(dirname "$0")"
for tz in UTC America/Los_Angeles Asia/Tokyo; do
  echo "# TZ=$tz"
  TZ=$tz node probe.cjs
done
