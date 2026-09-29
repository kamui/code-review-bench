# Detail 02 — internal/notify.go

Scope: `git diff main...review-head -- internal/notify.go`. Verified by reading; three `switch` blocks collapsed to `backupStatusTitle` (notify.go:31). The mapping is identical to the original, including the `succeeded == 0 && failed == 0` case (default -> "failed").

## Finding I — Status classification is duplicated in backup.go and now only partly centralised

Evidence: notify.go:31–40 classifies (succeeded, failed) into three states; backup.go:77–83 in `runProviderBackups` runs another `switch` over the same `(succeeded, failed)` pair to choose log text/exit path, with a subtly different predicate (`succeeded == 0 && failed >= 0`). So the same three-state model is expressed twice with different boolean shapes. The refactor fixed the notifier-side triplication and left the other copy.

Remedy: introduce a small typed status (`type runStatus int` with `statusSucceeded/statusErrors/statusFailed`) computed once by `classify(succeeded, failed)`; `backupStatusTitle` becomes a map/lookup from status to title and `runProviderBackups` switches on the same status. That makes the model the abstraction instead of a string-returning helper.

## Finding J — Title constants carry an invisible leading U+FE0F and the helper is undocumented

Evidence: notify.go:27–28, `"️⚠️ soba backups completed with errors"` and `"️🚨 soba backups failed"` begin with a stray variation-selector character (preserved verbatim from the original). It is invisible in review, and now hardened into named constants. Also `backupStatusTitle` has no doc comment while all new helpers in backup.go do, and the Telegram sender still builds its text by `+=` on the title while ntfy/Slack use it directly — three call sites, three usage shapes.

Remedy: strip the stray selector (or comment it), and add a one-line doc comment. Low severity; recorded as a legibility item.
