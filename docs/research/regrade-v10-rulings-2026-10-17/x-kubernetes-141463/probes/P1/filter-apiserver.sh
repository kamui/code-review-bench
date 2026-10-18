#!/bin/bash
# usage: filter-apiserver.sh <raw result> <saved result>
# Keeps the probe's own lines, the event recorder's "Server rejected event" lines (cut after the error text) and the
# test verdict. Drops the API server's, etcd's and the scheduler's own log lines.
{
  rg -a -n '^#' "$1" | sed -E 's/^[0-9]+://'
  echo "# kept: the probe's lines, the recorder's rejection log lines (cut after the error text) and the verdict; dropped: API server, etcd and scheduler log lines"
  rg -a '^(CONTENT TYPE|POD |  [a-zA-Z ].*|    note around.*|    POST.*|    \.\.\..*|=== RUN|--- (PASS|FAIL)|PASS|FAIL|ok  )' "$1" | rg -a -v '^ +[a-z_]+\.go:[0-9]+:' | sed -E 's/\t[0-9.]+s$//; s/ \([0-9.]+s\)$//'
  echo "# recorder log lines:"
  rg -a -o '"Server rejected event \(will not retry!\)" err="[^=]*characters"' "$1" | sed -E 's/\.[0-9a-f]{16}\\"/.<id>\\"/' | sort | uniq -c
} > "$2"
