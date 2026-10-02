# Thermo-nuclear review of grpc/grpc-go#6919

Request changes. The migration mostly replaces deprecated APIs directly and does
not cause file-size growth, but it changes two compatibility boundaries and two
aspects of checked duration conversion. It also misses a clear opportunity to
delete a duplicate serialization path created by the migration.

This is one primary review context, using the frozen selected skill. No child
reviewers or alternate models were used. Only the committed range was reviewed:
`5051eeae537cb2839dd499e1a63a141098a3a03a..b8374114d485b6957b15d8769d7d5d96ddeaafc6`,
as shown by `git diff main...review-head`. No upstream discussion, later changes,
ambient guidance, or external review material was consulted.

The five actionable findings below are ordered by the selected skill’s emphasis:
first the concrete structural simplification, then the compatibility and typed
boundary defects. F2 and F3 are compatibility blockers. F4 and F5 are behavioral
regressions. F1 is a maintainability blocker under this skill’s explicit bar for
obvious deletion of incidental complexity; it is not a claim of new JSON failure.

## F1 — Collapse the now-identical protobuf JSON paths

In `internal/pretty/pretty.go:39–61`, the migrated legacy-message branch now calls the same `protojson` serializer as the modern-message branch, but retains a second options object, marshal call, error branch, fallback, and return. The previously meaningful split between `jsonpb` and `protojson` has become incidental complexity: a generated message implementing both interfaces takes the first branch, while a modern-only implementation takes the second, even though nonempty `Indent` already implies multiline output. Normalize the input to `protoadapt.MessageV2` once, then use one protobuf serialization path and one fallback; replace the remaining legacy import with `protoadapt`. This is a concrete opportunity to delete a whole duplicate serialization branch while preserving behavior. Verified by source inspection of both branches and the pinned serializer options; the proposed refactor was not applied. Full evidence and a worked proposal are in [the formatting report](03_formatting.md).

## F2 — Preserve legacy messages at the default codec boundary

In `encoding/proto/proto.go:27`, changing the imported `proto.Message` interface silently narrows both `Marshal` and `Unmarshal` from legacy protobuf messages to messages implementing `ProtoReflect`. Existing generated messages such as the repository’s `SearchRequestV3` now fail both operations with “want proto.Message”, so applications using older generated request or response types lose their default RPC encoding. Keep the implementation on the new protobuf runtime, but normalize legacy and modern messages through one codec-local adapter using `protoadapt.MessageV2Of`; reuse it for both operations and retain rejection of non-message values. The overlay probe fails both operations on the head and passes with the base codec implementation. Full evidence and a worked proposal are in [the message-boundary report](01_message_boundaries.md).

## F3 — Return original legacy detail types instead of runtime wrappers

In `internal/status/status.go:158–163`, `Details` now appends the raw result of `Any.UnmarshalNew`, although `WithDetails` still explicitly accepts `protoadapt.MessageV1`. For a registered legacy-only message, this leaks `*impl.messageIfaceWrapper` through the public status API instead of returning the concrete generated message that callers supplied and previously received. Existing type assertions and type switches therefore stop matching, or panic when they assert the concrete type. Convert successful decoded results with `protoadapt.MessageV1Of` before appending them, preserving the existing error entries and modern generated message types. A round trip using `SearchRequestV3` returns the wrapper on the head and the expected concrete type with the base status implementation. Full evidence and a worked proposal are in [the message-boundary report](01_message_boundaries.md).

## F4 — Preserve checked conversion to time.Duration

