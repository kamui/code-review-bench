# 02 internal/notify.go and internal/backup_test.go

## notify.go (finding 7)
`backupStatusTitle(succeeded, failed)` (notify.go:~31) replaces three identical three-way switches in `sendTelegramMessage`, `sendNtfy` and `sendSlackMessage`. This is a genuine deduplication and is correct. Remaining issues: the title constants sit inside the env-var const block (notify.go:26-28), and the strings begin with U+FE0F, which was an accidental leading character in the original and is now copied. A pure function with three outcomes is trivially unit-testable offline, but no test was added. Verification: diff reading, and `go test ./internal/` passes.

## backup_test.go (finding 6)
Evidence: backup_test.go:600-690. Removing `TestPublicGitLabRepositoryBackup2` is correct, as the body was byte-identical to its sibling. In `TestGiteaOrgsRepositoryBackup`, `resetBackups()` is still called inside both case arms and again after the switch (lines 629, 632, 635), so every arm resets twice. `assertGiteaAllOrgsBackedUp` repeats the org-two checks from `assertGiteaOrgTwoOnlyBackedUp`. Worked proposal: drop the in-arm resets, and have the all-orgs helper call a shared `assertOrgTwoBackedUp` plus org-one assertions. Note also that no new tests cover the production helpers extracted in backup.go. Verification: the live Gitea tests skip offline, so the restructure is unexecuted here.
