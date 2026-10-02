# Strict duration conversion and LRS validation

## Scope and judgment

This subsystem covers `balancer/rls/config.go` and `xds/internal/xdsclient/transport/loadreport.go`. F4 is a shared semantic mismatch between the old checked conversion and the new pair of protobuf methods. F5 is a distinct error-variable regression in the LRS response path. The changes remain small: the RLS file shrinks from 312 to 311 lines and load reporting grows from 257 to 258 lines. Neither needs a large-file decomposition.

The canonical protobuf methods are appropriate for encoding Go durations. They are not a behavior-equivalent replacement for strict decoding without an additional representability check. RLS optionality and LRS diagnostics belong in their current callers; strict numeric conversion can share one explicit implementation.

## F4 evidence: validity is not representability

At `balancer/rls/config.go:306–310`, nil still maps to zero, but non-nil input now returns `d.AsDuration(), d.CheckValid()`. At `loadreport.go:173–177`, LRS checks protobuf validity and then calls `AsDuration()`.

Cached `github.com/golang/protobuf@v1.5.3/ptypes/duration.go` explicitly rejects both protobuf invalidity and overflow of `time.Duration`. It checks the seconds multiplication and the nanos addition. Cached `google.golang.org/protobuf@v1.32.0/types/known/durationpb/duration.pb.go` specifies that `AsDuration` returns the closest representable duration on overflow. `CheckValid` only checks protobuf's ±10,000-year range and nanos consistency.

The standalone probe compared both methods for positive/negative 10,000,000,000 seconds and for the positive Go boundary. The old conversion rejected the large values and `9223372036 seconds + 854775808 nanos`; the head's new operation accepted all three. Both APIs accepted `9223372036 seconds + 854775807 nanos`, which is exactly `math.MaxInt64` nanoseconds.

Scratch overlays directly called both changed functions. The RLS test observed saturated positive and negative values with nil error. The LRS test observed:

```text
overflow duration: interval=2562047h47m16.854775807s error=<nil>
```

The overlay tests assert preservation of the old rejection behavior and fail on the unchanged head. This is not an inference from deprecation text alone.

In RLS, `convertDuration` serves lookup timeout, maximum age, and stale age. Large lookup timeouts now pass parsing; large cache ages may reach downstream capping or comparison policies instead of being rejected. In LRS, the returned interval feeds `time.NewTicker` in `sendLoads`, so a huge positive interval delays subsequent load reporting for centuries while the stream remains open. Negative overflow also changes rejection into acceptance by the converter, although rejection of ordinary negative/zero LRS intervals is an existing separate concern and is not a new finding here.

## F4 worked code-judo proposal

Use a strict conversion with a lossless round-trip check. This avoids copying protobuf's sign/range checks and avoids hand-writing multiplication overflow logic. An internal utility, for example beside the existing duration functions in `internal/grpcutil`, can own this exact numeric contract:

```go
func ProtoDurationToDuration(d *durationpb.Duration) (time.Duration, error) {
    if err := d.CheckValid(); err != nil {
        return 0, err
    }
    value := d.AsDuration()
    roundTrip := durationpb.New(value)
    if roundTrip.Seconds != d.Seconds || roundTrip.Nanos != d.Nanos {
        return 0, fmt.Errorf("duration %v is out of range for time.Duration", d)
    }
    return value, nil
}
```

A protobuf-valid duration is normalized by its seconds/nanos sign rules; reconstructing it after conversion therefore distinguishes exact conversion from saturation, including the one-nanosecond overflow boundary. Use field comparison rather than generic serialization or reflection for this two-field invariant.

RLS can keep its existing local optional-field policy:

```go
func convertDuration(d *durationpb.Duration) (time.Duration, error) {
    if d == nil {
        return 0, nil
    }
    return grpcutil.ProtoDurationToDuration(d)
}
```

LRS should call the strict helper on the required response interval and wrap its error with the field name. This keeps nil handling explicit: the helper rejects nil, RLS locally permits it, and LRS keeps rejecting it. Do not replace other pre-existing saturating duration uses unless their owning contracts also require strict conversion.

The remedy should have boundary tests at exact positive/negative `time.Duration` limits, one nanos step beyond each, large protobuf-valid values, mismatched nanos signs, out-of-range nanos, and nil policy at each caller. The proposal has been reasoned from the cached implementations and reproduction; it has not been applied or executed as a fix.

## F5 evidence: the validation error is thrown away

At `loadreport.go:165`, `resp, err := stream.Recv()` creates the function's `err`. The function immediately returns if that error is non-nil. Thus at the changed line 174, the earlier `err` must be nil. `if rInterval.CheckValid() != nil` checks a new error without storing it, and line 175 formats the previous nil error.

The overlay provides a successful receive with `Duration{Nanos: 1000000000}` and directly calls `recvFirstLoadStatsResponse`. The returned error is exactly:

```text
invalid load_reporting_interval: <nil>
```

The caller still retries the failed LRS exchange, so this finding concerns loss of the diagnostic cause rather than claiming validation is bypassed. The existing `lrsRunner` logs the returned error as `Reading from LRS stream failed`; that is where operators need the invalid field's reason.

## F5 integrated remediation

The smallest isolated correction captures `if err := rInterval.CheckValid(); err != nil` and formats that `err`. The combined repair with F4 is cleaner because it binds validation, representability, and the error at one conversion boundary:

```go
interval, err := grpcutil.ProtoDurationToDuration(resp.GetLoadReportingInterval())
if err != nil {
    return nil, 0, fmt.Errorf("invalid load_reporting_interval: %w", err)
}
```

This replaces the validity-only sequence, preserves the field context, retains the actual cause, and eliminates the opportunity to refer to a stale receive error. It does not add transport feature branches or change report orchestration. Test malformed nanos, nil intervals, and excessive seconds and assert that the returned error includes the cause instead of `<nil>`.

## Verification status and commands

Existing RLS and xDS transport package tests passed. Existing transport load-report tests cover a normal 50ms interval; they do not establish strict conversion behavior for excessive durations or diagnostic correctness for invalid intervals.

The command below ran each overlay package once under its stated flag set, using only scratch additions:

```text
go test -overlay=<report>/verification/overlay.json ./balancer/rls ./xds/internal/xdsclient/transport -run '^TestThermo' -count=1 -v -timeout=60s
```

`TestThermoDurationOverflow`, `TestThermoLRSInvalidDuration`, and `TestThermoLRSDurationOverflow` all failed for the expected regressions. The overlay's LRS stub embeds the client interface and implements only `Recv`, which is the only stream method exercised by the function under test. It returns a real Envoy protobuf response. No fixture network calls are needed for these probes.

Sources and commands include `git diff main...review-head -- balancer/rls xds/internal/xdsclient/transport`, numbered load-report and RLS source, `parseRLSProto` caller inspection, and reads of the pinned old/new duration implementations. Full command environment and observations are retained in [verification/README.md](verification/README.md).
