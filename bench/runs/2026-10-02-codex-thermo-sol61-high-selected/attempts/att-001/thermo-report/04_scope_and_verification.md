# Scope, repository-wide migration, and verification record

## Review execution

The only instruction resource read was the explicitly selected frozen skill at `/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-001/clone-work/frozen-skill/SKILL.md`. It contains no required child call or referenced-resource step. No reviewer workers were created. Repository or ancestor guidance, personal skills, memories, client configuration, and prior reviews were not loaded.

The pinned range was inspected with `git diff main...review-head`, including every non-import hunk. Import changes were traced into boundary assertions rather than assumed to be mechanically safe. Cached protobuf source was read to verify adapter, duration, and JSON-option behavior. No upstream web or forge access occurred.

All scratch files, overlays, logs, and proposals are outside the clone under `/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-001/clone-work/review-probes`. The native layered deliverable is this directory, with a summary and four subsystem/verification details. The detail files are preserved as written; `finding-index.json` is produced afterward from the summary's exact finding text.

## Size and complexity measurements

`git diff --stat main...review-head` reports 68 changed files, 165 insertions, and 174 deletions. File-line counts were measured by comparing `git show main:<path>` bytes with the head file for every changed path, and are retained in `../review-probes/line-counts.json`.

There are no crossings from below 1,000 lines to 1,000 or above. The largest increases are `status/status_test.go`, from 468 to 470, and `xds/internal/httpfilter/fault/fault_test.go`, from 671 to 673. The remaining largest increases are one line each. `internal/pretty/pretty.go` decreases from 82 to 81. Its finding concerns removal of an obsolete branch, not size growth.

Twelve touched files already exceed 1,000 lines in the base and remain the same size. They include `test/end2end_test.go` at 6,389, `internal/transport/http2_server.go` at 1,446, and the LDS/RDS resource tests at 1,859 and 1,613. The PR does not add behavior to those large files beyond migration changes. No demand to decompose those preexisting modules is attached to this review.

No new state-machine modes, feature flags, concurrency orchestration, partial updates, or scattered policy branches were introduced. The important growth in conceptual complexity is keeping duplicate generation-sensitive serialization after a unified runtime becomes available, and letting runtime wrapper types leak through an application-facing result.

## Other source changes reviewed

The xDS cluster-specifier and HTTP-filter interfaces now consistently use the V2 message interface with their migrated implementations and test plugins. Their configs are repository-controlled V2 generated messages or native `Any` values. The switch from `ptypes.UnmarshalAny` to `Any.UnmarshalTo`, and from `ptypes.Is` to `Any.MessageIs`, is direct and keeps parsing in the appropriate resource/filter layer. No new wrapper or generic dispatcher is required.

Both old and new `TypedStruct` cases in `unwrapHTTPFilterConfig` describe distinct supported wire types, so they cannot be deleted merely because the protobuf runtime is unified. That duplication has a semantic reason unlike the JSON serializer split. Existing map override handling, optional filters, and config validation branches are not extended by the migration.

`internal/testutils.MarshalAny` and the xDS resource builder's `marshalAny` now receive V2 messages and call `anypb.New` directly. Removing the old `MessageV2Of` call at an already V2-controlled boundary is appropriate. Test utilities using concrete `Any`, duration, wrapper, and struct messages migrate to the new canonical packages; the old well-known-type packages in the pinned legacy runtime alias the same native types.

Transport status marshaling, grpclb requests, ORCA fixtures, interop fixtures, and test equality operations use concrete generated messages. The native marshal/clone/equality calls remain direct. The status tests are specifically discussed in detail 01 because their V2 casts do not prove the preserved V1-facing API contract.

The binary-log timestamp replacement uses `timestamppb.Now`, and Go-duration construction uses `durationpb.New`. These simplify direct construction without adding branches. No timestamp-related runtime regression was demonstrated within ordinary current timestamps. The duration conversion in the opposite direction is the checked-boundary finding in detail 03.

