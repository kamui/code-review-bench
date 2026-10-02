# Codec and status compatibility boundaries

## Scope and judgment

This subsystem covers the default protobuf codec, status details, their changed tests, and the repository's existing legacy generated messages. F2 and F3 are public compatibility regressions. Migrating the internal engine should not silently narrow the codec's accepted message interface or expose protobuf implementation wrappers through `Details()`.

The codec stays at 58 lines. The internal status implementation grows from 204 to 205 lines. These are type-boundary problems, not file-size or conditional-growth problems. The changed tests use modern generated messages, which implement both interfaces; passing those tests does not establish V1-only compatibility.

## F2 evidence: the default codec loses V1-only messages

The only implementation change in `encoding/proto/proto.go` is the import at line 27. The assertions at lines 41 and 49 now require `ProtoReflect()`. The previous interface required `Reset`, `String`, and `ProtoMessage` and accepted V1-only generated code. A type assertion against the new interface does not automatically use protobuf's compatibility adapter.

`reflection/grpc_testing_not_regenerate/testv3.go` supplies actual retained generated code. `SearchRequestV3` has the V1 methods and a legacy descriptor but no `ProtoReflect`. It is registered by the generated package. This evidence does not depend on a hand-written imitation of protobuf generation.

The standalone probe in [verification/legacy_probe.go](verification/legacy_probe.go) marshaled `SearchRequestV3{Query: "legacy request"}` using the old API. It produced bytes `0a0e6c65676163792072657175657374` and no error. The registered head codec then returned:

```text
head codec marshal: failed to marshal, message is *grpc_testing_not_regenerate.SearchRequestV3, want proto.Message
head codec unmarshal: failed to unmarshal, message is *grpc_testing_not_regenerate.SearchRequestV3, want proto.Message
```

Since this codec is the default codec, the narrowed interface affects RPC request and response processing wherever applications retain V1-only generated messages. The reproduction directly proves rejection by both codec methods; it does not claim a separately executed end-to-end RPC failure.

## F2 worked code-judo proposal

Use the canonical adapter once at the codec's input boundary, and reuse the same normalization for encode and decode. This small helper earns its abstraction by defining the accepted contract in one place instead of duplicating generation-dependent assertions:

```go
func messageV2Of(v any) protoadapt.MessageV2 {
    switch m := v.(type) {
    case protoadapt.MessageV1:
        return protoadapt.MessageV2Of(m)
    case protoadapt.MessageV2:
        return m
    default:
        return nil
    }
}

func (codec) Marshal(v any) ([]byte, error) {
    m := messageV2Of(v)
    if m == nil {
        return nil, fmt.Errorf("failed to marshal, message is %T, want proto.Message", v)
    }
    return proto.Marshal(m)
}

func (codec) Unmarshal(data []byte, v any) error {
    m := messageV2Of(v)
    if m == nil {
        return fmt.Errorf("failed to unmarshal, message is %T, want proto.Message", v)
    }
    return proto.Unmarshal(data, m)
}
```

The adapter returns existing V2 messages directly and wraps V1-only messages for the V2 engine. No reflection inspection, descriptor guessing, custom serialization, or scattered RPC-layer checks are needed. Keep rejection of non-message values local to each operation so the diagnostic still names the operation and original input.

Verification required after remediation: marshal and unmarshal the retained legacy generated type and a modern message; assert decoded field values; confirm non-message inputs still return errors. Check nil and typed-nil handling against the existing boundary before introducing any new policy. The proposal is not applied or compiled as a fix in this review.

## F3 evidence: status output leaks runtime wrappers

The changed `WithDetails` declaration at `internal/status/status.go:134` deliberately retains `protoadapt.MessageV1`. Packing at line 141 uses `MessageV2Of`. This is a clean input boundary and keeps the public method source-compatible because `MessageV1` is an alias for the original protobuf interface.

The corresponding output boundary is incomplete. At lines 158–163, `Any.UnmarshalNew()` creates a V2 message and `Details()` returns it directly. For legacy registered types, that message is a protobuf runtime wrapper rather than the caller's generated object.

The base used `ptypes.UnmarshalAny` with `DynamicAny`. The cached v1.5.3 `ptypes/any.go` shows that `Empty` resolves the registered message type and calls `proto.MessageV1(mt.New().Interface())`. That last conversion is the behavior lost here.

The same real generated `SearchRequestV3` produced the following output in the standalone probe:

```text
WithDetails: <nil>
head Details: type=*impl.messageIfaceWrapper original_type=false
adapted Details: type=*grpc_testing_not_regenerate.SearchRequestV3 original_type=true value=query:"legacy request"
base detail operation: type=*grpc_testing_not_regenerate.SearchRequestV3 err=<nil>
```

This affects the concrete types stored in the `[]any`, even though decoding succeeds and the protobuf bytes are unchanged. A type switch over the generated message no longer matches, and an unchecked assertion to that message panics. Current status tests assert modern generated messages as `protoreflect.ProtoMessage`; they do not cover the legacy concrete-type contract.

## F3 worked remediation

Keep `Any.UnmarshalNew()` and its existing error handling, but adapt the successful result at the return boundary:

```go
detail, err := any.UnmarshalNew()
if err != nil {
    details = append(details, err)
    continue
}
details = append(details, protoadapt.MessageV1Of(detail))
```

The cached v1.32.0 adapter implementation unwraps V2 runtime wrappers to their original V1 objects, and leaves messages already implementing V1 unchanged. The standalone probe verifies that this conversion recovers the original legacy concrete type and value. It also matches the base's conversion for registered V2-only types, which the old API represented through a V1 adapter.

This is a canonical boundary conversion, not a new wrapper abstraction. No changes to the public `WithDetails` signature or the immutable status representation are necessary. Add a registered V1-only detail test that checks concrete type and content after a round trip, while retaining the existing malformed-Any and modern message tests.

## Verification status

`go test ./encoding/proto ./status ./internal/pretty ./balancer/rls ./xds/internal/xdsclient/transport -count=1 -timeout=240s` passed on the unchanged head; pretty has no tests. The compatibility probe executed successfully and printed the failures above. It exercised public codec/status APIs and base-equivalent old protobuf operations using cached versions. No baseline checkout, full repository suite, race testing, or remedy execution occurred.

Source inspection commands included `git diff main...review-head -- encoding/proto internal/status status`, `cat encoding/proto/proto.go`, numbered status source, and reads of cached `protoadapt/convert.go`, `ptypes/any.go`, and `internal/impl/api_export.go`. All dependency evidence is from the versions pinned by the target's `go.mod`.
