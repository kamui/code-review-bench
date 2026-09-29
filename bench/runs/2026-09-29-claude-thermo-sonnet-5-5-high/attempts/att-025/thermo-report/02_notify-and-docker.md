# Detail: internal/notify.go and docker/Dockerfile

## notify.go (+18/-28)

Change: three copies of the succeeded/errors/failed switch (Telegram, ntfy, Slack) are replaced by `backupStatusTitle(succeeded, failed)` and three title constants (notify.go:26-40).

Assessment: this is a good dedup that removes real repetition, and the strings are byte-identical, including the leading U+FE0F variation selector on the warning and alarm titles. Findings are minor.

1. `backupStatusTitle` has no doc comment, unlike the helpers added in backup.go. Only one of three call sites (`sendTelegramMessage`) has a `text +=` after it, so the function name suggests "title" while Telegram uses it as the start of a message body. That is acceptable.
2. The classification `succeeded > 0 && failed == 0` / `failed > 0 && succeeded > 0` / default is now encoded once, but `notify()` earlier in the file makes its own `succeeded`/`failed` decisions for `SOBA_NOTIFY_ON_FAILURE_ONLY`. Consider whether a single `backupStatus` enum (`statusSucceeded`, `statusPartial`, `statusFailed`) with a `Title()` method would serve both. It is a modest reframing with no urgency.
3. The constants contain invisible characters (variation selector before the emoji). Constants mean this happens once, but a comment or `️` escape would stop an editor from silently normalising them. This is a nit.

Verification: read the diff; no tests exercise the notification titles. Existing tests were not run against live webhooks (no network).

## docker/Dockerfile (+3/-4)

Change: the `addgroup`/`adduser` and `apk add` RUN instructions are merged into one layer, apk packages are sorted, and the release URL is quoted.

Assessment: correct and low risk. The Dockerfile could not be built here (Docker unavailable), so this is by reading only.

1. Merging the user creation with `apk add` couples two unrelated concerns in one layer. The layer count drops, but a change to the package list now invalidates the cached user-creation step. It is negligible in cost. The stated aim is a Sonar rule (docker:S7031), so this is defensible.
2. `rm -f "/var/cache/apk/*"` is pre-existing and, being quoted, does not glob, so it removes nothing. `--no-cache` already avoids the cache, so the `rm` is dead code. The PR touched this line's neighbours and could have deleted it.
3. Quoting `"...${TAG}..."` is correct hardening. `curl -L` follows redirects with `--proto "=https"` applied only to the first request; adding `--proto-redir` would be tighter. That is out of scope.
