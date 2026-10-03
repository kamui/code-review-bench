# Protobuf boundaries: codec, status, and binary logging

This subsystem carries F2, F3, and F4 from the summary. The import-only appearance of the codec and binary-log changes hides a public message-contract change. Internal wire processing can use V2 uniformly, but application-owned V1 messages still need adaptation at entry, and legacy concrete detail types need restoration at exit.

## Source evidence and contract

`encoding/proto/proto.go:27` imports the new protobuf package. Its unchanged assertions at lines 41 and 49 now require `ProtoReflect`. In the base, the imported `proto.Message` requires only `Reset`, `String`, and `ProtoMessage`. The tests in `encoding/proto/proto_test.go` use `test/codec_perf.Buffer`; `test/codec_perf/perf.pb.go:65` supplies `ProtoReflect`, so the existing tests cannot distinguish these contracts.

`internal/status/status.go:134` explicitly preserves the V1 input interface through the `protoadapt.MessageV1` alias. Line 141 converts that interface to V2 for `anypb.New`. Lines 158–163 decode a V2 message and return it directly. The old implementation decoded through `ptypes.DynamicAny`. In the pinned legacy dependency, `ptypes.Empty` resolves the registered V2 message type and calls `proto.MessageV1` on the result. That inverse adaptation is the missing step.

`internal/binarylog/method_logger.go:242–250` and `:282–290` use the same duplicated payload serializer. Both now require V2 because the import at line 30 changed. Both log an informational unsupported-type message and produce an empty payload when the assertion fails. The outer protobuf log-entry construction remains valid, so a structurally valid log silently loses the application payload.

Application message objects reach those binary-log branches in `stream.go:908`, `:932`, `:1671`, and `:1747`, and `server.go:1372` and `:1470`. These paths do not replace legacy application messages with their V2 adapters before logging. The logging defect is therefore independent of the default-codec defect.

The existing `channelz/service/service.go:193` already uses `protoadapt.MessageV2Of` for a V1-owned value. The bridge exists in the pinned protobuf version and is already a repository convention; no reflection machinery or new compatibility framework is needed.

## Reproduction fixture

The scratch tests define a minimal valid legacy message with a protobuf field tag and the three V1 methods, deliberately excluding `ProtoReflect`:

```go
type reviewLegacy struct {
    Value string `protobuf:"bytes,1,opt,name=value,proto3" json:"value,omitempty"`
}
func (m *reviewLegacy) Reset() { *m = reviewLegacy{} }
func (m *reviewLegacy) String() string { return oldproto.CompactTextString(m) }
func (*reviewLegacy) ProtoMessage() {}
```

This exercises the documented legacy interface directly; it is not pretending that merely importing the old package makes modern generated types legacy-only. The baseline runtime can serialize the fixture. The status probe additionally registers it as `review.LegacyDetails`, successfully decodes the encoded status detail with the old `ptypes.DynamicAny`, and then inspects the concrete result of `Status.Details`.

The codec probe asserts both successful marshaling with exact expected wire bytes and successful unmarshaling into the V1 object. The binary-log probe asserts data and declared length for both directions. All preserve the original behavior as their expectation.

## Verification

| Probe | Pinned head | Affected base implementations |
| --- | --- | --- |
| `TestReviewLegacyCodec/marshal` | Rejects V1 message | Pass |
| `TestReviewLegacyCodec/unmarshal` | Rejects V1 destination | Pass |
| `TestReviewLegacyDetails` | Returns runtime wrapper | Pass: returns original concrete type |
| `TestReviewLegacyBinaryLog/client` | Empty payload, length 0 | Pass: 14-byte payload |
| `TestReviewLegacyBinaryLog/server` | Empty payload, length 0 | Pass: 14-byte payload |

The head codec error is `failed to marshal, message is *proto_test.reviewLegacy, want proto.Message`; unmarshaling reports the analogous error. The expected marshaled bytes are `0a0e6c6567616379207061796c6f6164`.

The head status probe reports `Details returned *impl.messageIfaceWrapper, want *reviewLegacy`. The old decoder control in that same test succeeds, proving the registration and encoded payload are valid.

