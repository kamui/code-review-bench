# Codec and binary-log message boundaries

The migration changes the meaning of existing assertions without changing their spelling. These are runtime boundaries accepting application-owned values through `any`, so switching an imported interface is an observable contract change. The default codec and binary logger need the compatibility adapter already used by `internal/status.WithDetails`.

## Evidence and measurements

`git diff main...review-head -- encoding/proto internal/binarylog` shows the codec import changing from `github.com/golang/protobuf/proto` to `google.golang.org/protobuf/proto`, with no corresponding adaptation in either method. The assertions at `encoding/proto/proto.go:41` and `:49` now require `ProtoReflect`. The imported v1 interface instead requires `Reset`, `String`, and `ProtoMessage`. Modern generated messages typically implement both interfaces; older generated messages need not implement the reflective interface.

The codec file remains 58 lines at both revisions. This is a boundary problem, not file growth. Its existing tests use `test/codec_perf.Buffer`; inspection of `test/codec_perf/perf.pb.go:65` confirms that fixture implements `ProtoReflect`. Those tests cannot exercise a legacy-only input.

`internal/binarylog/method_logger.go` grows from 445 to 446 lines. Its import at line 31 has the same interface change. The duplicated assertions at lines 242 and 282 independently reject legacy-only values. The fallback leaves `data` nil, then constructs a regular client/server message entry using `len(data)`, yielding an empty payload with length zero. Failure is logged at informational level; it is not returned to the caller.

`ClientMessage` and `ServerMessage` each repeat input classification, protobuf marshaling, the marshal-error log, byte-slice handling, and the unsupported-input log. Correcting only one path would leave the other broken. This duplication predates the PR, but repairing the new compatibility regression is a concrete opportunity to delete it.

## Executed differential checks

The scratch fixtures in `../repros/encoding_proto_test.go` and `../repros/internal_binarylog_test.go` define a tagged string field and only the three legacy message methods. The old runtime marshals `{Value: "legacy"}` as `0a066c6567616379`, eight bytes. This is an ordinary legacy message shape and deliberately has no `ProtoReflect` method.

`TestThermoLegacyCodec` checks both directions separately. At head, marshal returns `failed to marshal, message is *proto.thermoLegacy, want proto.Message`; unmarshal returns the analogous error. With the base codec file supplied through the base overlay, both operations pass. The independent old-runtime marshal establishes that the fixture itself is valid.

`TestThermoLegacyBinarylog` invokes both configurations' `toProto` methods. At head, both report `data= length=0` rather than the expected eight bytes. With the base logger implementation, both pass. This is a direct conversion check, not an end-to-end RPC or sink test; it verifies the payload loss at its source.

Existing codec and binary-log package tests pass at head. Exact invocation flags, overlays, and package results are in [verification.md](verification.md). No remedies were implemented or tested.

## Worked codec proposal

Keep one internal execution interface, but accept both supported interfaces at the application boundary. A local helper shared by `Marshal` and `Unmarshal` makes that rule explicit:

```go
func messageV2(v any) proto.Message {
    switch m := v.(type) {
    case protoadapt.MessageV1:
        return protoadapt.MessageV2Of(m)
    case protoadapt.MessageV2:
        return m
    default:
        return nil
    }
}
```

Both methods then obtain `vv := messageV2(v)`, retain their existing error for an unsupported value, and invoke v2 `proto.Marshal` or `proto.Unmarshal`. This helper earns its place by sharing a real boundary contract across two operations; it does not wrap the execution API or introduce a new codec mode. `protoadapt` is the canonical adapter, and no reflection tricks or copied compatibility implementation are needed.

Check untyped nil, typed nil pointers, a modern generated message, a legacy-only message, and a non-message input when implementing this change. The executed review fixture establishes the legacy regression; it does not claim coverage of every nil form or custom runtime behavior.

## Worked logging proposal

Use a single local payload helper for both message event builders:

```go
func messageData(v any) []byte {
    var m proto.Message
    switch value := v.(type) {
    case []byte:
        return value
    case protoadapt.MessageV1:
        m = protoadapt.MessageV2Of(value)
    case protoadapt.MessageV2:
        m = value
    default:
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

Each builder can replace its repeated conversion block with `data := messageData(c.Message)` and keep the event type, logger side, payload metadata, and truncation behavior. This deletes one full copy of the classification and failure policy. It preserves the existing informational failure behavior instead of creating an error API for the logger.

Do not route this through a globally registered codec merely to share conversion: that introduces registration and custom-codec behavior into protobuf logging. Do not export the codec's private adapter or create a public compatibility API for this repair. Small local boundary helpers using the existing adapter keep ownership clear.

The binary-log fix remains independently necessary. Fixing the default codec permits legacy RPCs again but does not make the logger's v2 assertions succeed. Applications using a compatible custom codec can also reach binary logging without the default codec accepting their value.

## Actionable remediation

Restore both message generations at the codec and logger inputs; centralize each package's repeated conversion where it is actually shared. Retain a legacy-only fixture in both package tests, with both codec operations and both log directions asserted. Verify the bytes and length rather than only requiring that an entry was produced.
