# Checked duration boundaries

This subsystem covers RLS configuration and the first xDS load-reporting response.
F4 is a conversion-contract regression at both sites. F5 is a separate diagnostic
regression at the LRS site. Both are confirmed by scratch overlay probes and a
base-implementation control. Existing happy-path tests pass.

## Conversion evidence — F4

`balancer/rls/config.go` shrinks from 312 to 311 lines. Its convertDuration helper
preserves nil-as-zero at lines 307–309 but now returns `d.AsDuration(),
d.CheckValid()` at line 310. parseRLSProto uses this helper for lookup-service
timeout, max age, and stale age at lines 210, 222, and 226. The code treats the
returned error as the validation decision and stores the returned durations.

`xds/internal/xdsclient/transport/loadreport.go` grows from 257 to 258 lines. At
lines 173–177 it checks the protobuf duration and then converts with AsDuration.
The resulting interval reaches sendLoads, which creates a time.Ticker at line
137. This is a runtime scheduler boundary, not a protobuf-only data copy.

The pinned `ptypes.Duration` implementation validates the protobuf shape and
then checks overflow when multiplying seconds by time.Second and adding nanos.
Its documented contract explicitly returns an error for time.Duration overflow.
The new Duration.CheckValid validates approximately ±10,000 years; Duration.AsDuration
explicitly returns the closest representable duration on overflow. Combining
them therefore validates a different numeric domain from the old helper.

The RLS probe supplies valid protobuf durations of +10,000,000,000 seconds,
−10,000,000,000 seconds, and 9,223,372,036 seconds plus 854,775,808 nanoseconds
(one nanosecond above MaxInt64). The head accepts all three with nil errors,
saturating to MaxInt64 or MinInt64 nanoseconds. The LRS probe accepts the positive
10,000,000,000-second value with nil error and MaxInt64 duration. With the two
base implementation files overlaid, every overflow case is rejected.

For lookupServiceTimeout this means an invalid Go-duration configuration now
becomes a roughly 292-year timeout. For LRS it becomes a roughly 292-year reporting
ticker. maxAge has a later five-minute cap, so not every RLS field produces the
same operational consequence; that cap does not restore the helper’s rejection
contract. No claim is made that positive overflow causes a ticker panic. Zero
and negative ticker intervals are a preexisting concern, not a new finding here.

## Worked conversion proposal

Make the numeric boundary explicit once. One internal checked-conversion helper
can own protobuf validity and Go representability; callers retain their own
missing-value policy. A normalized round trip catches both saturation limits
and nanosecond-edge overflow without duplicating arithmetic:

```go
func checkedDuration(p *durationpb.Duration) (time.Duration, error) {
    if err := p.CheckValid(); err != nil {
        return 0, err
    }
    d := p.AsDuration()
    roundTrip := durationpb.New(d)
    if roundTrip.Seconds != p.Seconds || roundTrip.Nanos != p.Nanos {
        return 0, fmt.Errorf("duration %v is out of range for time.Duration", p)
    }
    return d, nil
}
```

Keep the RLS nil check outside this shared contract, since nil means defaulting
there. LRS should keep rejecting a missing reporting interval. Compare typed
fields directly; there is no need for string comparison, proto.Equal, a generic
validator registry, or scattered checks for every RLS field. The shared helper
has substantive boundary semantics rather than merely wrapping AsDuration.
This is a worked proposal only, with no helper package or checkout edit created.

Tests should include zero, ordinary signed values, exact MaxInt64/MinInt64,
one nanosecond beyond each bound, oversized seconds, nil, out-of-range nanos,
and seconds/nanos sign mismatch. These are independent numeric contracts and
must not be inferred solely from a happy-path load-reporting test.

## Diagnostic evidence — F5

In recvFirstLoadStatsResponse, `err` comes from stream.Recv at line 165. The
function returns immediately for any non-nil receive error at lines 166–167.
Consequently the error formatted at line 175 is always nil. CheckValid at line
174 can reject a missing duration, malformed nanos, inconsistent signs, or a
protobuf-range overflow, but its actual error is discarded.

A fake stream supplies a LoadStatsResponse with Nanos equal to 1,000,000,000.
The head returns `invalid load_reporting_interval: <nil>`. The base implementation
reports the actual nanos validation failure. lrsRunner logs this error before
retrying the stream at lines 122–125, so this affects operational diagnostics,
not just a string in an isolated utility test.

Bind the error to the validation operation that owns it:

```go
if err := rInterval.CheckValid(); err != nil {
    return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err)
}
```

When implementing F4, use `interval, err := checkedDuration(rInterval)` instead
and retain the local error return. That resolves both findings in one typed
conversion flow. The proposed remedy was not applied. The diagnostic probe tests
for a meaningful error rather than copying a version-dependent protobuf error
string into the expectation.

## Verification status

The existing RLS and xDS transport tests passed once each. The head overflow and
diagnostic probes failed as described. The same probes passed with base versions
of config.go and loadreport.go overlaid. Tests called the production parsing
helpers, with a fake LRS stream; they did not wait on a long-lived ticker or
assert timeout timing. The scheduler and configuration consequences follow from
the inspected callers, and are clearly distinguished from the tested outputs.

Source inspection used `git diff main...review-head -- balancer/rls
xds/internal/xdsclient/transport`, numbered reads of the two files, and cached
reads of `ptypes/duration.go` and `types/known/durationpb/duration.pb.go`.
Raw output is in [head-probes.log](evidence/head-probes.log) and
[base-probes.log](evidence/base-probes.log); complete probe sources are in
[evidence/probes](evidence/probes). Exact test commands are in
[the scope report](04_migration_scope.md).
