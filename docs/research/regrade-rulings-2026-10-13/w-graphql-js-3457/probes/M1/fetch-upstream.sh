#!/usr/bin/env bash
# The upstream requests behind the files in ../../upstream/, as run on 2026-10-12 with gh 2.100.0.
# Run from the upstream/ directory. The cut-off is the last push, 2022-01-17T11:22:02Z.
set -euo pipefail
repo=repos/graphql/graphql-js
cut='created:<2022-01-17T11:22:02Z'

# The pull request and its description's edit history.
gh api $repo/pulls/3457 > pr-3457.json
gh api $repo/issues/3457/comments --paginate > pr-3457-issue-comments.json
gh api $repo/pulls/3457/reviews --paginate > pr-3457-reviews.json
gh api $repo/pulls/3457/comments --paginate > pr-3457-review-comments.json
gh api $repo/pulls/3457/commits --paginate > pr-3457-commits.json
gh api $repo/issues/3457/timeline --paginate > pr-3457-timeline.json
gh api graphql -f query='query { repository(owner:"graphql", name:"graphql-js") { pullRequest(number:3457) { title body createdAt lastEditedAt editor { login } includesCreatedEdit author { login } authorAssociation mergedAt mergedBy { login } userContentEdits(first:50) { totalCount nodes { createdAt editedAt editor { login } diff deletedAt } } } } }' > pr-3457-edits.graphql.json

# Maintainers before the cut-off: seven searches, then nine threads in full.
i=0
for q in \
  "repo:graphql/graphql-js OverlappingFieldsCanBeMerged $cut" \
  "repo:graphql/graphql-js sameArguments $cut" \
  "repo:graphql/graphql-js validation performance overlapping $cut" \
  "repo:graphql/graphql-js validation \"denial of service\" $cut" \
  "repo:graphql/graphql-js validate DoS $cut" \
  "repo:graphql/graphql-js validation slow query $cut" \
  "repo:graphql/graphql-js \"overlapping fields\" $cut"; do
  i=$((i + 1))
  gh api -X GET search/issues -f q="$q" -f per_page=100 > maintainers-q$i.json
  sleep 2
done
for n in 2185 386 2352 3442 3455 2331 723 2074 406; do
  gh api $repo/issues/$n > maintainers-issue-$n.json
  gh api $repo/issues/$n/comments --paginate > maintainers-issue-$n-comments.json
done
for n in 386 2352 3442 3455 2074; do
  gh api $repo/pulls/$n/comments --paginate > maintainers-pr-$n-review-comments.json
  gh api $repo/pulls/$n/reviews --paginate > maintainers-pr-$n-reviews.json
done

# Public code before the cut-off: two server packages at their last commit before it, and one code search.
gh api "repos/graphql/express-graphql/commits?path=src/index.ts&until=2022-01-17T11:22:02Z&per_page=1" > public-code-express-graphql-commit.json
gh api "repos/graphql/express-graphql/contents/src/index.ts?ref=92088332c3d81174a82a77abfdea8e4b5fc77100" -H "Accept: application/vnd.github.raw" > public-code-express-graphql-index.ts
gh api "repos/graphql/express-graphql/contents/src/parseBody.ts?ref=92088332c3d81174a82a77abfdea8e4b5fc77100" -H "Accept: application/vnd.github.raw" > public-code-express-graphql-parseBody.ts
gh api "repos/apollographql/apollo-server/commits?path=packages/apollo-server-core/src/requestPipeline.ts&until=2022-01-17T11:22:02Z&per_page=1" > public-code-apollo-server-commit.json
gh api "repos/apollographql/apollo-server/contents/packages/apollo-server-core/src/requestPipeline.ts?ref=294a8ed79becbbfe62a66b0b2545577cadac55c2" -H "Accept: application/vnd.github.raw" > public-code-apollo-server-requestPipeline.ts
gh api -X GET search/code -f q='"specifiedRules" "validate(schema, documentAST" language:TypeScript' -f per_page=50 > public-code-q1.json

# Later than the cut-off.
gh api "$repo/commits?path=src/validation/rules/OverlappingFieldsCanBeMergedRule.ts&per_page=100" --paginate > later-commits-rule-file-main.json
gh api "$repo/commits?path=src/validation/rules/OverlappingFieldsCanBeMergedRule.ts&per_page=100&sha=16.x.x" --paginate > later-commits-rule-file-16.x.x.json
gh api "$repo/commits?path=src/validation/rules/OverlappingFieldsCanBeMergedRule.ts&per_page=100&sha=15.x.x" > later-commits-rule-file-15.x.x.json
gh api "$repo/commits?path=src/validation/rules/OverlappingFieldsCanBeMergedRule.js&per_page=100&sha=15.x.x" > later-commits-rule-file-js-15.x.x.json
for n in 3958 3967 4857; do
  gh api $repo/pulls/$n > later-pr-$n.json
  gh api $repo/pulls/$n/files --paginate > later-pr-$n-files.json
  gh api $repo/issues/$n/comments --paginate > later-pr-$n-issue-comments.json
  gh api $repo/pulls/$n/reviews --paginate > later-pr-$n-reviews.json
  gh api $repo/pulls/$n/comments --paginate > later-pr-$n-review-comments.json
done
gh api $repo/pulls/3958/commits --paginate > later-pr-3958-commits.json
for n in 3972 4520 4840; do
  gh api $repo/pulls/$n > later-pr-$n.json
  gh api $repo/issues/$n/comments --paginate > later-pr-$n-issue-comments.json
done
gh api $repo/issues/3955 > later-issue-3955.json
gh api $repo/issues/3955/comments --paginate > later-issue-3955-comments.json
gh api $repo/issues/3955/timeline --paginate > later-issue-3955-timeline.json
gh api $repo/commits/8f4c64eb6a7112a929ffeef00caa67529b3f2fcf -H "Accept: application/vnd.github.diff" > later-commit-8f4c64eb-pr-3967.diff
gh api $repo/commits/f94b511386c7e47bd0380dcd56553dc063320226 -H "Accept: application/vnd.github.diff" > later-commit-f94b5113-pr-3958.diff
gh api $repo/commits/ee5ce41d4b68d1852306d3b56dba2cbbb6c43fea -H "Accept: application/vnd.github.diff" > later-commit-ee5ce41d-pr-4857.diff
gh api $repo > repo.json
for ref in 17.x.x 16.x.x; do
  gh api "$repo/contents/src/validation/rules/OverlappingFieldsCanBeMergedRule.ts?ref=$ref" -H "Accept: application/vnd.github.raw" > later-rule-file-$ref.ts
done
gh api "$repo/releases?per_page=100" --paginate > releases.json
gh api -X GET advisories -f cve_id=CVE-2023-26144 > later-advisory-CVE-2023-26144.json
i=0
for q in \
  'repo:graphql/graphql-js OverlappingFieldsCanBeMerged created:>2022-01-17T11:22:02Z' \
  'repo:graphql/graphql-js sameArguments OR stringifyArguments created:>2022-01-17T11:22:02Z' \
  'repo:graphql/graphql-js validation performance repeated fields created:>2022-01-17T11:22:02Z'; do
  i=$((i + 1))
  gh api -X GET search/issues -f q="$q" -f per_page=100 > later-search-q$i.json
  sleep 2
done
