#!/bin/sh
# Fetches the raw GitHub API responses this dossier relies on. Read-only.
set -u
R=repos/trpc/trpc
get() { out=$1; shift; gh api "$@" > "$out" || echo "FAILED $out"; }
get pr-5017.json $R/pulls/5017
get pr-5017-commits.json $R/pulls/5017/commits --paginate
get pr-5017-review-comments.json $R/pulls/5017/comments --paginate
get pr-5017-issue-comments.json $R/issues/5017/comments --paginate
get pr-5017-reviews.json $R/pulls/5017/reviews --paginate
get pr-5017-timeline.json $R/issues/5017/timeline --paginate
get issue-5020.json $R/issues/5020
get issue-5037.json $R/issues/5037
get issue-5037-comments.json $R/issues/5037/comments --paginate
get pr-5039.json $R/pulls/5039
get pr-5039-files.json $R/pulls/5039/files --paginate
get pr-5039-commits.json $R/pulls/5039/commits --paginate
get pr-5057.json $R/pulls/5057
get pr-5057-files.json $R/pulls/5057/files --paginate
get issue-5056.json $R/issues/5056
get release-v10.43.3.json $R/releases/tags/v10.43.3
get release-v10.43.4.json $R/releases/tags/v10.43.4
get release-v10.43.6.json $R/releases/tags/v10.43.6
get commits-utils-ts-v10.45.2.json "$R/commits?path=packages/server/src/core/internals/utils.ts&sha=v10.45.2&per_page=100"
# The @sentry/node tRPC middleware as its file stood on 2023-09-12, before the last push.
get sentry-handlers-commits-before-push.json "repos/getsentry/sentry-javascript/commits?path=packages/node/src/handlers.ts&until=2023-11-09T22:08:09Z&per_page=3"
get sentry-handlers-2023-09-12.json "repos/getsentry/sentry-javascript/contents/packages/node/src/handlers.ts?ref=a7f5911ce2092280dc035cefa48d0bbd2ae10d0a"
