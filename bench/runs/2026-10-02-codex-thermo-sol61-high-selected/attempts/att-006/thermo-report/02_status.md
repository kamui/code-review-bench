# Status detail compatibility

`WithDetails` preserves a v1 input contract, but `Details` now exposes the adapter used by the v2 implementation. Compatibility adaptation is asymmetric. The remedy is one canonical conversion at the output boundary, not another message registry or a special case for individual generated types.

## Source evidence

`git diff main...review-head -- internal/status/status.go status/status_test.go status/status_ext_test.go` shows `WithDetails` changing to `...protoadapt.MessageV1`, and its marshaling changing to `anypb.New(protoadapt.MessageV2Of(detail))`. This deliberately retains the previous input interface while using v2 for execution.

At `internal/status/status.go:158–163`, `Details` replaces `ptypes.DynamicAny` plus `ptypes.UnmarshalAny` with `Any.UnmarshalNew` and appends that v2 result directly. The file grows only from 204 to 205 lines. There is no size concern or need to decompose this cohesive operation.

Inspection of the cached, pinned `github.com/golang/protobuf@v1.5.3/ptypes/any.go` explains the behavior removed by the migration. `Empty` resolves the registered type and returns `proto.MessageV1(mt.New().Interface())`. `UnmarshalAny` populates the `DynamicAny.Message` with that value. The status implementation previously exposed this converted value.

The pinned `google.golang.org/protobuf@v1.32.0/protoadapt/convert.go` provides the symmetric `MessageV1Of` and `MessageV2Of` operations. They are the canonical way to keep implementation wrappers out of the public contract. No new adapter needs to be invented.

## Executed verification

The scratch `../repros/status_test.go` registers a legacy-only tagged message using the old runtime's `RegisterType`. It calls `status.New(codes.Internal, "error").WithDetails` with that message, then asserts the returned detail has the original concrete message type and field value.

At head, `WithDetails` succeeds, but `TestThermoLegacyStatus` fails with `Details type=*impl.messageIfaceWrapper want *thermoLegacy`. The same fixture passes when the base `internal/status/status.go` is supplied through the overlay. This demonstrates both a successful input and a broken output contract, rather than merely a failure to register or decode the fixture.

The existing status package tests pass. Their changed round-trip test declares `[]protoadapt.MessageV1` but casts both sides directly to `protoreflect.ProtoMessage` for comparison at `status/status_test.go:359`. Every included detail is a modern generated type, so these assertions do not reveal the legacy case. A legacy-only fixture would make such a direct v2 assertion invalid; the verification must adapt the comparison operands or use the v1 equality contract.

The reproduction tests the public status alias, not only an internal helper. It is not an end-to-end trailer test. Modern fixtures continue to pass in the existing tests; the review does not claim they become wrapped or unusable.

## Worked code-judo proposal

Keep the direct `Any.UnmarshalNew` implementation and convert only when the value crosses back to callers:

```go
for _, any := range s.s.Details {
    detail, err := any.UnmarshalNew()
    if err != nil {
        details = append(details, err)
        continue
    }
    details = append(details, protoadapt.MessageV1Of(detail))
}
```

This restores the old output conversion without bringing back `DynamicAny`, retaining a deprecated dependency, or checking for the runtime's private wrapper type. It is symmetric with the existing `WithDetails` input conversion. Modern messages implementing both interfaces are returned as their ordinary concrete values; adapted legacy messages are unwrapped by the canonical runtime conversion.

Keep the existing nil-status behavior, per-detail decode error placement, detail ordering, and immutable status construction. No additional mode, registry, public overload, or error-handling branch is necessary.

For fixture comparison, convert v1 operands through `protoadapt.MessageV2Of` before invoking v2 equality. Separately assert the legacy concrete type; equality of wire fields alone would allow the compatibility-wrapper leak to go unnoticed.

## Actionable remediation

Convert successfully decoded details through `protoadapt.MessageV1Of` before returning them. Add a registered legacy-only round trip, retaining the concrete-type assertion and decoded field check. Keep the modern and invalid-detail tests. The review verified the failure and the base behavior; the proposed remedy is source-level reasoning and has not been applied.
