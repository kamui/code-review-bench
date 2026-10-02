# Detail 03: module files, tooling, and what the migration left behind

Verification: `git diff main...review-head -- test/tools examples/go.mod`, plus `grep -rn "github.com/golang/protobuf" --include='*.go'` at HEAD, and reading `vet.sh`. No module download was possible (GOPROXY=off), so I did not try to re-tidy `test/tools`.

## D3.1 The migration is incomplete: legacy `golang/protobuf` imports remain in non-generated code

Remaining at HEAD (excluding generated `.pb.go`):

- `credentials/credentials.go:31` (public `AuthInfo`-adjacent type with `Value proto.Message`, line 290)
- `channelz/service/service.go:26-27`, `channelz/service/func_linux.go:24-25`, and the channelz service tests (`ptypes`, `ptypes/wrappers`, `ptypes/duration`)
- `xds/internal/xdsclient/bootstrap/bootstrap.go:32` (`jsonpb`, used at line 473 for `Unmarshaler{AllowUnknownFields: true}`)
- `internal/pretty/pretty.go:27` (the v1 branch, see D1.1)
- `reflection/grpc_testing_not_regenerate/testv3.go:41` (generated, exempt)

Because `jsonpb` and `ptypes` stay, the root `go.mod` must keep `github.com/golang/protobuf` as a direct requirement, so the stated goal (removing the old package) is not reached. The `vet.sh` line 97 check only bans unaliased `ptypes/<sub>` imports, so the aliased channelz imports (`wrpb`, `durpb`) and the bare `ptypes` import pass it; it only enforces renaming, not removal. Also, the `SA1019` suppression allowlist in `vet.sh` (around lines 148-158) still ignores `"github.com/golang/protobuf`, `: ptypes.` and `proto.RegisterType`, so the PR did not tighten the tooling that would have told the author what was left.

Remedy: either finish the migration in this PR (channelz becomes `durationpb`/`wrapperspb`/`AsDuration`; bootstrap's `jsonpb.Unmarshaler{AllowUnknownFields: true}` becomes `protojson.UnmarshalOptions{DiscardUnknown: true}`; `credentials.go` takes a v2 `proto.Message`), or state in the PR description that these are deliberately deferred, and add them to a tracking list. A half-migrated tree where both `ptypes.DurationProto` (channelz) and `durationpb.New` (everywhere else) coexist is a worse state than either end. Changing the public `credentials` field type is an API question and may reasonably be split out, but channelz and bootstrap are internal.

## D3.2 `test/tools/go.mod` and `go.sum` were downgraded as a side effect (`test/tools/go.mod:5-10, 16`)

The PR changes `golang.org/x/tools` from `v0.17.0` to `v0.14.0`, replaces `golang.org/x/sync v0.6.0` with `v0.4.0` in `go.sum`, and adds `golang.org/x/sys v0.13.0`. The PR purpose is an import migration. Nothing in `test/tools/tools.go` requires an older `x/tools`; the only source change there is swapping the `protoc-gen-go` blank import path. The versions look like the result of resolving the module graph without the base branch's newer pins (the commit history includes several "resolve conflicts" merges). The downgrade silently moves the `goimports` and `staticcheck` toolchain used by `vet.sh` to older releases and trims go.sum entries unrelated to the migration. The root `go.mod` still requires `x/sync v0.6.0` and `x/sys v0.16.0`, so the repo now pins two different versions of the same libraries across its modules.

Remedy: restore `golang.org/x/tools v0.17.0` (and the matching `x/sync`/`x/sys` indirects) in `test/tools`, keeping only the change that is actually needed: swap `github.com/golang/protobuf` for `google.golang.org/protobuf` as the direct requirement and re-run `go mod tidy` with the existing pins. That keeps the diff reviewable as a one-line module change.

## D3.3 `examples/go.mod` marks `golang/protobuf` as indirect

`examples/go.mod` moves `github.com/golang/protobuf v1.5.3` from direct to `// indirect`. That is correct only if no file in `examples/` imports it any more; the diff does update `examples/route_guide/server/server.go`, but I did not exhaustively grep the examples module. If another example still imports it, `go mod tidy` would flip it back. Low risk, worth a quick `grep` before merge.

## D3.4 Import grouping churn

Several files put `google.golang.org/protobuf/...` into the grpc import group and leave a stray blank-line group below it (`balancer/grpclb/grpclb.go:44-50`, `grpclb_remote_balancer.go`, `internal/testutils/xds/e2e/clientresources.go`), while neighbours keep protobuf imports in a trailing group. `encoding/proto/proto_benchmark_test.go` also gains an extra blank line. This is cosmetic, and `goimports` accepts it, but it adds diff noise. It is the lowest-priority item and not worth blocking.

## D3.5 File-size check

No file in the diff crosses 1000 lines because of this PR; all changes are net-neutral imports and one-for-one call replacements (68 files, +165/-174).
