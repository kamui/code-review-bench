# 03 — Backup status titles (`internal/notify.go`) and the parallel classification in `runProviderBackups`

Commands used:

```
git diff main...review-head -- internal/notify.go
grep -n "succeeded > 0\|failed > 0" internal/*.go | grep -v _test
grep -n "soba backups" internal/notify.go | cat -A      # byte-level check of the title constants
```

## Overall

Extracting `backupStatusTitle` and deleting three copies of the same `switch` in `sendTelegramMessage`, `sendNtfy` and `sendSlackMessage` is the right move. It removes code rather than moving it, and it is the strongest part of the PR.

## Finding N1 — The same three-way outcome classification still exists a fourth time, with differently written predicates, in `runProviderBackups`

**Where:** `internal/notify.go:31-40` (`backupStatusTitle`), `internal/backup.go:76-83` (log switch in `runProviderBackups`).

**Evidence.** `backupStatusTitle` classifies `(succeeded, failed)` as

- `succeeded > 0 && failed == 0` → succeeded
- `failed > 0 && succeeded > 0` → completed with errors
- otherwise → failed

`runProviderBackups` classifies the same pair for its log line as

- `succeeded == 0 && failed >= 0` → "all backups failed"
- `succeeded > 0 && failed > 0` → "backups completed with errors"
- otherwise → "backups complete"

The two describe the same three states for non-negative counts, but the reader has to prove that by hand, because the cases are ordered differently and "failed" is the default in one while "complete" is the default in the other. The PR's goal was to stop duplicating this idea, and it deduplicated three of the four copies.

**Remedy (code-judo).** Model the outcome once and derive both the title and the log line from it:

```go
type backupOutcome int

const (
    outcomeSucceeded backupOutcome = iota
    outcomePartial
    outcomeFailed
)

func classifyBackups(succeeded, failed int) backupOutcome {
    switch {
    case succeeded > 0 && failed == 0:
        return outcomeSucceeded
    case succeeded > 0:
        return outcomePartial
    default:
        return outcomeFailed
    }
}

func (o backupOutcome) title() string    { return [...]string{titleBackupsSucceeded, titleBackupsErrors, titleBackupsFailed}[o] }
func (o backupOutcome) logLine() string  { return [...]string{"backups complete", "backups completed with errors", "all backups failed"}[o] }
```

`runProviderBackups` then does `outcome := classifyBackups(succeeded, failed); logger.Println(outcome.logLine())` and passes `outcome` to `notify`. The Telegram, ntfy and Slack senders stop taking `succeeded, failed` just to recompute it. Behaviour is unchanged.

**Verification status:** I confirmed by reading that the two switches agree for all `succeeded, failed >= 0`. Not implemented.

## Finding N2 — The new title constants embed an invisible leading U+FE0F

**Where:** `internal/notify.go:27-28`.

**Evidence.** `cat -A` shows that `titleBackupsErrors` and `titleBackupsFailed` both begin with the bytes `EF B8 8F` (U+FE0F VARIATION SELECTOR-16) *before* the emoji, while `titleBackupsSucceeded` doesn't. A variation selector with no preceding base character is a stray code point. It was copy-pasted from the original literals, so behaviour is preserved. But the PR has now given these strings names and made them the single source of truth. A future editor can't see the character, so string comparisons in tests or grep-based alerting will silently mismatch.

**Remedy.** Drop the leading U+FE0F from both constants (a trivial user-visible change: the notification title loses an invisible character), or add a comment explaining why it is there if it was intentional.

**Verification status:** Confirmed at the byte level.