The head binary-log probes report `logged payload =  (length 0); want 0a0c6c6f676765642076616c7565 (length 14)`. This measures actual data loss, not just a type assertion difference.

Existing package tests for `encoding/proto`, `status`, and `internal/binarylog` all pass. Existing green tests do not establish V1 compatibility.

## Worked code-judo proposal: normalize once at the codec boundary

A small conversion function can keep both codec methods direct. The purpose is to remove generation handling from the actual wire operation, using the existing canonical bridge:

```go
func messageV2(v any) proto.Message {
    switch m := v.(type) {
    case protoadapt.MessageV2:
        return m
    case protoadapt.MessageV1:
        return protoadapt.MessageV2Of(m)
    default:
        return nil
    }
}
```

`Marshal` and `Unmarshal` each call the conversion, retain their existing rejection for unsupported values, and use native `proto.Marshal` or `proto.Unmarshal`. V2-first dispatch avoids adapting ordinary modern messages. Tests should include an ordinary modern message, a V1-only message, invalid non-protobuf input, and the existing nil behavior. The helper is a proposal; it has not been applied or claimed to pass that full matrix.

If the same conversion is used across packages, a small internal protobuf-boundary utility can own it. There is no need for configurable generations, fallback modes, or a general serializer registry. Prefer a local helper until shared ownership earns its cost.

## Worked proposal: inverse adaptation at the status output boundary

Preserve the existing loop and error branch. Change only the successful append conceptually to:

```go
details = append(details, protoadapt.MessageV1Of(detail))
```

For registered V1-only messages, the inverse adapter unwraps the runtime representation into the original application type. For ordinary generated types that implement both interfaces, the concrete generated type remains available. Add a registered V1-only status detail test with a concrete type assertion; protobuf-content equality alone misses the regression. Keep decode errors in the returned slice and preserve immutability.

The cast changes at `status/status_test.go:359` should not become the test contract. Those assertions expressly assume V2 despite the public input still accepting V1. Either keep separate concrete compatibility tests or adapt both sides before native equality when a fixture is genuinely declared as V1.

## Worked proposal: one binary-log payload converter

Both message events need the same accepted payload types, marshaling operation, and logging. A local function can own those responsibilities:

```go
func messageBytes(v any) []byte {
    if b, ok := v.([]byte); ok {
        return b
    }
    m := messageV2(v)
    if m == nil {
        grpclogLogger.Infof("binarylogging: message to log is neither proto.message nor []byte")
        return nil
    }
    data, err := proto.Marshal(m)
    if err != nil {
        grpclogLogger.Infof("binarylogging: failed to marshal proto message: %v", err)
    }
    return data
}
```

Each event builder can then obtain `data := messageBytes(c.Message)` and construct its direction-specific log entry unchanged. This removes duplicated serialization branches rather than adding a V1 branch twice. The conversion helper's package location must be chosen deliberately; the sketch assumes it is available locally or in a small internal boundary utility. No dependency on mutable global codec registration is necessary.

Keep byte-slice passthrough, truncation after encoding, logger direction, sequence numbers, and timestamps as they are. In particular, restoring only the codec would leave these log builders with the narrowed assertions.

## Commands and retained evidence

The common environment and five-minute wrapper are recorded in [04_scope_and_verification.md](04_scope_and_verification.md). The decisive command arguments were:

```sh
go test -timeout=240s -run TestReview -v -overlay="$PROBE_DIR/head-overlay.json" ./encoding/proto ./status ./internal/binarylog ./balancer/rls ./xds/internal/xdsclient/transport ./internal/pretty
go test -timeout=240s -run TestReview -v -overlay="$PROBE_DIR/base-overlay.json" ./encoding/proto ./status ./internal/binarylog ./balancer/rls ./xds/internal/xdsclient/transport
```

`PROBE_DIR` denotes the sibling `../review-probes` directory, where the actual commands used absolute paths. `head-probes.log` and `base-probes.log` retain the output. The transport scratch compilation failure in these combined commands is separately disclosed; it does not affect the completed codec, status, logging, or RLS results.

The base overlay substitutes only the affected codec, status, binary-log, RLS, and LRS implementation files with `git show main:<path>` content. It does not modify the checkout and does not apply a proposed remedy.
