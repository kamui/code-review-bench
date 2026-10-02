# Thermo-nuclear code quality review: grpc-go PR #6919 (migrate to google.golang.org/protobuf)

Range: `5051eeae..b8374114` (`main...review-head`), 68 files, +165/-174. Detail files: `01_codec_status_pretty.md` and `02_xds.md`. I read the full non-test diff and sampled the test diff. I built the touched non-test packages and ran the tests for `status`, `encoding/proto`, `internal/pretty` and `internal/status`; build and tests passed. I did not run the full suite.

## Verdict

Do not approve as it stands. Most of the PR is a clean, mechanical rewrite (`ptypes.UnmarshalAny` to `UnmarshalTo`, `DurationProto` to `durationpb.New`, import path swaps), and no file crosses 1000 lines. But the part that is not mechanical contains a real error-reporting bug, a silent behaviour change in the default codec, an unfinished migration that leaves two proto stacks side by side, and an unrelated toolchain downgrade. The structural opportunity (deleting the v1 dependency outright) was missed in `internal/pretty`.

## Findings

**The LRS interval check reports a stale error.** In `xds/internal/xdsclient/transport/loadreport.go` (lines 173-177) the old code reported the error from `ptypes.Duration`. The new code calls `rInterval.CheckValid() != nil` and then formats the `err` left over from `stream.Recv()`, which is `nil`. A server that sends an invalid `load_reporting_interval` produces "invalid load_reporting_interval: <nil>", so the cause is lost. The fix is to scope the error with `if err := rInterval.CheckValid(); err != nil`. Full evidence is in `02_xds.md`, Finding 5.

**The default proto codec now rejects messages that used to work.** `encoding/proto/proto.go` (lines 40-53) used to assert `v.(proto.Message)` against the golang/protobuf v1 interface, and now asserts against the v2 `protoreflect.ProtoMessage`. Any message that only implements the v1 methods marshalled before and now fails with "want proto.Message". This is the codec for all RPCs, so a "just change the import" edit has become a compatibility change with no test and no mention. The remedy is a single `messageV2Of` helper built on `protoadapt.MessageV2Of`, used by both `Marshal` and `Unmarshal`, which also removes the duplicated assertion and error text. See `01_codec_status_pretty.md`, Finding 1.

**The migration is half done, and it dragged tooling versions backwards.** Non-generated code still imports `github.com/golang/protobuf` in `credentials/credentials.go`, `xds/internal/xdsclient/bootstrap/bootstrap.go` (`jsonpb`), `internal/pretty/pretty.go` and `channelz/service`, so the root `go.mod` still requires it directly. Meanwhile `test/tools/go.mod` and `go.sum` downgrade `golang.org/x/tools` from v0.17.0 to v0.14.0 and `x/sync` to v0.4.0, which has nothing to do with the stated purpose and looks like a bad merge or tidy. Finish the job or document the remaining packages, and revert the unrelated version changes. See `02_xds.md`, Finding 6.

**`internal/pretty` keeps a legacy import to run a duplicate branch.** In `internal/pretty/pretty.go` (lines 27-58) the `protov1.Message` arm now does what the `protov2.Message` arm below it does, converting via `protov1.MessageV2` just to reuse `protojson`. Generated v2 messages also satisfy the v1 interface, so the v2 arm is effectively unreachable for them. The code-judo move is to collapse the two arms into one marshal-with-fallback body keyed on a single type and delete the golang/protobuf import from this package. See `01_codec_status_pretty.md`, Finding 2.

**The duration-validation idiom was copied twice in different shapes.** `balancer/rls/config.go` (line 310) returns `d.AsDuration(), d.CheckValid()` together, while `loadreport.go` validates and converts in two steps, and that divergence is what produced the stale-error bug above. One small helper that preserves `ptypes.Duration`'s "zero on error" contract would serve both. See `01_codec_status_pretty.md`, Finding 4.

**The status tests picked up casts and an unstable-API error.** `status/status_test.go` (lines 359 and 412) double-casts to `protoreflect.ProtoMessage` and forges the expected error with `protoimpl.X.NewError`, tying the test to library error text. Keeping the expectation typed as `proto.Message` and asserting only that the element is an `error` removes both. The exported `WithDetails` signature also now exposes `protoadapt.MessageV1`; it is source compatible but deserves a doc note. See `01_codec_status_pretty.md`, Finding 3.

## Proposed remediation sequence

First fix the `loadreport.go` error scoping, since it is a correctness bug, and add a small test with an out-of-range duration. Second, restore v1-message tolerance in the codec with a `messageV2Of` helper plus a test using a v1-only message. Third, revert the `test/tools` module downgrades. Fourth, either complete the migration (`bootstrap.go`, `credentials.go`, `pretty.go`) so `github.com/golang/protobuf` can leave the root `go.mod`, or state explicitly which packages remain. Fifth, collapse `internal/pretty` to one branch and introduce the shared duration helper. Last, tidy the status test casts.

## Questions for the author

Is the intent to remove `github.com/golang/protobuf` from the root module? If so, why do `credentials`, `bootstrap` and `pretty` still import it? Was the `test/tools` downgrade deliberate?
