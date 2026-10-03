# Thermo-nuclear code quality review: grpc-go #6919 (migrate to google.golang.org/protobuf)

Range: `5051eeae..b8374114`. 68 files, +165/-174. No file crosses 1000 lines. Verification was static (diff reading plus targeted greps); no tests were executed. Detail files: `01_core-library.md`, `02_xds.md`, `03_build-and-incomplete-migration.md`.

## Verdict

Do not approve as-is. Most of the diff is a clean mechanical import swap that deletes wrapper layers (`ptypes.UnmarshalAny` to `Any.UnmarshalTo`, `ptypes.DurationProto` to `durationpb.New`, `DynamicAny` to `UnmarshalNew`), which is exactly the right direction. But the migration contains one real behavioural regression, one accidental dependency downgrade, one structural duplication in `internal/pretty`, and it stops short of removing the legacy dependency it set out to remove.

## Findings

**1. Error is dropped in the load-report interval validation (regression).** In `xds/internal/xdsclient/transport/loadreport.go:173-177`, the old `interval, err := ptypes.Duration(...)` was replaced by `rInterval.CheckValid() != nil` whose result is thrown away, and the error message still formats `err`, which at that point is the nil error from `stream.Recv()`. Callers will see "invalid load_reporting_interval: <nil>" and the real cause is lost. The fix is a scoped `if err := rInterval.CheckValid(); err != nil`. Details in `02_xds.md`, Finding C.

**2. `test/tools/go.mod` downgrades `golang.org/x/tools` from v0.17.0 to v0.14.0 (and x/sync to v0.4.0).** A protobuf-import PR has no reason to touch linter tool versions; this looks like a conflict-resolution or `go mod tidy` artifact. See `test/tools/go.mod:8` and the matching `go.sum` hunk. Restore the previous versions and keep only the intended edits. Details in `03_build-and-incomplete-migration.md`, Finding E.

**3. `internal/pretty/pretty.go` now has two nearly identical marshalling branches.** The `protov1.Message` case (lines 39-47) and the `protov2.Message` case (lines 49-62) both build `protojson.MarshalOptions`, call `Marshal`, and fall back to `%+v` with the same comment; they differ only by `Multiline: true` and a `protov1.MessageV2` adapter call, and the v1 branch keeps the `golang/protobuf` import alive. Since every generated v2 message also satisfies the v1 interface, the second case is effectively unreachable. Collapse into one path using `protoadapt.MessageV2Of` and delete the v1 import. Also note that `jsonpb` output was stable while `protojson` output is deliberately unstable, so any exact-match consumer needs checking. See `01_core-library.md`, Finding A.

**4. The migration is incomplete while claiming to be repository-wide.** Non-generated code still imports `github.com/golang/protobuf` in `credentials/credentials.go:31`, `xds/internal/xdsclient/bootstrap/bootstrap.go:32` (jsonpb), `channelz/service/*.go` (ptypes, wrappers), `reflection/grpc_testing_not_regenerate/testv3.go`, and `internal/pretty/pretty.go:27`. The root `go.mod` still lists it as a direct requirement, while `examples/go.mod` was flipped to `// indirect`, so the modules now disagree. Finish the conversion (`jsonpb` to `protojson`, `ptypes.Duration` to `AsDuration`) or state the follow-up. See `03_build-and-incomplete-migration.md`, Finding F.

**5. `Status.WithDetails` exposes `protoadapt.MessageV1` on the public API, and tests grow casts and an unstable hook.** `internal/status/status.go:134` changes the signature (public through the `status.Status` alias); it is source compatible only by alias coincidence. In `status/status_test.go:359` each element is cast to `protoreflect.ProtoMessage` on both sides of `proto.Equal`, and at line 412 the test depends on `protoimpl.X.NewError`, an explicitly unstable surface. Type the test table with v2 messages, adapt at the single `WithDetails` call site, and match errors without `protoimpl.X`. See `01_core-library.md`, Finding B.

**6. Missed simplification in xDS Any handling.** `unmarshal_lds.go` (around lines 121-130 and 171-175) and similar sites translate `ptypes.Is` + `UnmarshalAny` into `MessageIs` + `UnmarshalTo`, keeping two allocations and two near-identical case bodies for the TypedStruct v1/v2 types. `Any.UnmarshalNew` with a type switch would delete the `MessageIs` calls. This is a follow-up level suggestion, not a blocker. See `02_xds.md`, Finding D.

**7. Import-group churn.** Several files lose blank-line grouping while moving imports (for example `examples/route_guide/server/server.go`, `balancer/grpclb/grpclb.go`). Cosmetic; see `03_build-and-incomplete-migration.md`, Finding G.

## Proposed remediation sequence

First fix the real defects: finding 1 (scoped error) and finding 2 (restore tool versions). Then finish or explicitly scope finding 4, since it determines whether finding 3's `protov1` import can go. Do finding 3 immediately after that so `pretty.go` has one marshal path. Clean up the test-side casts in finding 5, and treat findings 6 and 7 as optional follow-ups.

## What was done well

The wrapper deletions are real simplifications: `timestamppb.Now()` removes an ignored error, `UnmarshalNew` removes `DynamicAny`, and the `internal/testutils` helpers lose their `protoadapt` shims. No file sizes grew meaningfully and no new branching was added to existing flows apart from the `pretty.go` duplication.
