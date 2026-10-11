# Detail 02 — notify.go, backup_test.go, Dockerfile

Verification: diff read; `go vet` and `go test ./internal/ -count=1` pass offline. The Docker build was not possible (Docker is unavailable).

## Finding F8 — `backupStatusTitle` dedupes the strings, but the status model is still re-derived in every sender

Evidence: `notify.go:31-41` adds `backupStatusTitle(succeeded, failed int)`, and the three senders (`sendTelegramMessage` ~104, `sendNtfy` ~191, `sendSlackMessage` ~216) call it. All three still take the same `succeeded, failed int, errs []errors.E` triple, and `notify()` fans that triple out to each. The helper takes two bare ints whose argument order matters and is easy to swap in a future caller (`backupStatusTitle(failed, succeeded)` compiles). The helper has no doc comment, and the title constants were put inside the env-var `const` block (notify.go:26-28) next to `envTelegramChatID`, mixing configuration names with display strings.

Code-judo proposal: build the summary once in `notify()`, for example `type backupSummary struct{ succeeded, failed int; errs []errors.E }` with a `Title()` method. Pass `backupSummary` to each sender. The "succeeded/failed/errs" parameter cluster and the title derivation then live in one type, and the three senders lose two parameters each. Alternatively, move the title strings into their own `const` block directly above the helper.

Verification: read; a title change was the extent of the Sonar duplicate-literal fix, so this goes beyond it by design.

## Finding F9 — The extracted title constants freeze a stray U+FE0F prefix

Evidence: `xxd` of notify.go lines 27-28 shows `efb88f` (U+FE0F variation selector) at the start of both `titleBackupsErrors` and `titleBackupsFailed`, before the emoji (`"️⚠️ ..."`, `"️🚨 ..."`). The first title (`🚀`) has no such prefix. This was copied from the original literals, so it is not a regression. Naming the strings as constants makes the artefact look intentional and it is invisible in review. Telegram, ntfy, and Slack all send the extra invisible character.

Remedy: fix it in the constants (a one-line change, though a user-visible change) or add a comment saying the prefix is deliberate. Add a three-case table test for `backupStatusTitle`, since no test exists for it and the success, partial, and failed branches are otherwise only reached by live notifications.

Verification: `xxd` on `internal/notify.go`.

## Finding F10 — `TestGiteaOrgsRepositoryBackup` was split, but its odd control flow and the duplicated assertions remain

Evidence: backup_test.go ~620-640: inside the loop, each `switch` case calls `resetBackups()` and the loop body calls `resetBackups()` again after the switch, so every iteration resets twice. The `org` loop with a `switch` over exactly two known values is a table that has been unrolled. `assertGiteaOrgTwoOnlyBackedUp` and `assertGiteaAllOrgsBackedUp` (651-680) rebuild the same `path.Join(os.Getenv(envGitBackupDir), "gitea.lessknown.co.uk", ...)` four times each and repeat the `ReadDir` plus `Len` plus prefix checks.

Code-judo proposal: a table such as

```go
cases := []struct{ org string; want map[string][]string; absent []string }{...}
```

plus one `assertGiteaOrgDirs(t, want, absent)` helper. Drop the `switch` and the redundant `resetBackups()` calls. The two helper functions and the double reset disappear, and a third org case becomes a table row rather than a new function.

Verification: read; the test skips without Gitea credentials, so it was not run.

## Finding F11 — Dockerfile: layer merge is fine, but a dead `rm` remains and unrelated steps were fused

Evidence: docker/Dockerfile:4-8 now chains `addgroup`/`adduser` with `apk add` and `rm -f "/var/cache/apk/*"`. The quoted glob is not expanded by the shell, so the `rm` matches nothing (`--no-cache` already avoids the cache). Fusing user creation with package installation couples two unrelated concerns into one layer, which is what S7031 asked for. The separate `RUN` for the user was cheap, and the lint rule outweighs the readability here. Quoting `${TAG}` and sorting the apk list are fine. The hardcoded `soba_linux_amd64` asset still ignores `TARGETARCH`.

Remedy: delete the dead `rm -f` line, and decide whether the Sonar rule should be suppressed for the user-creation step rather than reshaping the file.

Verification: read only; the image build was not run (Docker unavailable).

## Positive notes

Removing `TestPublicGitLabRepositoryBackup2` is correct: its body is line-for-line identical to `TestPublicGitLabRepositoryBackup` (backup_test.go:570-583). No file crosses 1000 lines.
