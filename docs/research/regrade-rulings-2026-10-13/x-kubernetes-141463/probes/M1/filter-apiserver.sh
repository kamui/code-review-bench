#!/bin/bash
# usage: filter-apiserver.sh <raw result> <saved result>
# Keeps the probe's own lines, the event recorder's "Server rejected event" lines (cut after the error text) and the
# test verdict. Drops the API server's and etcd's start-up and shut-down log lines.
{
  rg -n '^#' "$1" | sed -E 's/^[0-9]+://'
  echo "# kept: the probe's lines, the recorder's rejection log lines (cut after the error text) and the verdict; dropped: API server and etcd log lines"
  rg '^(CONTENT TYPE|POD |  [a-zA-Z ].*|=== RUN|--- (PASS|FAIL)|PASS|FAIL|ok  )' "$1" | rg -v '^    [a-z_]+\.go:[0-9]+:' | sed -E 's/\t[0-9.]+s$//; s/ \([0-9.]+s\)$//'
  echo "# recorder log lines:"
  rg -o '"Server rejected event \(will not retry!\)" err="[^=]*characters"' "$1" | sed -E 's/\.[0-9a-f]{16}\\"/.<id>\\"/' | sort | uniq -c
} > "$2"
