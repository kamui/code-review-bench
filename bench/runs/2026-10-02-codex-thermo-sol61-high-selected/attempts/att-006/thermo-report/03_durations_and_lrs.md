# Checked durations and LRS error ownership

The duration migration separates validation from conversion while changing what validation guarantees. Two consumers lose the Go representability check, and the LRS consumer also discards its new validation error. These defects share a conversion boundary; an explicit checked operation can remove the incidental sequencing and stale-variable dependency.

## Evidence and measurements

`git diff main...review-head -- balancer/rls/config.go xds/internal/xdsclient/transport/loadreport.go` shows RLS's `convertDuration` returning `d.AsDuration(), d.CheckValid()` at line 310, while retaining nil-as-zero at lines 307–308. The file shrinks from 312 to 311 lines.

LRS now obtains `rInterval`, calls `CheckValid` without binding its error, and invokes `AsDuration` at lines 173–177. The transport file grows from 257 to 258 lines. The previous version used one `ptypes.Duration` call and formatted that call's error.

The cached pinned implementations establish a non-equivalence:

- `github.com/golang/protobuf@v1.5.3/ptypes/duration.go` validates the protobuf duration and then rejects multiplication/addition overflow when converting to Go's duration representation.
- `google.golang.org/protobuf@v1.32.0/types/known/durationpb/duration.pb.go:200` validates the protobuf range of approximately ±10,000 years, nanos range, sign consistency, and nil.
- `AsDuration` at line 171 explicitly saturates overflow to Go's maximum or minimum duration, whose magnitude is approximately 292 years.

A protobuf-valid duration is therefore not necessarily a representable Go duration. `CheckValid` succeeds on 315576000000 seconds; `AsDuration` saturates it.

In RLS, `parseRLSProto` obtains the lookup-service timeout, max age, and stale age through `convertDuration`. It returns an error when conversion fails. Max age is subsequently capped, but the lookup timeout is not. Accepting a saturated timeout changes the rejection contract and can produce a practically unbounded RPC deadline. The reproduction invokes the converter directly; it does not assert a complete service-config parse.

In LRS, a successful converted interval is used by `sendLoads` in `time.NewTicker(interval)` at line 137. A saturated positive interval is accepted and used for periodic reporting. The change does not add a negative-interval ticker panic: negative values were already accepted by the old converter when representable. That pre-existing issue is outside this review's findings.

## Executed overflow verification

`TestThermoOverflowDuration` in `../repros/balancer_rls_test.go` checks both 315576000000 and -315576000000 seconds, first confirming each satisfies `CheckValid`. At head, the conversion returns respectively `2562047h47m16.854775807s` and `-2562047h47m16.854775808s`, each with nil error. Both are rejected by the base converter, so the same expected-rejection test passes there.

`TestThermoLRSOverflowDuration` uses a stub stream returning one successful LRS response with the positive value. At head, the receiver returns the saturated interval and nil error. With the base transport file it returns a conversion error, so the test passes.

These checks establish conversion and receiver behavior without waiting for timers or starting a network server. They do not measure long-running production reporting. The existing RLS and transport package tests pass at head.

## Executed diagnostic verification

`TestThermoInvalidDurationDiagnostic` returns a successful LRS response with `Nanos: 1000000000`, which is invalid. At head the receiver returns exactly `invalid load_reporting_interval: <nil>`. With the base implementation the receiver returns a non-nil diagnostic containing a real validation failure, and the test passes.

The earlier `err` comes from `stream.Recv()`. The function already returns when it is non-nil, so every execution reaching the new `CheckValid` branch has `err == nil`. This is a deterministic data-flow defect, not a race or dependence on the stub's implementation.

A minimal local repair binds the new error:

```go
if err := rInterval.CheckValid(); err != nil {
    return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err)
}
```

This fixes only diagnostic ownership; it does not restore overflow rejection.

## Worked checked-conversion proposal

Make one internal conversion enforce both invariants. Since `CheckValid` guarantees normalized seconds and nanos, a round trip through the canonical constructor detects saturation directly:

```go
func checkedProtoDuration(d *durationpb.Duration) (time.Duration, error) {
    if err := d.CheckValid(); err != nil {
        return 0, err
    }
    value := d.AsDuration()
    roundTrip := durationpb.New(value)
    if roundTrip.Seconds != d.Seconds || roundTrip.Nanos != d.Nanos {
        return 0, fmt.Errorf("duration: %v is out of range for time.Duration", d)
    }
    return value, nil
}
```

This uses the existing duration model to eliminate manual multiplication limits and sign-overflow arithmetic. It accepts exactly representable valid values, including the maximum and minimum Go durations, and rejects a saturated result. It returns zero on failure, keeping the old converter's convention. The proposal is statically reasoned; it was not applied or executed as a remedy.

Keep nil policy at the consumer:

```go
// RLS: absent optional duration remains unset.
if d == nil {
    return 0, nil
}
return checkedProtoDuration(d)
```

LRS calls the checked converter directly and formats that operation's error:

```go
interval, err := checkedProtoDuration(resp.GetLoadReportingInterval())
if err != nil {
    return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err)
}
```

Place a shared checked converter in an appropriate internal utility if both packages use it; do not make xDS depend on the RLS balancer to reuse its private helper. The operation is small and has one precise contract. Avoid adding a nil-acceptance flag or duplicating bounds arithmetic in each caller.

## Actionable remediation

Restore overflow rejection at both conversion boundaries, capture the actual conversion error in LRS, and keep the separate nil policies. Retain focused cases for invalid nanos, absent LRS intervals, absent optional RLS values, exact Go limits, one-unit excursions beyond each limit, and valid ordinary positive intervals. The review's six contract checks cover the reported regressions; additional limit cases are suggested implementation verification, not claimed executed coverage.
