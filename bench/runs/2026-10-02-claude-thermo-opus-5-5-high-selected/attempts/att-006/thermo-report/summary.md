# Thermo-nuclear code quality review — grpc/grpc-go#6919

Migration of protobuf imports and call sites from `github.com/golang/protobuf` to `google.golang.org/protobuf`.

Review range `5051eeae537cb2839dd499e1a63a141098a3a03a..b8374114d485b6957b15d8769d7d5d96ddeaafc6`, inspected with `git diff main...review-head` (68 files, 165 insertions, 174 deletions).

## Verdict

Not approvable as it stands. There are eight actionable findings, three of which are behavioural regressions confirmed by execution.

Most of the diff is what it claims to be: a mechanical, behaviour-preserving swap of well-known-type imports and `ptypes` helpers in code that only ever handles generated xDS, grpclb and test messages. That part is fine, and no file crosses the 1000-line threshold because of it.

The problem is that the PR treats the whole migration as an import rename, and it is not one. The two modules both export a type spelled `proto.Message`, but the old one is the APIv1 interface (`Reset`/`String`/`ProtoMessage`) and the new one is the APIv2 interface (`ProtoReflect`). Wherever grpc-go type-asserts a caller-supplied `any` against `proto.Message`, changing the import silently changes which user types are accepted. The old `proto.Marshal` adapted APIv1-only messages internally; the new one does not. The PR recognised this in exactly one place (`Status.WithDetails`), handled it with a different adapter in a second place (`pretty.ToJSON`), and missed it in the three places that matter most: the default codec, binary logging, and `Status.Details`. That is a boundary that needed to be made explicit once and was instead left implicit in four different shapes.

The remaining findings are smaller: a mistranslated error path, an unrelated tool-dependency downgrade that rode in on a merge, a test that now imports a generated-code-only package to forge an expected error string, and a migration that stops short without a guard against regression.

## Findings

### 1. The default proto codec now rejects APIv1-only messages

In `encoding/proto/proto.go` lines 40-54 the bodies of `codec.Marshal` and `codec.Unmarshal` are unchanged, but the import under their `v.(proto.Message)` assertions moved from `github.com/golang/protobuf/proto` to `google.golang.org/protobuf/proto`, so the assertion now demands `ProtoReflect()` and any message that implements only `Reset`/`String`/`ProtoMessage` (output of `protoc-gen-go` before v1.4, gogo/protobuf, hand-written legacy types) fails with `failed to marshal, message is *T, want proto.Message`. This is the codec registered as `proto`, so every RPC on such a service breaks after upgrading. I confirmed it by execution: an overlay test with a V1-only message returns that error for both directions at head and round-trips correctly (`[10 1 110]`, then `"n"`) when only `proto.go` is swapped back to base. A pure import swap is the wrong shape for this file; the codec is the boundary where grpc-go accepts user message types and it needs to say what it accepts. The remedy is a small `messageV2Of(v any) proto.Message` that returns V2 messages as-is, adapts `protoadapt.MessageV1` through `protoadapt.MessageV2Of`, and returns nil otherwise, used by both methods, plus a test with a V1-only type. Full evidence and code are in `01_v1_message_boundary.md`.

### 2. Binary logging silently drops the payload of APIv1-only messages

In `internal/binarylog/method_logger.go` lines 242-251 (`ClientMessage.toProto`) and 282-291 (`ServerMessage.toProto`) the same import swap narrowed `c.Message.(proto.Message)` to APIv2, so a V1-only message falls through to the final `else`, logs "message to log is neither proto.message nor []byte" at info level, and the emitted `GrpcLogEntry` carries `Length: 0` and no data. This is worse than finding 1 in one respect: nothing fails, the audit record is just empty. Confirmed by execution: head produces `len=0 data=[]` for both client and server entries, base `method_logger.go` produces `len=3 data=[10 1 110]`. The two blocks were already copy-pasted before this PR, the PR had to edit the file anyway, and it left both copies with the same defect. Collapse them into one `marshalPayload(msg any) []byte` that uses the same V1-to-V2 helper as the codec, so the two `toProto` methods differ only in their event type. While there, `timestamp := timestamppb.Now(); m.Timestamp = timestamp` at lines 92-93 is a leftover temporary from the old two-value call and should be one assignment. See `01_v1_message_boundary.md`.

### 3. `Status.Details()` no longer returns what `Status.WithDetails()` accepts