`examples/go.mod` moves the legacy module from direct to indirect use. `test/tools/tools.go` switches the pinned generator import to `google.golang.org/protobuf/cmd/protoc-gen-go`, matching the generator invoked by the unchanged `regenerate.sh`. `test/tools/go.mod` additionally downgrades `golang.org/x/tools` from v0.17.0 to v0.14.0 and changes indirect dependencies. That downgrade is outside the core migration purpose, but no concrete failure from it was established; it is not promoted into an actionable finding. The nested tools and examples modules were not executed.

## Go command environment

Commands ran from `/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-001/clone`. Each was bounded with `timeout 300s`, used `go test -timeout=240s`, and set:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-001/clone-cache/gomodcache
GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-001/clone-cache/gocache
GOFLAGS=-mod=readonly
GOPROXY=off
GOSUMDB=off
GOTOOLCHAIN=local
```

The actual calls passed these settings through `env` and piped output through `tee` with shell `pipefail`. Dependencies were not fetched. Focused fixtures used by existing tests could communicate with their local listeners under the execution allowance.

The existing-test command was:

```sh
go test -timeout=240s ./encoding/proto ./status ./internal/binarylog ./balancer/rls ./internal/pretty ./internal/testutils ./xds/internal/xdsclient/transport ./xds/internal/clusterspecifier/rls ./xds/internal/httpfilter/router ./xds/internal/httpfilter/rbac ./xds/internal/xdsclient/xdsresource ./xds/internal/xdsclient/xdslbregistry
```

Nine packages completed tests successfully. `internal/pretty`, `xds/internal/httpfilter/router`, and `xds/internal/httpfilter/rbac` compiled and report no test files. Exact timings and outcomes are in `../review-probes/existing-tests.log`. This is a focused set, not the full repository suite.

## Probe commands and comparison design

Six scratch test files were exposed through Go overlays at virtual `review_probe_test.go` paths inside the relevant packages. They do not exist in the physical clone. Two overlays identify the same tests. The base overlay additionally substitutes five production files with exact `git show main:<path>` content: codec, internal status, binary-log method logger, RLS config, and LRS load-report processing.

The base control is therefore the affected base implementations running with otherwise-head dependencies and fixtures. No finding relies on a proposed remedy passing, and no remedy was applied. Both revisions already pin the relevant protobuf runtime versions.

The combined head command used `-run TestReview -v -overlay=<head-overlay>` over six packages. The combined base command used the same flags with `<base-overlay>` over five packages. Completed codec, status, binary-log, and RLS results are decisive despite the combined commands' overall nonzero status. The formatter equivalence case also completed successfully on the head.

Transport fixture compilation initially failed because the scratch code referenced an unavailable component constructor, then because it passed a non-depth logger interface. Those mistakes were corrected in scratch. The corrected error-diagnostic command used `-count=1 -run TestReviewLoadIntervalError` on the head and `-run TestReviewLoadIntervalError` on the base. The overflow command used `-run TestReviewLoadIntervalOverflow` on each overlay. Details 01 and 03 record the command arguments and outcomes. Each executed package/flag set was run once, all commands stayed under five minutes, and no full suite was invoked.

## Verification status by actionable finding

| Summary finding | Verification status |
| --- | --- |
| F1, duplicate JSON serializer | Pinned option semantics inspected; equivalence probe passes |
| F2, V1 codec rejection | Both head directions fail; both base directions pass |
| F3, wrapped legacy status detail | Head concrete type fails; base and old decoder control pass |
| F4, legacy binary-log payload loss | Both head directions lose bytes; both base directions pass |
| F5, duration overflow acceptance | Four RLS cases and two LRS cases fail on head, pass on base |
| F6, lost validation cause | Three head cases lose cause; all base cases retain it |

No full suite, race testing, benchmarks, nested-module tests, or generator execution was performed. The worked restructuring proposals are source-level designs; their production implementations still require the targeted compatibility and nil/error tests described in the detail reports.

## Checkout identity

`git status --porcelain` is empty and HEAD is `b8374114d485b6957b15d8769d7d5d96ddeaafc6`. A SHA256 digest over each tracked path and its file bytes, in `git ls-files -z` order with separators, was computed before and after test execution. Both digests are `b8b97d2826205f5556b9713bdd1760b31afa7c92fc2294932197bb18aaf0ba37`, retained in `../review-probes/tree-before.sha256` and `tree-after.sha256`. The checkout has not been edited.
