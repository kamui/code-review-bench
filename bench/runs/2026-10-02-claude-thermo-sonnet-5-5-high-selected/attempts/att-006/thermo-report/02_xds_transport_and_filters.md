# Detail 02: xDS call sites

Scope: `xds/internal/xdsclient/transport/loadreport.go`, `xds/internal/xdsclient/xdsresource/*`, `xds/internal/httpfilter/*`, `xds/internal/clusterspecifier/rls/rls.go`, and associated tests. Verification: read the diff; `go vet ./xds/internal/xdsclient/transport/` is clean. The vet pass does not flag the issue below because `err` is a legitimately declared variable in scope.

## D2.1 Stale `err` in `recvFirstLoadStatsResponse` (`xds/internal/xdsclient/transport/loadreport.go:173-177`)

Status: verified by reading the code at HEAD.

```go
resp, err := stream.Recv()
if err != nil { return ... }
...
rInterval := resp.GetLoadReportingInterval()
if rInterval.CheckValid() != nil {
	return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err)
}
interval := rInterval.AsDuration()
```

The old code was `interval, err := ptypes.Duration(...)` followed by `if err != nil`, so `err` carried the failure. The migration split the check from the value and discarded the error from `CheckValid()`. The `err` formatted in the message is the earlier `stream.Recv()` error, which is necessarily nil at this point. A malformed or missing `load_reporting_interval` therefore produces `invalid load_reporting_interval: <nil>`, which hides the actual reason (nil duration, out-of-range seconds, sign mismatch). The code still rejects the response, so behavior of accept/reject is preserved, but the diagnostic is wrong. This is a regression introduced by the PR and it compiles cleanly, which is exactly how this class of mistake survives review.

Remedy: bind the error where it is produced, so a stale variable cannot be reused:

```go
if err := rInterval.CheckValid(); err != nil {
	return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err)
}
```

Because `resp.GetLoadReportingInterval()` is nil-safe and `CheckValid` on a nil `*Duration` returns an error, the `nil` case still behaves like the previous `ptypes.Duration(nil)`.

Code-judo proposal: a small helper next to `convertDuration` in the RLS balancer, or a single `internal/` helper `durationFromProto(d *durationpb.Duration) (time.Duration, error)`, would replace both hand-rolled `CheckValid`/`AsDuration` pairs and make this mistake impossible to repeat. Two call sites is borderline, so this is optional; the one-line fix above is required.

## D2.2 Mechanical `ptypes.UnmarshalAny` to `Any.UnmarshalTo` and `ptypes.Is` to `MessageIs` conversions

Status: verified by reading; no findings. In `unmarshal_lds.go`, `fault.go`, `rbac.go`, `router.go`, `rls.go` and `filter_chain.go` the replacements are one-to-one. In `unmarshal_lds.go` the `if cfg.MessageIs(s) { cfg.UnmarshalTo(s) }` pair still does two type-URL checks (as before). This is unchanged behavior and not worth a finding.

## D2.3 Interfaces still expose v1 types where the diff touched them

`xds/internal/httpfilter/httpfilter.go` and `clusterspecifier/cluster_specifier.go` now import `google.golang.org/protobuf/proto` in place of golang/protobuf. `proto.Message` there is a type alias to the same underlying interface family, so implementers outside the PR keep compiling. That is a good example of the migration being mostly import-only, and it is why I rate the risk of the rest as low.
