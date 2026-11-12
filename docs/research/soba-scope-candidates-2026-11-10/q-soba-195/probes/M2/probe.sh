#!/bin/sh
# Usage: probe.sh <scratch-dir> <probe-dir> <base|head> <sha>
# Copies the checkout at <sha> twice (guard intact, guard disabled), runs the named test,
# then adds zz_m2_probe_test.go to print Run()'s error and to try the proposed assertion.
set -u
S=$1; P=$2; NAME=$3; SHA=$4
GO=/home/t3/.local/share/mise/installs/go/1.27.3/bin
mkdir -p "$S/home" "$S/tmp" "$S/config" "$S/work"
run_env() {
	env -i PATH="$GO:/usr/local/bin:/usr/bin:/bin" HOME="$S/home" TMPDIR="$S/tmp" XDG_CONFIG_HOME="$S/config" \
		GOPATH="$S/gopath" GOCACHE="$S/gocache" GOMODCACHE="$S/gomodcache" GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
		"$@"
}
echo "commit $NAME $SHA"
for variant in intact disabled; do
	W="$S/work/$NAME-$variant"
	rm -rf "$W"
	git -C "$S/soba" worktree add -q --detach "$W" "$SHA"
	if [ "$variant" = disabled ]; then
		before=$(grep -c 'if !backupDIRKeyExists || backupDIR == "" {' "$W/internal/backup.go")
		sed -i 's/if !backupDIRKeyExists || backupDIR == "" {/if false \&\& (!backupDIRKeyExists || backupDIR == "") {/' "$W/internal/backup.go"
		after=$(grep -c 'if false && (!backupDIRKeyExists || backupDIR == "") {' "$W/internal/backup.go")
		echo "--- guard disabled: $before line(s) matched, $after line(s) rewritten"
		git -C "$W" diff --no-color -U1
	fi
	echo "=== $NAME, guard $variant: go test ./internal/ -run 'TestFailureIfGitBackupDirUndefined\$' -count=1 -v"
	(cd "$W" && run_env go test ./internal/ -run 'TestFailureIfGitBackupDirUndefined$' -count=1 -v 2>&1)
	echo "exit status: $?"
	cp "$P/zz_m2_probe_test.go" "$W/internal/zz_m2_probe_test.go"
	echo "=== $NAME, guard $variant: probe, Run() error under the same setup"
	(cd "$W" && run_env go test ./internal/ -run 'TestM2ProbeRunError$' -count=1 -v 2>&1)
	echo "exit status: $?"
	echo "=== $NAME, guard $variant: probe, the proposed require.ErrorContains assertion"
	(cd "$W" && run_env go test ./internal/ -run 'TestM2ProbeProposedAssertion$' -count=1 -v 2>&1)
	echo "exit status: $?"
	git -C "$S/soba" worktree remove --force "$W"
done
