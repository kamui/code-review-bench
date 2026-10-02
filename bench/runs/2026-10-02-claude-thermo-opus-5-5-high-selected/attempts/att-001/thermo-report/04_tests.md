# 04 — Test code

Scope: `status/status_test.go`, plus the test helpers touched in `internal/testutils/marshal_any.go` and `internal/testutils/xds/e2e/clientresources.go`.

## Finding A — `status_test.go` builds its expected error through `protoimpl.X` and compares message text

`status/status_test.go:412` at head:

```go
[]any{
	protoimpl.X.NewError("invalid empty type URL"),
	&epb.ResourceInfo{ ... },
},
```

compared with

```go
func equalError(x, y error) bool {
	return x == y || (x != nil && y != nil && x.Error() == y.Error())
}
```

At base the expectation was `errors.New(`message type url "" is invalid`)`, which was already a string match against a dependency's wording. The new version is worse in kind. `google.golang.org/protobuf/runtime/protoimpl` is documented as being for generated code only, and `protoimpl.X.NewError` is used here because protobuf-go's error prefix is deliberately unstable (it varies between a space and a non-breaking space by build) to stop callers matching on it. The test reaches into the implementation package to reproduce the exact bytes that the library takes pains to prevent anyone depending on.

Verification status: verified by reading. `go test ./status/` passes at head.

What the test actually wants to establish is "an `Any` with an empty type URL yields an `error` in that slot of `Details()`, and the valid detail after it still decodes". Assert that:

```go
got := tc.s.Details()
if _, ok := got[0].(error); !ok {
	t.Errorf("Details()[0] = %T, want error", got[0])
}
```

or, keeping the table shape, give the table a sentinel (`wantErr: true`) and have the comparer treat any two non-nil errors as equal. Either removes the `protoimpl` import and stops the test from breaking when protobuf-go rewords a message.

## Finding B — double type assertion to compare details

`status/status_test.go:359` at head:

```go
if !proto.Equal(details[i].(protoreflect.ProtoMessage), tc.details[i].(protoreflect.ProtoMessage)) {
```

`proto.Message` is already an alias of `protoreflect.ProtoMessage`, so the new `reflect/protoreflect` import on line 33 exists only to spell an assertion that `proto.Message` would spell. The right-hand assertion exists because the table was retyped to `[]protoadapt.MessageV1` to fit the `WithDetails` signature.

Verification status: verified by reading.

This is the test-side shadow of the asymmetry described in `01_message-boundary.md`, Finding C: `WithDetails` speaks APIv1, `Details` now hands back APIv2, and the test has to cast on both sides to make them meet. With `Details()` restored to return what `WithDetails` was given, the line is:

```go
if !proto.Equal(protoadapt.MessageV2Of(details[i].(protoadapt.MessageV1)), protoadapt.MessageV2Of(tc.details[i])) {
```

or, simpler, keep `cmp.Diff(got, want, protocmp.Transform())` as the repository does elsewhere. Either way the `protoreflect` import goes.

## Note — three copies of "marshal to Any or die"

The PR edited all three of these and left all three in place:

- `internal/testutils/marshal_any.go:29` — `MarshalAny(t, m)`, calls `t.Fatalf`
- `internal/testutils/xds/e2e/clientresources.go:130` — `marshalAny(m)`, panics
- `status/status_test.go:444` — `mustMarshalAny(msg)`, panics

Before the PR they differed in which adapter they needed. After it, each is `anypb.New(m)` plus a failure mode. This predates the PR and is not raised as a finding in the summary, but the PR removed the last reason for them to differ, and it would have been the moment to collapse the two panicking variants into one.
