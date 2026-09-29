# Detail 03 — docker/Dockerfile

Scope: `git diff main...review-head -- docker/Dockerfile`. Docker is unavailable, so the image was not built; verified by reading.

## Finding K — `rm -f "/var/cache/apk/*"` remains a no-op; merging layers made it look intentional

Evidence: Dockerfile line ~7. The glob is inside double quotes so the shell does not expand it and `rm -f` removes nothing (pre-existing). `--no-cache` already avoids leaving an index, so the `rm` is dead code. The PR merged the `adduser` and `apk add` instructions into one `RUN` to satisfy Sonar S7031 and edited this very line, so the moment was there to drop it (or fix the quoting), rather than carry a command that silently does nothing.

Other changes verified: apk packages are sorted alphabetically, and `${TAG}` is quoted inside the URL. Merging the user-creation step with package install couples two unrelated concerns into one layer, which reduces cache reuse marginally; acceptable given the Sonar rule, but the `adduser` step could remain separate with no layer-count benefit lost since the later `RUN` lines are already merged.
