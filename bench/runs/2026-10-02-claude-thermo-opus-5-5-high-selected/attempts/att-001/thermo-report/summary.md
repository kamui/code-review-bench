# Thermo-nuclear code quality review — grpc/grpc-go#6919

Range reviewed: `5051eeae537cb2839dd499e1a63a141098a3a03a..b8374114d485b6957b15d8769d7d5d96ddeaafc6` (`git diff main...review-head`), 68 files, 165 insertions, 174 deletions.

Review configuration: one primary review context, `claude-opus-5-5` at `high`. No subagents, no cross-model or alternate-model review, no prior reviews or upstream discussion consulted.

## Verdict

**Not approvable as it stands.** Request changes.

Most of this diff is what it says it is: import paths swapped, `ptypes.UnmarshalAny(a, m)` turned into `a.UnmarshalTo(m)`, `ptypes.Is` turned into `MessageIs`. Those lines are fine and read better than what they replace.

The problem is that the PR treats the whole change as mechanical, and in a handful of places it is not. `github.com/golang/protobuf/proto.Message` and `google.golang.org/protobuf/proto.Message` are different interfaces, and the old `ptypes` helpers validated and adapted in ways their replacements do not. Wherever the old import was doing work at a boundary, swapping it changed behaviour without changing a visible line of logic. Three of those changes are user-facing regressions, confirmed by running the same test against head and against the base version of the file. A fourth is a plain bug introduced by hand-translating one call into two.

The structural criticism is the same in every case: the PR needed one small adapter and one small duration helper, wrote neither, and instead scattered ad-hoc translations that disagree with each other. It also stops short of the repository-wide end state it describes and adds nothing to keep the old imports from returning.

No file crosses the 1,000-line threshold because of this change.

## Findings

Findings are ordered by priority. Each one names its detail file, which carries the measurements, commands and worked proposal.

### F1. The default codec now rejects APIv1-only messages

`encoding/proto/proto.go:40-54`. The codec's `Marshal` and `Unmarshal` assert `v.(proto.Message)` on a caller-supplied `any`, and the PR changed which `proto.Message` that is by moving one import line. Messages that implement only the APIv1 interface — gogo/protobuf output, pre-1.4 golang/protobuf output — no longer pass the assertion. Running the same APIv1-only message through the codec gives `[10 1 120], err=<nil>` with the base file and `failed to marshal, message is *proto.legacyMsg, want proto.Message` at head; `Unmarshal` fails the same way. This is the default codec for every RPC, so an affected service goes from working to failing every call, with an error that tells the author their proto message is not a proto message. The fix is a single boundary adapter, `messageV2Of(v any) proto.Message`, that switches on `protoadapt.MessageV1` and `protoadapt.MessageV2` and returns nil otherwise; the codec calls it in place of the bare assertion. That proposal was applied to a scratch copy and restores base behaviour with the existing suite still passing. If dropping APIv1-only support is intended, make that decision explicitly in one place with an honest error and a release note. Detail: `01_message-boundary.md`, Finding A. Status: confirmed by execution, head and base.

### F2. `Status.Details()` returns a different concrete type for APIv1-only details

`internal/status/status.go:157-164`. `Details()` used `ptypes.UnmarshalAny` into a `DynamicAny`, which handed back the registered APIv1 type; it now appends the result of `any.UnmarshalNew()` directly. For a legacy registered type, a `WithDetails` → `Details` round trip returned `*status.legacyDetail` at base and returns `*impl.messageIfaceWrapper` at head, so both `d.(*legacyDetail)` and `d.(protov1.Message)` flip from true to false. `Details()` returns `[]any` so that callers can type-switch on it; this silently stops their cases matching. The write side, `WithDetails` on line 141, was given `protoadapt.MessageV2Of`; the read side was not given the inverse. The remedy is one call, `details = append(details, protoadapt.MessageV1Of(detail))`, which unwraps legacy messages and leaves dual-API messages untouched. Applied to a scratch copy it restores the base result and `./status/` still passes. Detail: `01_message-boundary.md`, Finding C. Status: confirmed by execution, head and base.

### F3. Binary logging silently writes empty payloads for APIv1-only messages

`internal/binarylog/method_logger.go:242-251` and `:282-291`. `ClientMessage.toProto` and `ServerMessage.toProto` type-assert `c.Message.(proto.Message)` and fall through to an info log when it fails. With the import swapped, an APIv1-only message takes the fall-through. The same input produces a payload of length 3 with the base file and length 0 at head. Unlike F1 this does not surface as an error: the log entry is written, it is simply empty. This is the same missing adapter as F1 and should be fixed by the same helper, shared from one internal package rather than copied. While in there, the two `toProto` bodies duplicate their payload-marshalling half and should share it. Detail: `01_message-boundary.md`, Finding B. Status: `ClientMessage` confirmed by execution, head and base; `ServerMessage` verified by reading.

