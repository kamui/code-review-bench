#!/usr/bin/env bash
# Usage: run-probe-d.sh <scratch> <GROUP> [jsdom|chromium]
# The runner of ../run-probe.sh, copied unchanged in what it does, for the groups added later (D1, D2,
# D3): runs probes/<GROUP>/probe.test.tsx inside <scratch>/source, in the project's own test setup, at
# the commit before the change (base), the first revision pushed to the pull request (6a40cf1, written
# "earlier-revision") and the reviewed head, and saves what each run printed beside the probe.
set -u
exec "$(cd "$(dirname "$0")" && pwd)/run-probe.sh" "$1" "$2" "${3:-jsdom}" probe.test.tsx result
