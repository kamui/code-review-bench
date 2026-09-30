# 02 — `internal/notify.go`: backup status titles

Scope: `internal/notify.go` hunks (+18 / −28), plus the related log switch in `internal/backup.go:75-82`.

## Commands run

```
git diff main...review-head -- internal/notify.go
grep -n -e "succeeded > 0" -e "succeeded == 0" -e "failed > 0" internal/*.go
sed -n 26,28p internal/notify.go | od -c
go test ./internal/ -count=1 -coverprofile=...   # backupStatusTitle 0.0%, sendNtfy/sendSlackMessage/sendTelegramMessage 0.0%
```

## What the PR did well

The three copies of the succeeded/errors/failed `switch` in `sendTelegramMessage`, `sendNtfy` and `sendSlackMessage` are replaced by one `backupStatusTitle(succeeded, failed)`. I traced every call site and the extraction is behaviour-preserving.

---

## Finding 2.1 — The outcome classification is still duplicated; the dedup stopped at the file boundary

**Where.** `internal/notify.go:31-40` (`backupStatusTitle`) and `internal/backup.go:75-82` (`runProviderBackups`).

**Evidence.**

```go
// notify.go:31
switch {
case succeeded > 0 && failed == 0: return titleBackupsSucceeded
case failed > 0 && succeeded > 0:  return titleBackupsErrors
default:                           return titleBackupsFailed
}

// backup.go:75
switch {
case succeeded == 0 && failed >= 0: logger.Println("all backups failed")
case succeeded > 0 && failed > 0:   logger.Println("backups completed with errors")
default:                            logger.Println("backups complete")
}
```

These two switches encode the same three-state classification (succeeded / partial / failed), with the predicates written in a different order and style. I checked the edge cases (`0/0` → failed in both, `n/0` → succeeded in both, `n/m` → partial in both), and they currently agree. That agreement is by coincidence, not by construction. The PR's stated goal was removing duplicated literals, and it removed three copies while leaving the fourth copy of the same decision in place. The two copies are also a readability trap: `failed >= 0` is always true for a count.

**Code-judo proposal.** Model the outcome once and derive both strings from it:

```go
type backupOutcome int

const (
	outcomeSucceeded backupOutcome = iota
	outcomePartial
	outcomeFailed
)

func classifyBackups(succeeded, failed int) backupOutcome {
	switch {
	case succeeded == 0:
		return outcomeFailed
	case failed > 0:
		return outcomePartial
	default:
		return outcomeSucceeded
	}
}

var outcomeTitle = map[backupOutcome]string{ /* the three title constants */ }
var outcomeLog   = map[backupOutcome]string{ /* "backups complete", ... */ }
```

`runProviderBackups` logs `outcomeLog[classifyBackups(s, f)]`, and `backupStatusTitle` becomes `outcomeTitle[classifyBackups(s, f)]`. Even better, `notify` can compute the outcome once and pass it to each sender instead of re-passing `succeeded, failed` for the senders to re-derive. `classifyBackups` is pure and takes a four-row table test.

**Verification status.** Equivalence of the two switches was checked by hand over the sign cases above. `backupStatusTitle` has 0.0% offline coverage (see summary finding 6).

---

## Minor — title constants carry an invisible leading variation selector

`od -c` on `notify.go:27-28` shows that `titleBackupsErrors` and `titleBackupsFailed` start with the bytes `357 270 217` (U+FE0F, VARIATION SELECTOR-16) before the emoji. `titleBackupsSucceeded` does not. The byte existed in the old string literals, and the PR copied it into named constants. It is harmless to the recipients but invisible in review, and it means the three constants are not formatted consistently. Now that they are constants, removing it is a one-character change. It is folded into summary finding 8.