### F4. The LRS interval check reports a stale, always-nil error

`xds/internal/xdsclient/transport/loadreport.go:173-177`. The new code is `if rInterval.CheckValid() != nil { return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err) }`. The `err` being formatted is the one from `stream.Recv()` at the top of the function, which is necessarily nil at that point; the result of `CheckValid()` is discarded. The returned error is always the literal text `invalid load_reporting_interval: <nil>`. At base, `interval, err := ptypes.Duration(...)` assigned the real cause; the diff removed the assignment and kept the format argument. `go build` and `go vet` both pass, so nothing catches it. Write it in the standard form — `interval := resp.GetLoadReportingInterval(); if err := interval.CheckValid(); err != nil { ... }` — so the error is declared on the line that uses it and the `rInterval`/`interval` name pair collapses to one. Detail: `02_duration-conversion.md`, Finding A. Status: verified by reading; not executed.

### F5. Duration conversion was hand-translated twice, inconsistently, and changed overflow semantics

`balancer/rls/config.go:306-311`. `convertDuration` is now `return d.AsDuration(), d.CheckValid()`. That returns a non-zero duration together with a non-nil error on the invalid path, which no Go reader expects from a `(value, error)` function. It also changes what is accepted: `CheckValid` allows protobuf's ±10,000-year range, `time.Duration` holds about ±292 years, and `AsDuration` saturates instead of failing. A 10,000,000,000-second duration was a config error at base and is `2562047h47m16.854775807s, err=<nil>` at head. That applies to `lookupServiceTimeout`, `maxAge` and `staleAge`, and to the LRS interval in F4, which is the second hand-rolled copy of this translation and already disagrees with the first about whether to keep the error. The practical impact of the clamp is small; the design problem is an unannounced validation change delivered through two divergent one-offs. Write one helper that checks first and converts second (`if err := d.CheckValid(); err != nil { return 0, err }; return d.AsDuration(), nil`), use it at both sites, and decide in that one place whether out-of-range stays an error. Detail: `02_duration-conversion.md`, Finding B. Status: confirmed by execution, head and base.

### F6. `test/tools/go.mod` downgrades `golang.org/x/tools` for no stated reason

`test/tools/go.mod:7-16`. Alongside the intended swap to `google.golang.org/protobuf/cmd/protoc-gen-go`, the manifest moves `golang.org/x/tools` from `v0.17.0` to `v0.14.0` and adds `golang.org/x/sys v0.13.0 // indirect`; `go.sum` follows, including `golang.org/x/sync` `v0.6.0` → `v0.4.0`. Nothing in a protobuf import migration requires moving the linter toolchain backwards three minor versions. The tip commit is "resolve conflicts and add changes", and this looks like a conflict resolved toward a stale side. This module pins what `vet.sh` runs, so the downgrade changes what CI lints with. Restore `x/tools v0.17.0`, drop the `x/sys` line, and re-run `go mod tidy` in `test/tools`; the only differences from `main` in this file should be the `golang/protobuf` removal and the `google.golang.org/protobuf` promotion. Detail: `03_migration-completeness-and-deps.md`, Finding A. Status: confirmed from the committed diff.

### F7. `pretty.ToJSON` keeps two switch arms that are now the same arm

`internal/pretty/pretty.go:27-60`. Before this PR the `protov1.Message` arm used `jsonpb` and the `protov2.Message` arm used `protojson`, which justified two arms. Now both call `protojson.MarshalOptions.Marshal` on an APIv2 message with the same indent, followed by the same fallback and the same three-line comment. The one textual difference, `Multiline: true`, is redundant: protojson documents that a non-empty `Indent` implies it. The file also still imports `protov1 "github.com/golang/protobuf/proto"` solely to name the first arm and call `protov1.MessageV2`. This is a refactor that moved the code without deleting the concept it had just made unnecessary. Collapse both arms into `if m := messageV2Of(e); m != nil { ... }` using the adapter from F1; that leaves one marshal call, one copy of the comment, no `protov1`/`protov2` aliases, and no golang/protobuf import in the file. Detail: `01_message-boundary.md`, Finding D. Status: verified by reading the vendored protojson source.

### F8. The migration stops short of "across the repository" and adds no guard against regression

