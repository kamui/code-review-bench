#!/usr/bin/env bash
# Usage: ./run.sh <site-dir holding python-dateutil 2.8.0 and six 1.12.0>
for tz in UTC Europe/Paris Asia/Tokyo Pacific/Kiritimati America/New_York America/Los_Angeles Pacific/Honolulu; do
  TZ=$tz python3 -I -W ignore "$(dirname "$0")/probe.py" "$1"
done
