#!/bin/sh
# Usage: probe.sh <scratch-dir> <probe-dir> <base|head> <sha>
# 1. Runs zz_m1_probe_test.go: which providers the start-up log names, for three settings.
# 2. (head only) Adds a sixth provider function in the head's pattern without a call, then a call without
#    the function, and reports what go build, go vet, staticcheck (U1000) and the project's golangci-lint say.
set -u
S=$1; P=$2; NAME=$3; SHA=$4
GO=/home/t3/.local/share/mise/installs/go/1.27.3/bin
run_env() {
	env -i PATH="$GO:$S/gopath/bin:/usr/local/bin:/usr/bin:/bin" HOME="$S/home" TMPDIR="$S/tmp" XDG_CONFIG_HOME="$S/config" \
		GOPATH="$S/gopath" GOCACHE="$S/gocache" GOMODCACHE="$S/gomodcache" GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
		GOLANGCI_LINT_CACHE="$S/golangci-cache" "$@"
}
mkdir -p "$S/home" "$S/tmp" "$S/config" "$S/work"
echo "commit $NAME $SHA"
W="$S/work/$NAME-m1"
rm -rf "$W"
git -C "$S/soba" worktree add -q --detach "$W" "$SHA"
cp "$P/zz_m1_probe_test.go" "$W/internal/"
echo "=== $NAME: start-up log lines (go test ./internal/ -run 'TestM1ProbeStartupLog\$' -count=1 -v)"
(cd "$W" && run_env go test ./internal/ -run 'TestM1ProbeStartupLog$' -count=1 -v 2>&1 | grep -v '^=== RUN')
rm "$W/internal/zz_m1_probe_test.go"
if [ "$NAME" = head ]; then
	echo "=== head, unmodified: staticcheck -checks U1000 ./internal/"
	(cd "$W" && run_env staticcheck -checks U1000 ./internal/ 2>&1; echo "exit status: $?")
	if command -v "$S/gopath/bin/golangci-lint" >/dev/null 2>&1; then
		echo "=== head, unmodified: golangci-lint run --config .golangci.yml ./internal/"
		(cd "$W" && run_env golangci-lint run --config .golangci.yml ./internal/ > "$S/tmp/golangci.out" 2>&1; echo "exit status: $?"; grep -v "^level=warning" "$S/tmp/golangci.out")
	fi
	echo "=== head + function without call: internal/zz_forgotten.go"
	cp "$P/forgotten_call.go.txt" "$W/internal/zz_forgotten.go"
	echo "--- go build ./..."; (cd "$W" && run_env go build ./... 2>&1; echo "exit status: $?")
	echo "--- go vet ./internal/"; (cd "$W" && run_env go vet ./internal/ 2>&1; echo "exit status: $?")
	echo "--- staticcheck -checks U1000 ./internal/"; (cd "$W" && run_env staticcheck -checks U1000 ./internal/ 2>&1; echo "exit status: $?")
	if command -v "$S/gopath/bin/golangci-lint" >/dev/null 2>&1; then
		echo "--- golangci-lint run --config .golangci.yml ./internal/"
		(cd "$W" && run_env golangci-lint run --config .golangci.yml ./internal/ > "$S/tmp/golangci.out" 2>&1; echo "exit status: $?"; grep -v "^level=warning" "$S/tmp/golangci.out")
	fi
	rm "$W/internal/zz_forgotten.go"
	echo "=== head + call without function: displaySourcehutStartupConfig() added after line 199"
	sed -i '199a\	displaySourcehutStartupConfig()' "$W/internal/backup.go"
	git -C "$W" diff --no-color -U2
	echo "--- go build ./..."; (cd "$W" && run_env go build ./... 2>&1; echo "exit status: $?")
fi
git -C "$S/soba" worktree remove --force "$W"
