# Protobuf migration maintainability review

Request changes. The bulk of the migration is direct and reduces deprecated API use, but the public
message boundaries do not preserve the old compatibility contract. Six executed contract checks
expose five behavioral issues across codec, logging, status details, and durations. The changed JSON
renderer also preserves a duplicate implementation that this migration makes unnecessary.

This review covers
`5051eeae537cb2839dd499e1a63a141098a3a03a..b8374114d485b6957b15d8769d7d5d96ddeaafc6`, inspected as
`git diff main...review-head`. It uses the frozen thermo-nuclear review skill and one primary
reviewer context. No other reviewers, prior reviews, forge discussions, or ambient repository
guidance were used.

## Findings

### Collapse the protobuf JSON rendering branches

In `internal/pretty/pretty.go:39–61`, the migration makes both protobuf branches use
`protojson.MarshalOptions` with the same effective indentation, but retains two copies of
marshaling, error fallback, and the Any-resolution explanation. The explicit `Multiline: true` in
the second branch creates no policy difference: protobuf v1.32.0 treats a nonempty `Indent` as
multiline. Normalize v1 inputs with `protoadapt.MessageV2Of` at the input boundary and render all
protobuf messages through one branch. This deletes an entire duplicate rendering path and removes
the remaining dependency on the old `proto` package from this helper while preserving the ordinary
JSON fallback. Verification is static against the pinned protobuf implementation; the worked
replacement and nil-handling considerations are in
[04_pretty_and_migration.md](04_pretty_and_migration.md).

### Preserve legacy messages at the default codec boundary

In `encoding/proto/proto.go:27`, changing the imported `proto` package rebinds the existing
assertions in `Marshal` and `Unmarshal` to the v2 interface, which requires `ProtoReflect`. A
message implementing the legacy `Reset`, `String`, and `ProtoMessage` methods now fails both
operations, although the same message works with the base codec. This changes the accepted RPC
payload contract during an import migration and makes existing clients or servers using old
generated messages fail before normal application handling. Normalize either message interface to v2
using the canonical `protoadapt` conversions, share that normalization between the two codec
methods, and retain the existing rejection of unrelated inputs. The overlay reproduction fails both
operations at head and passes against the base implementation;
[01_codec_and_binarylog.md](01_codec_and_binarylog.md) contains the evidence and worked boundary
design.

### Preserve legacy payloads in both binary-log directions

In `internal/binarylog/method_logger.go:31`, the new `proto` import also narrows the existing
assertions in `ClientMessage.toProto` and `ServerMessage.toProto`. A legacy protobuf message falls
into the unsupported-input branch and produces a normal message log entry with empty data and length
zero instead of its serialized payload. Both directions reproduce this loss at head, while the base
implementations record the expected eight bytes. Fix the logging boundary independently of the codec
so legacy messages remain loggable, including when an application uses a compatible custom codec.
Extract the repeated payload conversion into one local helper that accepts v1 messages, v2 messages,
and `[]byte`, using `protoadapt` for conversion and retaining the current logging behavior for
failures. [01_codec_and_binarylog.md](01_codec_and_binarylog.md) supplies the results and a concrete
simplification.

### Unwrap status details before returning them to callers

In `internal/status/status.go:158–163`, `Details` appends the direct result of `Any.UnmarshalNew`,
although `WithDetails` explicitly continues to accept `protoadapt.MessageV1`. For a registered
legacy message, `WithDetails` succeeds but `Details()[0]` is now `*impl.messageIfaceWrapper` rather
than the original concrete message type; callers' type switches stop recognizing the detail, and
direct type assertions can panic. The old `ptypes.DynamicAny` path performed the conversion back to
v1. Append `protoadapt.MessageV1Of(detail)` after a successful decode so the compatibility wrapper
stays inside the status implementation. Add a registered legacy-only round-trip case and avoid using
direct v2 assertions to verify fixtures that are declared as v1 messages. The concrete-type
reproduction fails at head and passes with the base implementation; [02_status.md](02_status.md)
explains the adapter symmetry and testing gap.

### Keep duration overflow rejection at both configuration boundaries

