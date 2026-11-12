#!/usr/bin/env bash
# Usage: ./run.sh > output.txt   (needs node; no other dependency)
for tz in UTC Europe/London Europe/Paris Asia/Kolkata Asia/Tokyo Pacific/Kiritimati America/Sao_Paulo America/New_York America/Los_Angeles Pacific/Honolulu; do
  TZ=$tz node "$(dirname "$0")/probe.js"
done
