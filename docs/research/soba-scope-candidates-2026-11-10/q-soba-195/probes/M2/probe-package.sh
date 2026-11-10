#!/bin/sh
# Usage: probe-package.sh <scratch-dir> <base|head> <sha>
# Runs the whole internal package with the GIT_BACKUP_DIR guard intact and disabled,
# listing which tests fail and how many skip.
set -u
S=$1; NAME=$2; SHA=$3
GO=/home/t3/.local/share/mise/installs/go/1.27.3/bin
mkdir -p "$S/home" "$S/tmp" "$S/config" "$S/work"
run_env() {
	env -i PATH="$GO:/usr/local/bin:/usr/bin:/bin" HOME="$S/home" TMPDIR="$S/tmp" XDG_CONFIG_HOME="$S/config" \
		GOPATH="$S/gopath" GOCACHE="$S/gocache" GOMODCACHE="$S/gomodcache" GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
		"$@"
}
echo "commit $NAME $SHA"
for variant in intact disabled; do
	W="$S/work/$NAME-pkg-$variant"
	rm -rf "$W"
	git -C "$S/soba" worktree add -q --detach "$W" "$SHA"
	if [ "$variant" = disabled ]; then
		sed -i 's/if !backupDIRKeyExists || backupDIR == "" {/if false \&\& (!backupDIRKeyExists || backupDIR == "") {/' "$W/internal/backup.go"
		echo "--- guard disabled: $(git -C "$W" diff --numstat)"
	fi
	echo "=== $NAME, guard $variant: go test ./internal/ -count=1 -v (summary lines only)"
	(cd "$W" && run_env go test ./internal/ -count=1 -v > "$S/tmp/pkg-$NAME-$variant.log" 2>&1; echo "exit status: $?")
	printf 'PASS: %s  FAIL: %s  SKIP: %s\n' \
		"$(grep -c '^--- PASS' "$S/tmp/pkg-$NAME-$variant.log")" \
		"$(grep -c '^--- FAIL' "$S/tmp/pkg-$NAME-$variant.log")" \
		"$(grep -c '^--- SKIP' "$S/tmp/pkg-$NAME-$variant.log")"
	grep -E '^--- (FAIL|PASS): TestFailureIfGitBackupDirUndefined|^--- FAIL' "$S/tmp/pkg-$NAME-$variant.log"
	tail -1 "$S/tmp/pkg-$NAME-$variant.log"
	git -C "$S/soba" worktree remove --force "$W"
done
