# Release metadata

## Scope and outcome

The added `.changeset/common-cats-travel.md` is five lines. It declares `'@astrojs/vercel': patch` and describes the ISR path rewrite fix. The adapter package is named `@astrojs/vercel`, so the release target matches the changed executable subsystem.

There are no actionable findings in this subsystem. The change is a direct release declaration and introduces no conditional flow, abstraction, type boundary, orchestration, or file-size concern. A standalone metadata module or a longer release explanation would not remove implementation complexity.

## Evidence and verification

`git diff main...review-head` supplied the entire added file. Reading `packages/integrations/vercel/package.json` confirmed the package identity. `git diff --check main...review-head` exited successfully. No external release tooling or registry was invoked.

The changeset accurately describes the positive behavior observed by the ISR probe: the built ISR handler can restore `/one` from the query and render it. Whether the executable remedy should be changed before release is addressed in [01_serverless-routing.md](01_serverless-routing.md), rather than treated as a second metadata finding.

No changeset parsing test was necessary for this conventional five-line declaration. The executable verification and its platform limits are recorded in the routing detail report. The checkout was not edited.
