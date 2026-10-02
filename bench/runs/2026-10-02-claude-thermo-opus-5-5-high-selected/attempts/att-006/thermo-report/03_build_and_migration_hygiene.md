# 03 — Build files and migration hygiene

Scope: `test/tools/go.mod`, `test/tools/go.sum`, `vet.sh`, and the files the migration
did not reach.

Review range: `5051eeae..b8374114` (`git diff main...review-head`).

## Finding 5 — `test/tools/go.mod` downgrades `golang.org/x/tools`

`git show main:test/tools/go.mod` (base, last touched by `5051eeae grpc: Update go mod (#6939)`):

```
require (
	github.com/client9/misspell v0.3.4
	github.com/golang/protobuf v1.5.3
	golang.org/x/tools v0.17.0
	honnef.co/go/tools v0.4.6
)
```

Head, `test/tools/go.mod:5-10`:

```
require (
	github.com/client9/misspell v0.3.4
	golang.org/x/tools v0.14.0
	google.golang.org/protobuf v1.32.0
	honnef.co/go/tools v0.4.6
)
```

and in the indirect block `golang.org/x/sys v0.13.0` appears. `test/tools/go.sum`
correspondingly drops the `golang.org/x/tools v0.17.0` and `golang.org/x/sync v0.6.0`
hashes and adds `x/tools v0.14.0`, `x/sync v0.4.0`, `x/sys v0.13.0`.

The intended change in this module is one line in `tools.go` (pin
`google.golang.org/protobuf/cmd/protoc-gen-go` instead of the old
`github.com/golang/protobuf/protoc-gen-go`) and the matching `require` swap. The
`x/tools` change is unrelated to protobuf and reverses the bump that the base commit made
one commit earlier. The head commit is titled "resolve conflicts and add changes", which
is consistent with the branch's older `go.mod` winning a merge conflict.

Effect: `goimports`, which `vet.sh` installs from this module, moves back three minor
releases, and the next dependency-update PR has to redo the bump.

### Verification

Confirmed from the diff and `git log main -- test/tools/go.mod`. Not confirmed by a
build: the module cache provided for this review does not contain `golang.org/x/tools`
at either version, and fetching is outside the execution allowance, so `go build` in
`test/tools` could not be run for head or base.

### Remedy

Take `main`'s `test/tools/go.mod` and `go.sum`, change only `tools.go`, and run
`go mod tidy` in `test/tools`. The resulting diff should be: `github.com/golang/protobuf`
removed from the direct requires, `google.golang.org/protobuf v1.32.0` promoted from
indirect to direct, `golang.org/x/tools` untouched at v0.17.0.

## Finding 8 — the migration has no ratchet and leaves undocumented holdouts

After this PR, `git grep '"github.com/golang/protobuf' review-head -- '*.go' ':!*.pb.go'`
still returns:

```
channelz/service/func_linux.go:24          ptypes
channelz/service/func_linux.go:25          ptypes/duration
channelz/service/service.go:26             ptypes
channelz/service/service.go:27             ptypes/wrappers
channelz/service/service_sktopt_test.go:34 ptypes
channelz/service/service_sktopt_test.go:40 ptypes/duration
channelz/service/service_test.go:30        proto
channelz/service/service_test.go:31        ptypes
credentials/credentials.go:31              proto
internal/pretty/pretty.go:27               proto
reflection/grpc_testing_not_regenerate/testv3.go:41  proto
xds/internal/xdsclient/bootstrap/bootstrap.go:32     jsonpb
```

Sorting these:

- `credentials/credentials.go:290` (`OtherChannelzSecurityValue.Value proto.Message`) is
  exported API typed on the old interface, and `testv3.go` is a deliberately frozen
  generated file. Both are legitimate holdouts.
- `internal/pretty/pretty.go` is touched by this PR and half-migrated (see finding 6 in
  `01_v1_message_boundary.md`).
- `xds/internal/xdsclient/bootstrap/bootstrap.go:473` still builds a
  `jsonpb.Unmarshaler{AllowUnknownFields: true}`; the direct equivalent is
  `protojson.UnmarshalOptions{DiscardUnknown: true}`.
- `channelz/service` was migrated and then reverted twice on the branch (commits
  `58ad782d` and `f58f647d`, both "revert changes in channelz") with no note in the tree
  saying why or who owns the follow-up.

`go.mod:11` therefore still lists `github.com/golang/protobuf v1.5.3` as a direct
requirement, and `vet.sh` was not touched. Its deprecation filter at `vet.sh:152-157`
still blanket-ignores every SA1019 hit whose text contains `"github.com/golang/protobuf`
or `: ptypes.`:

```
XXXXX Protobuf related deprecation errors:
"github.com/golang/protobuf
.pb.go:
grpc_testing_not_regenerate
: ptypes.
proto.RegisterType
```

and the only import guard (`vet.sh:97`) matches unaliased `ptypes/` sub-package imports,
not `proto`, `ptypes` or `jsonpb` themselves. The consequence is that nothing stops the
next change from importing `github.com/golang/protobuf/proto` again in any of the ~65
files this PR just cleaned, and nothing records that the four remaining locations are a
known, bounded list. A migration of this size without a guard decays.

### Verification

Confirmed by `git grep` on `review-head` and by reading `vet.sh`. `vet.sh` itself was not
run (it requires tool installation and network access).

### Remedy

Add an allow-listed guard next to the existing one in `vet.sh`, in the same idiom the
file already uses:

```sh
# - Do not import the deprecated golang/protobuf module outside the remaining
#   known users (public API in credentials, channelz service, frozen testv3).
git grep -l '"github.com/golang/protobuf' -- "*.go" 2>&1 | not grep -v \
  '\.pb\.go\|grpc_testing_not_regenerate\|^channelz/service/\|^credentials/credentials.go'
```

Finish `pretty.go` and `bootstrap.go` in this PR so they do not need to be on the list,
and drop the `: ptypes.` line from the SA1019 filter once channelz is done. If channelz
is intentionally deferred, say so in the PR description so the list above is a
documented boundary rather than an accident of two reverts.

## Things checked and found acceptable

- No file crosses the 1000-line threshold because of this PR (165 insertions, 174
  deletions across 68 files; the large test files touched were already far above it).
- `any.UnmarshalTo(msg)` and `config.MessageIs(m)` are nil-receiver-safe in protobuf-go
  v1.32.0, matching `ptypes.UnmarshalAny` / `ptypes.Is` on nil inputs, so the xDS filter
  and listener parsing changes (`unmarshal_lds.go`, `filter_chain.go`, `fault.go`,
  `rbac.go`, `router.go`, `clusterspecifier/rls/rls.go`) are behaviour-preserving for
  the generated xDS types they handle.
- `Status.WithDetails(...protoadapt.MessageV1)` is source-compatible:
  `protoadapt.MessageV1` and the old `golang/protobuf/proto.Message` are both aliases of
  `protoiface.MessageV1`.
- `internal/testutils.MarshalAny`, `e2e.marshalAny` and `status_test.mustMarshalAny` are
  now three near-identical must-wrappers around `anypb.New`. This predates the PR and is
  test-only; noted, not raised as a finding.