In `balancer/rls/config.go:310` and `xds/internal/xdsclient/transport/loadreport.go:173–177`,
replacing `ptypes.Duration` with `CheckValid` plus `AsDuration` removes rejection of values that are
valid protobuf durations but exceed Go's `time.Duration` range. A duration of 315576000000 seconds
now becomes 2562047h47m16.854775807s with no error; the base rejects it. RLS can consequently accept
an unrepresentable lookup timeout, and LRS can accept an interval that effectively stops periodic
reporting for centuries. Keep protobuf validity and Go representability as one checked conversion
contract, with nil-as-unset handled only by the RLS caller. A small conversion helper can validate,
convert, and compare `durationpb.New(converted)` with the original seconds and nanos to detect
saturation without hand-maintained integer-bound arithmetic. Positive and negative overflow
reproduce in RLS, and positive overflow reproduces in LRS; all pass against the base
implementations. [03_durations_and_lrs.md](03_durations_and_lrs.md) gives the evidence and worked
conversion.

### Return the LRS duration validation error

In `xds/internal/xdsclient/transport/loadreport.go:174–175`, the condition discards the error
returned by `rInterval.CheckValid()` and formats the earlier `stream.Recv()` error instead. That
earlier error is necessarily nil on this path, so an interval with out-of-range nanos returns
`invalid load_reporting_interval: <nil>` and loses the reason the server response was rejected. Bind
the validation error in the conditional, or format the error returned by the checked duration
conversion proposed above. This preserves the diagnostic boundary and removes reliance on an
unrelated variable. The invalid-nanos reproduction fails at head and passes against the base
implementation; [03_durations_and_lrs.md](03_durations_and_lrs.md) records the exact result.

## Question

In `test/tools/go.mod:7`, why does switching `protoc-gen-go` also downgrade `golang.org/x/tools`
from v0.17.0 to v0.14.0? Please identify a requirement for the downgrade or retain the base version.
This is a scope question, not a demonstrated defect;
[04_pretty_and_migration.md](04_pretty_and_migration.md) records the manifest evidence and
verification limit.

## Proposed remediation sequence

First make the external message contracts explicit. Retain both protobuf interface generations at
the codec and logging inputs, translate into v2 only for execution, and translate decoded status
details back through the compatibility adapter before exposing them. These changes belong at the
serialization boundaries. Internal xDS configuration interfaces can remain v2 because their concrete
messages already satisfy that interface.

Next make duration conversion a checked operation. Reuse a small internal converter for the two
affected consumers, preserve RLS's existing nil-as-unset policy outside it, and keep LRS's rejection
of a missing interval. Route the converter's actual error into the LRS diagnostic. Do not add a
boolean mode to combine those different nil policies.

Then remove the duplicate protobuf JSON rendering branch. Interface adaptation should choose a
normalized message; it should not select separate copies of JSON options, error handling, and
rendering. Resolve the tools downgrade question independently so the migration's dependency changes
remain explainable.

Finally keep the demonstrated contract cases as focused regression coverage. The codec needs legacy
input and output, binary logging needs both event directions, status needs the concrete legacy
detail type, and duration handling needs representable limits as well as overflow and invalid input.
Existing successful tests do not establish those contracts.

## Verification and scope

The committed range changes 68 files, with 165 insertions and 174 deletions. No changed file moves
from below 1,000 lines to 1,000 or more. Twelve changed files are already over that threshold, but
all twelve keep their line counts. This migration does not justify a demand to decompose those
unrelated existing files. Full file measurements are in
[file-measurements.json](file-measurements.json).

The existing focused command passed in eight packages; three additional packages compiled and
reported no test files. The packages were the codec, binary logging, status, RLS, pretty printing,
xDS transport, xDS resource parsing, the RLS cluster specifier, and the fault, RBAC, and router HTTP
filters. [verification.md](verification.md) contains the exact command and outcomes.

Six scratch-overlay contract tests failed at head and all six passed with the five affected
implementation files replaced by their base versions through a second overlay. These are
differential checks of the affected implementations within the head dependency graph, not a claim
that the entire base checkout was tested. Overlay tests used `-vet=off`; the separate existing-test
command used normal vet behavior. No remedies were applied, and no full repository suite or
dependency fetch was attempted.

The pretty-printing simplification is statically verified against the pinned protobuf source. It is
a maintainability finding, not an executed claim of broken JSON behavior. The tools-version question
has no demonstrated runtime failure. No performance conclusions are made.

The detail files retain subsystem evidence and worked proposals:
[01_codec_and_binarylog.md](01_codec_and_binarylog.md), [02_status.md](02_status.md),
[03_durations_and_lrs.md](03_durations_and_lrs.md), and
[04_pretty_and_migration.md](04_pretty_and_migration.md). The checkout's tracked file contents and
clean status were verified after the review.
