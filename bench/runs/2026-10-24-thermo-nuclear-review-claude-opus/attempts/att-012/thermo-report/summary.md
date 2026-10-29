# Thermo Nuclear Review — grpc/grpc-go#6919

Range: `5051eeae537cb2839dd499e1a63a141098a3a03a..b8374114d485b6957b15d8769d7d5d96ddeaafc6` (`git diff main...review-head`, 68 files).

Intent: move protobuf imports and call sites from `github.com/golang/protobuf` (APIv1) to `google.golang.org/protobuf` (APIv2) across the repository.

## Verification performed

- `go build ./...` and `go vet ./...` on the root module: clean. `go build ./...` in `examples` and `test/tools`: clean. The other submodules could not resolve their dependencies offline. Those submodules are unchanged by this PR.
- Focused tests pass at head: `encoding/proto`, `status`, `internal/binarylog`, `balancer/rls`, `xds/internal/httpfilter/...`, `xds/internal/clusterspecifier/...`, `xds/internal/xdsclient/xdsresource/...`, `xds/internal/xdsclient/transport`, `xds/internal/xdsclient/xdslbregistry/...`.
- I ran overlay-only scratch tests (files kept in the work directory, clone untouched). They exercise the default codec and `status.Details()` with an APIv1-only message, meaning a type that has `Reset/String/ProtoMessage` but no `ProtoReflect`. Each test ran twice: once against head, and once with the base version of the changed file swapped in through `-overlay`.
- The other migrated call sites check out as equivalent:
  - `ptypes.Is` → `Any.MessageIs` and `ptypes.UnmarshalAny` → `Any.UnmarshalTo`: same suffix/`/` boundary rule, same mismatch and nil-`Any` errors.
  - `ptypes.DurationProto` → `durationpb.New` and `ptypes.TimestampProto(time.Now())` → `timestamppb.Now()`.
  - `ptypes/wrappers`, `ptypes/struct`, `ptypes/duration`, `ptypes/timestamp` → `types/known/*`: these are type aliases.
  - `ptypes.MarshalAny` → `anypb.New(protoadapt.MessageV2Of(..))`.
  - `WithDetails(...protoadapt.MessageV1)`: same underlying alias `protoiface.MessageV1`, so no API break.
  - `jsonpb` → `protojson` in `internal/pretty`: debug output only.

## Findings

### [P1 / High] Default `proto` codec now rejects every message that does not implement APIv2 (`ProtoReflect`). This breaks RPCs that use gogo or pre-APIv2 generated messages.

**File:** `encoding/proto/proto.go`, lines 27, 40–54.

**Evidence:** The PR swaps the import from `github.com/golang/protobuf/proto` to `google.golang.org/protobuf/proto`. The codec body stays the same: `vv, ok := v.(proto.Message)` in both `Marshal` (line 41) and `Unmarshal` (line 49).
- Before, `proto.Message` was `protoiface.MessageV1` (`Reset/String/ProtoMessage`). The v1 `proto.Marshal`/`proto.Unmarshal` wrapped legacy messages through `protoimpl.X.ProtoMessageV2Of`.
- Now, `proto.Message` is `protoreflect.ProtoMessage`, which requires `ProtoReflect()`.
- Messages from protoc-gen-go before 1.4 / golang/protobuf before 1.4, and gogo/protobuf messages, only implement MessageV1. They now fail the type assertion.

This is the default codec, the one `rpc_util` uses when no other codec is set, so every unary and streaming RPC that sends such a message fails on both client and server. I confirmed it with an overlay test that sends a legacy struct-tagged message through `encoding.GetCodec("proto")`:
- Head: `Marshal` → `failed to marshal, message is *proto.legacyMsg, want proto.Message`, and `Unmarshal` → `failed to unmarshal, message is *proto.legacyMsg, want proto.Message`.
- Base file through overlay: `Marshal` → `[10 1 120] <nil>`.

**Failure:** After upgrading, any application whose messages are not APIv2-generated has every RPC fail at runtime with `codes.Internal` marshal/unmarshal errors. The code still compiles (generated stubs pass `any`), so nothing catches it before production. The PR's goal is an import migration, not dropping APIv1 message support. The codec should accept both forms, e.g. by switching on `protoadapt.MessageV1` and wrapping it with `protoadapt.MessageV2Of`.

### [P2 / Medium] `status.Details()` now returns internal wrapper objects instead of the user's concrete type for registered APIv1-only detail messages.

