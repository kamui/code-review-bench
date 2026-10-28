# Thermo Nuclear Review: grpc/grpc-go#6919 (5051eeae..b8374114)

Scope: the change moves imports and call sites from `github.com/golang/protobuf` to `google.golang.org/protobuf` (68 files). I read every non-test hunk and the important test hunks, and compared each replaced `ptypes`/v1 helper against the v1.5.3 and v1.32.0 sources in the module cache. I also ran scratch experiments with `-overlay` on both the head tree and an extracted base tree. The clone was not modified. `go vet ./...` passes on head, the `examples` module builds, and focused tests pass for status, binarylog, rls, encoding/proto, httpfilter, clusterspecifier, xdsresource, xdslbregistry and testutils.

Most of the mechanical replacements behave the same as before:
- `ptypes.Is` → `MessageIs`
- `ptypes.UnmarshalAny` → `UnmarshalTo`
- `DurationProto`/`TimestampProto` → `durationpb.New`/`timestamppb.Now`
- the aliased `durationpb`/`timestamppb`/`wrapperspb`/`structpb` imports
- `WithDetails` taking `protoadapt.MessageV1`, which is the same type as the old v1 `proto.Message`

Two changes, though, quietly narrow a public, user-facing contract from "any APIv1 message" to "APIv2 messages only". The findings below are ordered by severity.

## Finding 1 — High: the default gRPC proto codec now rejects every message that implements only APIv1 (gogo/protobuf, pre-1.4 protoc-gen-go output, hand-written v1 messages)

**File:** `encoding/proto/proto.go`, lines 23-27 (import swap) and lines 40-54 (`codec.Marshal`/`codec.Unmarshal`).

**Evidence:** The import changed from `github.com/golang/protobuf/proto` to `google.golang.org/protobuf/proto`. The function bodies were left as they were: `vv, ok := v.(proto.Message)` followed by `proto.Marshal(vv)` / `proto.Unmarshal(data, vv)`.
- Before, `proto.Message` was `protoiface.MessageV1`, which only needs `Reset/String/ProtoMessage`. The v1 `Marshal`/`Unmarshal` wrapped such messages with `MessageV2`.
- Now `proto.Message` is `protoreflect.ProtoMessage`, which needs a `ProtoReflect()` method. APIv1-only types don't have one.

This codec is registered in `init()` as the default `"proto"` codec. Every unary and streaming RPC uses it unless the caller sets a custom codec.

I ran a scratch overlay test in `encoding/proto` with a v1-only message (struct with protobuf tags plus `Reset/String/ProtoMessage`):
- base: `marshal bytes=0a026869 err=<nil>`, `unmarshal out="hi" err=<nil>`
- head: `marshal ... err=failed to marshal, message is *proto.legacyMsg, want proto.Message` and the matching unmarshal failure.

**Failure:** Any application whose request/response types come from gogo/protobuf or older generated code still compiles, because the codec takes `any`. After upgrading, every RPC fails at runtime with an Internal "failed to marshal/unmarshal ... want proto.Message" error. That breaks existing functionality across the whole library, and the PR description ("migrate imports") gives no sign it is intended. The fix is to keep accepting `protoadapt.MessageV1` and convert with `protoadapt.MessageV2Of` (or accept either interface).

## Finding 2 — Medium: `Status.Details()` no longer returns the caller's concrete type for APIv1-only detail messages

**File:** `internal/status/status.go`, lines 152-166 (`Details`). This is exposed publicly through `status.Status`, an alias of `internal/status.Status`.

**Evidence:**
- Old code: `ptypes.UnmarshalAny(any, &ptypes.DynamicAny{})`. `ptypes.Empty` returns `proto.MessageV1(mt.New().Interface())`, which unwraps legacy wrapper types back to the user's original Go type.
- New code: `any.UnmarshalNew()`, which returns `mt.New().Interface()` directly. For legacy (APIv1-only) types registered through `github.com/golang/protobuf/proto.RegisterType`, that value is the internal `*impl.messageIfaceWrapper` (see `messageReflectWrapper.Interface` in `google.golang.org/protobuf@v1.32.0/internal/impl/message_reflect_gen.go:140-145`).

I ran a scratch overlay test in `status` that registers a v1-only `LegacyDetail` and round-trips it through `WithDetails` and `Details()`:
- base: `type=*status_test.LegacyDetail assertsToLegacyDetail=true`
- head: `type=*impl.messageIfaceWrapper assertsToLegacyDetail=false`

**Failure:** Clients that inspect error details with type switches or assertions (`case *mypb.QuotaFailure:`) on legacy-generated detail types silently stop matching. Retry, quota and error-handling logic built on rich error details then quietly falls through, with no compile error. Fixing it means wrapping the result with `protoadapt.MessageV1Of(detail)` before appending, which matches the old `DynamicAny` behavior.

The same narrowing to v2 also applies in `internal/binarylog/method_logger.go` lines 242 and 282 (`c.Message.(proto.Message)`). With binary logging enabled, the payloads of APIv1-only messages are no longer marshaled. They are dropped with the "message to log is neither proto.message nor []byte" log line. This is lower impact, but it has the same root cause and the same fix.

## Finding 3 — Low: the LRS load-reporting-interval error message always prints `<nil>`

**File:** `xds/internal/xdsclient/transport/loadreport.go`, lines 173-177.

**Evidence:** The new code is `if rInterval.CheckValid() != nil { return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err) }`. `err` is not the validation error. It is the variable from `resp, err := stream.Recv()`, which is guaranteed nil at that point because the earlier `if err != nil` already returned. The old code assigned `interval, err := ptypes.Duration(...)`, so the message carried the real cause.

**Failure:** When a management server sends a nil or out-of-range `load_reporting_interval`, the stream is torn down with "invalid load_reporting_interval: <nil>". The actual reason (nil Duration, seconds out of range, mismatched nanos sign) is lost, which makes LRS misconfiguration harder to debug. Rejection itself still works, because `CheckValid` returns an error for nil and out-of-range values.

## Finding 4 — Low: the conflict resolution downgrades the `test/tools` module's `golang.org/x/tools` from v0.17.0 to v0.14.0

**File:** `test/tools/go.mod`, lines 7 and 16, and `test/tools/go.sum`.

**Evidence:** The base commit 5051eeae ("Update go mod (#6939)") raised `golang.org/x/tools` to v0.17.0. The head's `test/tools/go.mod` pins `golang.org/x/tools v0.14.0` and adds `golang.org/x/sys v0.13.0 // indirect`. `go.sum` replaces the x/tools v0.17.0 and x/sync v0.6.0 hashes with v0.14.0 and v0.4.0. The protobuf migration of this module only needed `google.golang.org/protobuf` as a direct requirement in place of `github.com/golang/protobuf`. The downgrade looks like an artifact of resolving conflicts against an older branch state in commit b8374114, not an intended change.

**Failure:** `vet.sh` installs `goimports` from this module, so CI and local developer tooling silently roll back the dependency bump that landed just before this PR. They lose three minor versions of x/tools fixes and newer Go-version support. The next dependency update will also churn it again.
