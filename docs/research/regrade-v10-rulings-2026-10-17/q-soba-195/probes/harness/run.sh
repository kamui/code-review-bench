#!/usr/bin/env bash
# Usage: run.sh BIN WORKDIR PORT [SCENARIO...]
# Runs the soba binary BIN once per scenario, each with a clean environment (env -i),
# against the local fake API and git server (fake_api.py) on PORT. With no SCENARIO,
# runs all. Prints one block per scenario; sanitize.py makes the output stable.
set -u
BIN=$1 W=$2 PORT=$3
shift 3
HERE=$(cd "$(dirname "$0")" && pwd)
API="http://127.0.0.1:$PORT"
STUBDIR="$W/stub"
SYS_PATH=/usr/local/bin:/usr/bin:/bin

mkdir -p "$W/files" "$STUBDIR" "$W/home"
cp "$HERE/git-lfs" "$STUBDIR/git-lfs" && chmod +x "$STUBDIR/git-lfs"
printf 'true\n' > "$W/files/true"
printf 'refs\n' > "$W/files/refs"
# git 2.34 prints "warning: redirecting to ..." when a dumb-HTTP URL carries credentials, and
# githosts reads that warning as a ref, so a refs comparison could never match here. Strip the
# token from the fake server's URL inside git only; soba and githosts still see the URL with it.
git config --file "$W/home/.gitconfig" "url.http://127.0.0.1:$PORT/.insteadOf" "http://dummy-token@127.0.0.1:$PORT/"

# One scenario: NAME, backup dir key (or -), then VAR=VALUE settings.
sc() {
  local name=$1 dirkey=$2
  shift 2
  local d="$W/runs/$dirkey"
  mkdir -p "$d/backups"
  printf '%s\n' "$d/backups" > "$d/backup_dir_file"
  local settings=()
  local s
  for s in "$@"; do settings+=("${s//@DIR@/$d}"); done
  : > "$W/server.log"
  : > "$W/lfs.log"
  echo "=== scenario $name"
  echo "--- settings"
  printf '%s\n' "${settings[@]}"
  echo "--- soba output"
  if [ -n "${STRACE_FILE:-}" ]; then
    env -i PATH="$STUBDIR:$SYS_PATH" HOME="$W/home" GIT_CONFIG_NOSYSTEM=1 LFS_STUB_LOG="$W/lfs.log" \
      GITHUB_WORKER_DELAY=0 "${settings[@]}" \
      timeout 60 strace -f -qq -e trace=openat -o "$W/strace.out" "$BIN" 2>&1
  else
    env -i PATH="$STUBDIR:$SYS_PATH" HOME="$W/home" GIT_CONFIG_NOSYSTEM=1 LFS_STUB_LOG="$W/lfs.log" \
      GITHUB_WORKER_DELAY=0 "${settings[@]}" \
      timeout 60 "$BIN" 2>&1
  fi
  echo "--- exit code: $?"
  echo "--- requests to the fake server"
  cat "$W/server.log"
  echo "--- git-lfs calls"
  cat "$W/lfs.log"
  if [ -n "${STRACE_FILE:-}" ]; then
    echo "--- opens of $STRACE_FILE"
    grep -c "\"$STRACE_FILE\"" "$W/strace.out"
  fi
  echo "--- files in the backup directory"
  (cd "$d/backups" && find . -type f ! -path './.working/*' | sort)
  echo
}

GH=(GITHUB_TOKEN=dummy-token "GITHUB_APIURL=$API/graphql")
ALL=(P1-file-only P1-plain P2-file P2-plain PEq1-file PEq1-plain
     P3-gitea-url-only P3-sourcehut-url-only P3-gitea-token-file-missing P3-gitea-url-only-failure-only
     P3-control-nothing P3-control-github-token-file-missing P3-control-gitea-complete
     C1n-missing-lfs-file C1n-open-count C1n-skip-user-repos-file C1n-skip-user-repos-plain)
[ $# -eq 0 ] && set -- "${ALL[@]}"

for name in "$@"; do
  case $name in
    P1-file-only) sc "$name" p1a "GIT_BACKUP_DIR_FILE=@DIR@/backup_dir_file" "${GH[@]}" ;;
    P1-plain) sc "$name" p1b "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" ;;
    P1-note-trailing-newline) sc "$name" p1c "GIT_BACKUP_DIR=@DIR@/backups"$'\n' "${GH[@]}" ;;
    P2-file)
      sc "$name run 1" p2a "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" "GITHUB_COMPARE_FILE=$W/files/refs"
      sleep 1.2
      sc "$name run 2" p2a "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" "GITHUB_COMPARE_FILE=$W/files/refs" ;;
    P2-plain)
      sc "$name run 1" p2b "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" GITHUB_COMPARE=refs
      sleep 1.2
      sc "$name run 2" p2b "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" GITHUB_COMPARE=refs ;;
    PEq1-file) sc "$name" pe1a "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" "GITHUB_BACKUP_LFS_FILE=$W/files/true" ;;
    PEq1-plain) sc "$name" pe1b "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" GITHUB_BACKUP_LFS=true ;;
    P3-gitea-url-only) sc "$name" p3a "GIT_BACKUP_DIR=@DIR@/backups" "GITEA_APIURL=$API/api/v1" "SOBA_NTFY_URL=$API/ntfy" ;;
    P3-sourcehut-url-only) sc "$name" p3b "GIT_BACKUP_DIR=@DIR@/backups" "SOURCEHUT_APIURL=$API/query" "SOBA_NTFY_URL=$API/ntfy" ;;
    P3-gitea-token-file-missing) sc "$name" p3c "GIT_BACKUP_DIR=@DIR@/backups" "GITEA_APIURL=$API/api/v1" "GITEA_TOKEN_FILE=$W/files/missing-token" "SOBA_NTFY_URL=$API/ntfy" ;;
    P3-gitea-url-only-failure-only) sc "$name" p3d "GIT_BACKUP_DIR=@DIR@/backups" "GITEA_APIURL=$API/api/v1" "SOBA_NTFY_URL=$API/ntfy" SOBA_NOTIFY_ON_FAILURE_ONLY=true ;;
    P3-control-nothing) sc "$name" p3e "GIT_BACKUP_DIR=@DIR@/backups" "SOBA_NTFY_URL=$API/ntfy" ;;
    P3-control-github-token-file-missing) sc "$name" p3f "GIT_BACKUP_DIR=@DIR@/backups" "GITHUB_TOKEN_FILE=$W/files/missing-token" "SOBA_NTFY_URL=$API/ntfy" ;;
    P3-control-gitea-complete) sc "$name" p3g "GIT_BACKUP_DIR=@DIR@/backups" "GITEA_APIURL=$API/api/v1" GITEA_TOKEN=dummy-token "SOBA_NTFY_URL=$API/ntfy" ;;
    C1n-missing-lfs-file) sc "$name" c1a "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" "GITHUB_BACKUP_LFS_FILE=$W/files/missing-lfs" ;;
    C1n-open-count) STRACE_FILE="$W/files/true" sc "$name" c1b "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" "GITHUB_BACKUP_LFS_FILE=$W/files/true" ;;
    C1n-skip-user-repos-file) sc "$name" c1c "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" "GITHUB_SKIP_USER_REPOS_FILE=$W/files/true" ;;
    C1n-skip-user-repos-plain) sc "$name" c1d "GIT_BACKUP_DIR=@DIR@/backups" "${GH[@]}" GITHUB_SKIP_USER_REPOS=true ;;
    *) echo "unknown scenario $name" >&2; exit 2 ;;
  esac
done
