# 02 notify.go, backup_test.go, Dockerfile

Commands: `git diff main...review-head -- internal/notify.go internal/backup_test.go docker/Dockerfile`; `wc -l`; `go test ./internal/ -count=1` (ok).
Verification status: read-only; Docker build unavailable; live Gitea test skips without credentials, so the refactored assertions were not executed.

## A. TestGiteaOrgsRepositoryBackup (finding 5)

Evidence: internal/backup_test.go:600-680, with `resetBackups()` called inside each case and again after the switch. Proposal: table `[]struct{org string; assert func(*testing.T)}`, a `giteaOrgDir(org string) string` helper and a single reset per iteration. The extracted assertions are behaviourally identical to the old inline blocks (same Len and prefix checks), verified by diff reading.

## B. notify.go (finding 6)

Evidence: internal/notify.go:26-40 (constants and `backupStatusTitle`), call sites in `sendTelegramMessage`, `sendNtfy`, `sendSlackMessage`. Title strings are byte-copied from the prior literals including the leading U+FE0F. Behaviour unchanged.

## C. Dockerfile

Merged RUN layers, sorted packages and quoted URL are correct and behaviour-preserving. Not buildable here.