**File:** `internal/status/status.go`, lines 152–166 (it is reached through the public `status.Status.Details()` and `status.FromError(...).Details()`).

**Evidence:** The old code used `ptypes.UnmarshalAny(any, &ptypes.DynamicAny{})` and appended `detail.Message`. That value had gone through `protoimpl.X.ProtoMessageV1Of`, so legacy messages came back as their original Go type. The new code appends the raw result of `any.UnmarshalNew()`, which is the APIv2 view. For a legacy type registered with `github.com/golang/protobuf/proto.RegisterType`, that view is `*impl.messageIfaceWrapper`. I confirmed this with an overlay test in package `status_test` that registers a legacy detail type and runs `WithDetails(&legacyDetail{...}).Details()`:
- Head: `detail type *impl.messageIfaceWrapper, is *legacyDetail: false`.
- Base `internal/status/status.go` through overlay: `detail type *status_test.legacyDetail, is *legacyDetail: true`.

The error entries for undecodable details also change. They are now `protoregistry`/`protoimpl` errors such as `invalid empty type URL` instead of `message type url "" is invalid`. The PR edits `status/status_test.go` to accept the new text, so that part looks deliberate.

**Failure:** Clients that inspect error details with type switches or assertions such as `d.(*mypb.MyDetail)` on gogo or pre-APIv2 detail types silently stop matching. Error-handling branches (retry hints, quota info, custom details) get skipped with no compile error or runtime error. Well-known `errdetails` types are APIv2 and are unaffected. Returning `protoadapt.MessageV1Of(detail)` would keep the old behavior.

### [P3 / Low] The LRS `invalid load_reporting_interval` error drops the validation error and always prints `<nil>`.

**File:** `xds/internal/xdsclient/transport/loadreport.go`, lines 173–177.

**Evidence:** `if rInterval.CheckValid() != nil { return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err) }` discards the `CheckValid()` result. The `err` it formats is the one from `resp, err := stream.Recv()` (line 165), and that is always `nil` at this point because the function already returned on a non-nil `err`. Before, `err` was the `ptypes.Duration` error. Nil and out-of-range intervals are still rejected, since `durationpb.CheckValid` returns `invalid nil Duration` for nil, so control flow is right.

**Failure:** When a management server sends a bad interval, operators see `invalid load_reporting_interval: <nil>` in logs and stream-failure errors and cannot tell why the LRS stream keeps restarting.

### [P3 / Low] The merge resolution downgrades the pinned `golang.org/x/tools` in the tools module from v0.17.0 to v0.14.0, reverting the base's #6939 bump.

**File:** `test/tools/go.mod`, lines 7 and 16 (plus the matching `test/tools/go.sum` edits).

**Evidence:**
- Base `main` (commit 5051eeae, "grpc: Update go mod (#6939)") pins `golang.org/x/tools v0.17.0`.
- Head pins `golang.org/x/tools v0.14.0` and adds `golang.org/x/sys v0.13.0 // indirect`.
- `go.sum` swaps `x/tools v0.17.0` / `x/sync v0.6.0` for `x/tools v0.14.0` / `x/sync v0.4.0` / `x/sys v0.13.0`.

Nothing in the PR's intent needs this; it looks like stale `go.mod` content left over from the "resolve conflicts" merge commit. `vet.sh -install` runs `go install golang.org/x/tools/cmd/goimports` from `test/tools`, so developers and CI get the older goimports. The module still builds offline. I could not run `go mod tidy -compat=1.19` (the `vet.sh` tidy gate) to completion offline, because the module graph needs a network lookup.

**Failure:** This quietly undoes a dependency update that just landed. Developers and CI fall back to an older goimports pin, and the next routine dependency bump will keep fighting this change.

## Non-findings checked

- `balancer/rls/config.go` `convertDuration`: `d.AsDuration(), d.CheckValid()` now saturates durations between about 292 and 10000 years instead of erroring. All callers clamp or default these values (`maxAge`/`staleAge` are capped at 5 minutes, and a huge lookup timeout is harmless), so this has no practical effect.
- The interfaces in `xds/internal/httpfilter` and `xds/internal/clusterspecifier` change their parameter from v1 to v2 `proto.Message`. All implementations are internal and build and vet cleanly. Every caller passes `*anypb.Any`.
- `internal/pretty.ToJSON`: protojson's output whitespace is intentionally unstable, but it is only used for logging. No tests compare against it.