In `balancer/rls/config.go:310`, and likewise `xds/internal/xdsclient/transport/loadreport.go:173–177`, replacing `ptypes.Duration` with `CheckValid` plus `AsDuration` drops the previous rejection of values that overflow Go’s `time.Duration`. `CheckValid` accepts protobuf durations up to 10,000 years, while `AsDuration` silently saturates at the approximately 292-year Go limit. A valid protobuf value of 10,000,000,000 seconds now becomes `2562047h47m16.854775807s` with no error; RLS can accept an enormous lookup timeout and LRS can start a ticker that effectively never reports. Use a checked conversion that validates the protobuf and rejects any value changed by saturation, and reuse that explicit conversion contract at both boundaries while retaining RLS’s existing nil-as-zero policy. Head probes accept positive, negative, and nanosecond-boundary overflow in RLS and positive overflow in LRS; all are rejected by the base implementations. Full evidence and a worked proposal are in [the duration report](02_duration_boundaries.md).

## F5 — Report the actual LRS validation error

In `xds/internal/xdsclient/transport/loadreport.go:174–175`, the new condition discards the error returned by `rInterval.CheckValid` and formats the earlier `stream.Recv` error instead. That earlier error is necessarily nil on this path, so malformed control-plane intervals now produce `invalid load_reporting_interval: <nil>` and hide the cause of the stream rejection and retry. Bind the validation error in the condition and format that error, or propagate the checked-conversion error when implementing F4. An interval with 1,000,000,000 nanoseconds reproduces the nil diagnostic on the head and returns the actual validation cause with the base implementation. Full evidence and a worked proposal are in [the duration report](02_duration_boundaries.md).

## Remediation sequence

First restore the input and output contracts at the public boundaries. Keep the
new runtime, add a single message normalization helper to the default codec,
and unwrap decoded status details through the canonical adapter. Preserve the
concrete legacy type on the return path; converting only on input is insufficient.

Next restore checked protobuf-to-Go duration conversion. Share its validation
contract between RLS and LRS, preserve the existing RLS nil policy, and bind the
actual validation error locally. Do not turn a checked conversion API into an
unchecked conversion with a separate check for a different numeric range.

Finally collapse the formatting branches into input normalization followed by
one serializer call. The worked proposal removes the old protobuf import from
this module and keeps formatting, generic JSON behavior, and fallback ownership
in the existing package. No broad repository restructuring is needed.

Retain the compatibility and boundary probes as focused regression tests. Cover
old generated messages, modern generated messages, modern-only message objects,
non-message input, status concrete types, exact duration limits, missing durations,
and malformed intervals. These cases exercise the contracts that an import-only
test update does not protect.

## Verification and scope

The existing focused tests passed for `encoding/proto`, `status`, `balancer/rls`,
and `xds/internal/xdsclient/transport`. `internal/pretty` compiled and has no tests.
The four scratch probe files failed on the head exactly as described in F2–F5.
The same probes all passed when only the four affected implementation files were
overlaid with their base versions. This isolates the regressions without changing
the checkout; it is not a claim that the entire base suite was executed.

Commands used cached dependencies with `GOPROXY=off`, `GOSUMDB=off`,
`GOTOOLCHAIN=local`, `GOFLAGS=-mod=readonly`, explicit cache paths, a 300-second
command timeout, and a 240-second package-test timeout. Each package was tested
once per flag set. Overlay probes disabled vet because their test files exist
only in the scratch overlay; the existing package tests used normal vet behavior.
The full repository suite, dependency downloads, code generation, and nested
module test suites were not run.

The diff changes 68 files, with 165 insertions and 174 deletions. No changed file
crosses 1,000 lines. Maximum file growth is two lines. Already-large test and
transport files mostly have unchanged line counts, and their size is not a new
PR defect. The direct well-known-type substitutions and typed xDS Any methods
are appropriate simplifications; no additional actionable findings were identified
in those mechanical changes. Detailed coverage and measurements are in
[the migration-scope report](04_migration_scope.md).

Checkout identity was unchanged before and after test execution, including the
head commit, committed tree, tracked-file content digest, and clean status.
Evidence logs, probe sources, and line measurements are retained under
[`evidence/`](evidence/). The worked remedies are proposals only; none was applied.

## Questions

There are no outstanding questions. All actionable findings are stated above.
