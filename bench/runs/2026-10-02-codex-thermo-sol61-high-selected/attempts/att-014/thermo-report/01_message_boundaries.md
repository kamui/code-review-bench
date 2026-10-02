# Message compatibility boundaries

This subsystem covers the default protobuf codec, status details, and their
changed tests. F2 and F3 are confirmed behavioral regressions, not just different
names for equivalent interfaces. The architectural problem is asymmetric
normalization: the runtime migration changes implementation representation at
boundaries that still serve existing generated message types.

## Default codec evidence — F2

`encoding/proto/proto.go` remains 58 lines at both revisions. The only production
diff is the import replacement at line 27. Both assertions at lines 41 and 49
therefore acquire a stronger interface requirement with no visible local code
change. The old interface requires Reset, String, and ProtoMessage; the new one
requires ProtoReflect. The repository contains authentic older generated code
in `reflection/grpc_testing_not_regenerate/testv3.go`, including the exported,
registered `SearchRequestV3` with a Query field and no ProtoReflect method.

The existing codec test constructs `test/codec_perf.Buffer`, which has the modern
interface. Its passing result does not cover old generated user messages. The
scratch external-package test uses the registered default codec obtained through
`encoding.GetCodec("proto")`, constructs SearchRequestV3, and exercises both
Marshal and Unmarshal. The unmarshal wire bytes come from the old protobuf
runtime so it remains independently testable even when the new Marshal fails.

The head returns `failed to marshal, message is
*grpc_testing_not_regenerate.SearchRequestV3, want proto.Message`, and the analogous
unmarshal error. With the base codec implementation overlaid, both subtests pass
and the Query value survives. This directly proves the encoding boundary break;
no network RPC was required to reproduce it. RPC calls depend on this codec.

## Worked codec proposal

Use the protobuf library’s compatibility adapter at the codec-owned boundary,
rather than teaching RPC dispatch, clients, or each generated message about it.
One helper has an actual responsibility: normalizing either public input shape
for the one chosen runtime. Both methods then retain one serialization flow.

```go
func messageV2(v any) protoadapt.MessageV2 {
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
    m := messageV2(v)
    if m == nil {
        return nil, fmt.Errorf("failed to marshal, message is %T, want proto.Message", v)
    }
    return proto.Marshal(m)
}
```

Unmarshal should use the same helper and `proto.Unmarshal(data, m)`, preserving
its existing rejection text. The helper is local to the codec; no public wrapper
API, per-RPC compatibility flag, or additional serializer is needed. Verify old
messages, dual-interface generated messages, modern-only messages, ordinary
non-messages, and nil behavior before landing. This code is a proposal and was
not applied or executed.

## Status details evidence — F3

`internal/status/status.go` grows from 204 to 205 lines. WithDetails at line 134
retains the old contract via `protoadapt.MessageV1`, and at line 141 explicitly
adapts it for the new runtime. Details at lines 158–163 decodes through
`Any.UnmarshalNew` but returns that runtime object directly.

The pinned old helper explains the missing step: in
`github.com/golang/protobuf@v1.5.3/ptypes/any.go`, Empty resolves the message type
through the modern registry, then returns `proto.MessageV1(mt.New().Interface())`.
UnmarshalAny with DynamicAny thus returns the original legacy object shape. The
new adapter `protoadapt.MessageV1Of` provides that same conversion canonically.

The probe creates a status with codes.Internal, adds SearchRequestV3 as a detail,
then retrieves and asserts its concrete type. WithDetails succeeds in both
versions. Details returns `*impl.messageIfaceWrapper` on the head. The concrete
assertion succeeds, including the Query field check, with the base implementation
overlaid. This is a return-type regression observable through the public alias
`status.Status`; the internal package name does not contain the impact.

The migrated status tests use modern ResourceInfo, DebugInfo, and RetryInfo.
Their comparison asserts protoreflect.ProtoMessage on both operands, so those
tests exercise the modern shape and do not protect concrete legacy detail types.

## Worked status proposal

Keep the simplified Any decoding, including error entries in the output slice,
and restore the boundary conversion at the successful append:

```go
detail, err := any.UnmarshalNew()
if err != nil {
    details = append(details, err)
    continue
}
details = append(details, protoadapt.MessageV1Of(detail))
```

This deletes the old DynamicAny plumbing without leaking the replacement runtime
wrapper. MessageV1Of unwraps a legacy adapter and preserves a normal generated
message already satisfying both interfaces. Do not add bespoke type-name or
reflection-based unwrapping branches; the pinned library already owns this
representation boundary. This proposal was not applied. A regression test should
assert concrete type and fields, rather than only comparing serialized equality.

## Verification artifacts and commands

Source inspection used `git diff main...review-head -- encoding/proto status
internal/status`, `nl -ba` on the production files, and targeted reads of the
pinned cached protobuf implementations. Probes and raw outcomes are retained in
[evidence/probes](evidence/probes), [head-probes.log](evidence/head-probes.log),
and [base-probes.log](evidence/base-probes.log).

The exact common test environment and all three test commands are documented in
[the scope report](04_migration_scope.md). Existing codec and status tests passed;
the head compatibility probes failed; the base-implementation control probes
passed. No implementation remedy was applied, and no wider RPC integration suite
was executed.