`go.mod:11`. At head, `github.com/golang/protobuf` is still a direct requirement of the root module and is still imported by `channelz/service/service.go`, `channelz/service/func_linux.go`, both `channelz/service` test files, `xds/internal/xdsclient/bootstrap/bootstrap.go` (`jsonpb`), `credentials/credentials.go`, and `internal/pretty/pretty.go`. The `channelz/service` sites are thirteen calls of exactly the kinds converted elsewhere, and the bootstrap site is a single `jsonpb.Unmarshaler`. `vet.sh` was not touched: line 97 still polices the alias style of `ptypes` imports rather than forbidding them, and the SA1019 allowlist still excuses `"github.com/golang/protobuf` and `: ptypes.` deprecations everywhere. A new file importing `ptypes` passes CI today as it did before. A 68-file mechanical change earns its review cost by ending in an enforced invariant; this one ends in a convention that holds in some directories and not others. Either finish the remaining packages and let `go mod tidy` demote the dependency, or state the deferral and land a `vet.sh` rule now that forbids the import outside an explicit path allowlist, deleting the blanket SA1019 exemptions so the allowlist becomes the to-do list. Detail: `03_migration-completeness-and-deps.md`, Finding B. Status: confirmed by grep at head.

### F9. `status_test.go` reaches into `protoimpl` and casts on both sides to compare details

`status/status_test.go:359-412`. The expected error for an empty type URL is now built with `protoimpl.X.NewError("invalid empty type URL")` and compared by `Error()` text. `runtime/protoimpl` is for generated code, and it is needed here only because protobuf-go deliberately varies its error prefix to stop callers matching on it; the test imports the implementation package to reproduce bytes the library is trying to keep private. Assert what the test means — that slot is an `error` and the following detail still decodes — and drop the import. Separately, line 359 compares with `proto.Equal(details[i].(protoreflect.ProtoMessage), tc.details[i].(protoreflect.ProtoMessage))`, adding a `protoreflect` import to spell a type that `proto.Message` already aliases, and casting both operands because `WithDetails` takes APIv1 while `Details` now returns APIv2. That double cast is the test-side symptom of F2 and goes away with it. Detail: `04_tests.md`, Findings A and B. Status: verified by reading; `go test ./status/` passes at head.

## Questions for the author

### Q1. Is dropping APIv1-only message support intended?

F1, F2 and F3 all follow from APIv1-only messages no longer being adapted. If that is a deliberate deprecation, where is it recorded, and should the codec's error say so? If it is not deliberate, the adapter in F1 restores it.

### Q2. Was the `golang.org/x/tools` downgrade in `test/tools/go.mod` intentional?

If there is a reason `v0.14.0` is required, it should be in the commit message. Otherwise see F6.

## Proposed remediation sequence

1. Add the `messageV2Of` adapter in one small internal package. Use it in `encoding/proto` (F1), `internal/binarylog` (F3) and `internal/pretty` (F7). Add an APIv1-only message test to `encoding/proto` so the contract is pinned.
2. Add `protoadapt.MessageV1Of` on the read side of `Status.Details()` (F2). Add a legacy-type round-trip test. Simplify the comparison in `status_test.go` and replace the `protoimpl` expectation (F9).
3. Introduce one validated-duration helper and use it in `balancer/rls/config.go` and `loadreport.go`. This fixes the stale `err` (F4) and settles the overflow question in one place (F5).
4. Revert the unrelated version changes in `test/tools/go.mod` and `go.sum` (F6).
5. Convert `channelz/service` and `bootstrap.go`, or explicitly defer them, and in either case replace the `vet.sh` alias rule and SA1019 exemptions with a rule that forbids the import outside a named allowlist (F8).

Steps 1–4 are required before merge. Step 5 can be split into a follow-up only if the `vet.sh` ratchet lands with this PR.

## What was run

`go build ./...` and `go vet` on the touched core packages pass at head. The existing suites for `./status/`, `./encoding/proto/`, `./internal/binarylog/`, `./internal/testutils/`, `./balancer/rls/`, `./xds/internal/clusterspecifier/rls/` and `./xds/internal/xdsclient/xdsresource/` pass at head. Scratch tests were injected with `go test -overlay` from the clone root, comparing head against the `main` version of each production file; the clone was not modified. The full repository suite was outside the allowance and was not run.

## Detail files

- `01_message-boundary.md` — codec, binarylog, `Status.Details()`, `pretty.ToJSON`; measurements and the adapter proposal.
- `02_duration-conversion.md` — LRS stale error and `convertDuration`; measurements and the helper proposal.
- `03_migration-completeness-and-deps.md` — `test/tools` downgrade, remaining imports, `vet.sh` ratchet, file-size check.
- `04_tests.md` — `status_test.go` coupling and casts; duplicated marshal-any helpers.