`internal/status/status.go` line 134 deliberately keeps the input typed as `...protoadapt.MessageV1` and adapts on line 141, which is correct and source-compatible. The output side at lines 152-166 was not given the same care: `any.UnmarshalNew()` returns an APIv2 message and it is appended as-is, whereas base went through `ptypes.DynamicAny`, which unwrapped legacy messages to their concrete Go type. Confirmed by execution: for a V1-only detail, head returns `*impl.messageIfaceWrapper` (a type from protobuf-go's internal package that does not implement APIv1 and that callers cannot name), base returns the caller's own `*legacyDetail`. User code that type-switches on `st.Details()` silently stops matching, and feeding a returned detail back into `WithDetails` panics on the type assertion. The contract is now V1 in, V2 out, undocumented. The fix is one call, `details = append(details, protoadapt.MessageV1Of(detail))`, which unwraps legacy wrappers and returns dual-API generated messages unchanged, with a doc sentence stating the round-trip guarantee and a V1-only case in `TestStatus_ErrorDetails`. The branch history has a commit titled "add compatibility test for status" but no such test survives in the tree. The double cast through a newly imported `protoreflect.ProtoMessage` at `status/status_test.go` line 359 is fallout from the same asymmetry and goes away with the fix. See `01_v1_message_boundary.md`.

### 4. LRS interval validation throws away its own error and reports `<nil>`

In `xds/internal/xdsclient/transport/loadreport.go` lines 173-177 the old `interval, err := ptypes.Duration(...)` became `if rInterval.CheckValid() != nil { return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err) }`. The result of `CheckValid()` is never bound; the `err` being formatted is the one from `stream.Recv()` on line 165, which is necessarily nil at that point because line 166 already returned otherwise. The error is therefore always the literal text `invalid load_reporting_interval: <nil>`, and it only compiles because a stale `err` happened to be in scope. Confirmed by reading, not executed. Write it as `if err := interval.CheckValid(); err != nil { ... }` and convert with `AsDuration()` at the single return on line 190, which also removes the `rInterval`/`interval` pair. The same validate-then-convert idiom was written a second, different way in `balancer/rls/config.go` lines 306-311 as `return d.AsDuration(), d.CheckValid()`, which returns a non-zero value alongside an error and quietly drops the old `time.Duration` overflow check; its callers happen to be safe, but both sites should use the same boring form. See `02_duration_and_error_call_sites.md`.

### 5. `test/tools/go.mod` downgrades `golang.org/x/tools` from v0.17.0 to v0.14.0

`test/tools/go.mod` line 7 at head pins `golang.org/x/tools v0.14.0`; base has v0.17.0, set by the base commit itself (`5051eeae grpc: Update go mod (#6939)`). `go.sum` moves `x/sync` from v0.6.0 to v0.4.0 and adds `x/sys v0.13.0` to match. None of this has anything to do with protobuf; the intended change in this module is swapping the `protoc-gen-go` tool import in `tools.go` and its `require` line. The head commit is "resolve conflicts and add changes", so this looks like the branch's stale file winning a conflict. It rolls back the `goimports` that `vet.sh` installs from this module and guarantees churn in the next dependency bump. Confirmed from the diff and file history; I could not build the tools module because `x/tools` is not in the provided module cache. Regenerate from `main`'s `go.mod` and `go.sum`, change only `tools.go`, and run `go mod tidy` in `test/tools`. See `03_build_and_migration_hygiene.md`.

### 6. `pretty.ToJSON` keeps two proto branches that are now the same branch

In `internal/pretty/pretty.go` lines 39-61 the `protov1.Message` case used to call `jsonpb.Marshaler` and now calls `protojson.MarshalOptions{Indent: jsonIndent}` on `protov1.MessageV2(ee)`; the `protov2.Message` case below it calls `protojson.MarshalOptions{Multiline: true, Indent: jsonIndent}` on `ee`. `protojson` documents that a non-empty `Indent` implies `Multiline`, so after this PR the two branches are the same code with the same duplicated fallback and comment, differing only in an adapter call. The file also still imports `github.com/golang/protobuf/proto` on line 27 just to name the V1 interface and its adapter, while the rest of the PR uses `protoadapt` for that. This is a refactor that changed the implementation without noticing it had removed the reason for the branch. Normalise to V2 once with the same helper proposed in finding 1 and keep a single proto branch; that deletes about ten lines, the import alias pair, and one of the remaining uses of the old module. See `01_v1_message_boundary.md`.

### 7. The status test imports `runtime/protoimpl` to forge an expected error string

`status/status_test.go` line 412 replaces the expected value `errors.New(...)` with `protoimpl.X.NewError("invalid empty type URL")`, compared by `equalError` on `.Error()` text (lines 429-431). `runtime/protoimpl` is documented as "should only ever be imported by generated messages", and this is now the only hand-written file in the repository that imports it. The reason it is needed is that protobuf-go deliberately randomises the `proto:` prefix so callers do not compare error strings; the test defeats that safeguard by calling the library's private constructor and is still pinned to the exact wording inside `anypb`. `Details()` promises only that an undecodable detail is replaced by an error, so the test should assert that the first element is a non-nil `error` and stop there. That removes the import and stops the next protobuf-go release from breaking the test by rewording a message. See `02_duration_and_error_call_sites.md`.

### 8. The migration stops short and adds no guard against regression

After the PR, non-generated code still imports the old module in `channelz/service` (four files, `ptypes`), `xds/internal/xdsclient/bootstrap/bootstrap.go` line 473 (`jsonpb.Unmarshaler`), `internal/pretty/pretty.go`, `credentials/credentials.go` and the frozen `testv3.go`. The last two are legitimate (exported API typed on the old interface, and a deliberately frozen generated file). `pretty.go` and `bootstrap.go` are simply unfinished, and channelz was migrated and reverted twice on the branch with nothing in the tree explaining why. `go.mod` therefore still lists `github.com/golang/protobuf` as a direct requirement, and `vet.sh` is untouched: lines 152-157 still blanket-ignore every staticcheck deprecation hit mentioning `"github.com/golang/protobuf` or `: ptypes.`, and the only import guard at line 97 covers unaliased `ptypes/` sub-packages. Nothing prevents the next change from re-importing `github.com/golang/protobuf/proto` in any of the files just cleaned, and nothing records that the holdouts are a bounded list. Add an allow-listed `git grep` guard to `vet.sh` in the idiom the file already uses, finish `pretty.go` and `bootstrap.go` here, and either finish channelz or state in the PR that it is deferred. See `03_build_and_migration_hygiene.md`.

## The code-judo move

Findings 1, 2 and 6 want the same ten-line function: take an `any`, return it as an APIv2 message (adapting APIv1-only messages) or nil. Define it once in a leaf internal package that imports only `google.golang.org/protobuf/proto` and `protoadapt`, and use it from `encoding/proto`, `internal/binarylog` and `internal/pretty`. That deletes three hand-rolled type switches, one duplicated marshal block in binary logging, one duplicated branch in `pretty`, and the last non-API import of the old `proto` package. More importantly it gives "what counts as a protobuf message in grpc-go" one answer in one place, so the migration stops depending on every reader remembering that two interfaces share a name. Finding 3 is the mirror image on the output side and is fixed with the matching `protoadapt.MessageV1Of`.

## Proposed remediation sequence

1. Add the shared V2-normalising helper and use it in `encoding/proto/proto.go`; add a codec test with a V1-only message. This is the release blocker.
2. Use the helper in `internal/binarylog/method_logger.go`, collapsing the two payload blocks into one function; add a V1-only case to the method logger test.
3. Apply `protoadapt.MessageV1Of` in `Status.Details()`, document the round-trip guarantee, and add a V1-only detail to `TestStatus_ErrorDetails`; simplify the comparison there.
4. Collapse the two proto branches in `pretty.ToJSON` onto the helper and drop the old-module import.
5. Fix the `CheckValid` error binding in `loadreport.go` and give `rls.convertDuration` the same shape.
6. Restore `test/tools/go.mod` and `go.sum` from `main` and re-apply only the `protoc-gen-go` swap.
7. Relax the error assertion in `TestStatus_ErrorDetails_Fail` and remove the `protoimpl` import.
8. Migrate `bootstrap.go` off `jsonpb`, decide on channelz, and add the allow-listed import guard to `vet.sh`.

Steps 1 to 3 restore behaviour and should land together. Steps 4 to 8 are independent of each other.

## Open questions

Is dropping APIv1-only message support in the default codec and in binary logging intended? If it is, it is a breaking change for users of old generators and gogo/protobuf and needs to be stated in the PR and release notes rather than arriving as a side effect of an import rename; if it is not, findings 1 and 2 stand as written.

Why was the channelz migration reverted twice on this branch, and is there a tracked follow-up? The answer decides whether `channelz/service` belongs on a permanent allow-list or a temporary one in finding 8.

## Review method and verification status

One primary review context on `claude-opus-5-5` at high reasoning effort. No subagents were started, and no cross-model or alternate-model review was run. No prior reviews, upstream discussion, or reference findings were consulted.

Findings 1, 2 and 3 were confirmed by execution using `go test -overlay` with scratch files kept in the work directory, each run once at head and once with the single relevant file overlaid by its `main` version. Finding 4 was confirmed by reading the control flow. Findings 5 and 8 were confirmed from the diff, `git log` and `git grep`; the tools module could not be built because its dependencies are not in the provided module cache. Findings 6 and 7 were confirmed by reading the repository and the protobuf-go v1.32.0 sources in the module cache. The full test suite and `vet.sh` were not run. The clone was not modified.

Detail files:

- `01_v1_message_boundary.md` — findings 1, 2, 3 and 6, with commands, outputs and worked code.
- `02_duration_and_error_call_sites.md` — findings 4 and 7.
- `03_build_and_migration_hygiene.md` — findings 5 and 8, plus what was checked and found acceptable.
