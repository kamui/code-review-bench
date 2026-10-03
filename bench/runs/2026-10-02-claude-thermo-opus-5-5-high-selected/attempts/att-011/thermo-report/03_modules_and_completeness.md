# 03 — Module files and migration completeness (findings 5, 8)

## Finding 5 — `test/tools` dependency downgrade

`git diff main...review-head -- test/tools/go.mod`:

```
 require (
 	github.com/client9/misspell v0.3.4
-	github.com/golang/protobuf v1.5.3
-	golang.org/x/tools v0.17.0
+	golang.org/x/tools v0.14.0
+	google.golang.org/protobuf v1.32.0
 	honnef.co/go/tools v0.4.6
 )
 ...
-	google.golang.org/protobuf v1.32.0 // indirect
+	golang.org/x/sys v0.13.0 // indirect
```

`go.sum` moves `golang.org/x/sync` from v0.6.0 to v0.4.0, `golang.org/x/tools` from v0.17.0 to v0.14.0, and adds `golang.org/x/sys v0.13.0`.

History: `git log --format='%h %ad %s' --date=short review-head -- test/tools/go.mod`

```
b8374114 2024-01-26 resolve conflicts and add changes
5051eeae 2024-01-24 grpc: Update go mod (#6939)
40c279a8 2023-11-14 deps: update dependencies for all modules (#6795)
```

`5051eeae` is the base of this review and is the commit that raised `x/tools` to v0.17.0. The head commit undoes it. v0.14.0 is the version from the November dependency update, which is what the PR branch carried before the merge; the conflict was resolved in favour of the branch's stale lines.

The only change the migration needs in this module is in `test/tools/tools.go:31` (`google.golang.org/protobuf/cmd/protoc-gen-go` instead of `github.com/golang/protobuf/protoc-gen-go`), which correctly brings the pin in line with `regenerate.sh:34-35`, already installing the v2 generator from this module. That requires `google.golang.org/protobuf` to become a direct requirement and `github.com/golang/protobuf` to go. It does not require touching `x/tools`, `x/sync` or `x/sys`.

Why it matters: `x/tools` here pins `goimports` and is also the `x/tools` version that `staticcheck` is built against for `vet.sh`. Moving it backwards changes the tooling CI runs for everyone, in a PR whose description gives no reason for it.

Verification status: **confirmed from the diff and history; not executed.** `golang.org/x/tools` v0.14.0 and v0.17.0 are not present in the offline module cache and fetching is outside the allowance, so the module could not be built or tidied.

Remedy: `git checkout main -- test/tools/go.mod test/tools/go.sum`, keep the `tools.go` edit, run `go mod tidy` in `test/tools`, and confirm the resulting diff touches only the two protobuf lines.

## Finding 8 — the migration stops short and has no guard

Command: `grep -rn "github.com/golang/protobuf" --include='*.go' . | grep -v '\.pb\.go'` at head:

```
internal/pretty/pretty.go:27:                           protov1 "github.com/golang/protobuf/proto"
channelz/service/service.go:26:                         "github.com/golang/protobuf/ptypes"
channelz/service/service.go:27:                         wrpb "github.com/golang/protobuf/ptypes/wrappers"
channelz/service/service_test.go:30:                    "github.com/golang/protobuf/proto"
channelz/service/service_test.go:31:                    "github.com/golang/protobuf/ptypes"
channelz/service/service_sktopt_test.go:34:             "github.com/golang/protobuf/ptypes"
channelz/service/service_sktopt_test.go:40:             durpb "github.com/golang/protobuf/ptypes/duration"
channelz/service/func_linux.go:24:                      "github.com/golang/protobuf/ptypes"
channelz/service/func_linux.go:25:                      durpb "github.com/golang/protobuf/ptypes/duration"
xds/internal/xdsclient/bootstrap/bootstrap.go:32:       "github.com/golang/protobuf/jsonpb"
reflection/grpc_testing_not_regenerate/testv3.go:41:    proto "github.com/golang/protobuf/proto"
credentials/credentials.go:31:                          "github.com/golang/protobuf/proto"
```

`reflection/grpc_testing_not_regenerate/testv3.go` is deliberately frozen generated code and is correctly left alone. The rest are live:

- `channelz/service/service.go` and `func_linux.go`: 13 `ptypes.`/`wrpb.`/`durpb.` references (`ptypes.TimestampProto` nine times, `ptypes.DurationProto` once, `wrpb` wrapper types twice, `durpb.Duration` once). These are the same substitutions the PR already made in `internal/binarylog/method_logger.go` (`timestamppb`, `durationpb.New`) and `internal/testutils/xds/e2e/clientresources.go` (`wrapperspb`).
- `xds/internal/xdsclient/bootstrap/bootstrap.go:473`: `jsonpb.Unmarshaler{AllowUnknownFields: true}`, whose v2 form is `protojson.UnmarshalOptions{DiscardUnknown: true}`. The PR migrated this package's test (`bootstrap_test.go`) but not the code it tests.
- `credentials/credentials.go:290`: `OtherChannelzSecurityValue.Value proto.Message`. This is an exported field type on an experimental API, so changing it is an API decision; it needs to be made deliberately rather than left as an accident of where the sweep stopped.
- `internal/pretty/pretty.go:27`: removable today via the collapse in `01_message_boundary.md`.

Consequences:

- Root `go.mod:11` keeps `github.com/golang/protobuf v1.5.3` as a direct requirement, so the headline benefit of the migration (dropping the legacy module from the direct dependency set) is not delivered.
- `vet.sh:97` still reads `not git grep "\(import \|^\s*\)\"github.com/golang/protobuf/ptypes/" -- "*.go"`, a rule about renaming `ptypes` imports. It does not forbid the module, so a new `github.com/golang/protobuf/proto` import added next week passes CI. `vet.sh:153` continues to whitelist deprecation warnings for the whole module.

Verification status: **confirmed by grep at head.** `git diff --stat main...review-head -- vet.sh go.mod` is empty.

Remedy, in order of preference:

1. Finish the mechanical sites (channelz, bootstrap, pretty) in this change. They are small and identical in kind to what is already here.
2. For `credentials.go`, either migrate with a note on compatibility (the two `proto.Message` interfaces are both satisfied by generated messages, but the field type is part of the public surface) or record it as the single known exception.
3. Replace the `vet.sh:97` rule with one that fails on any `github.com/golang/protobuf` import outside an explicit allow-list (`grpc_testing_not_regenerate`, plus `credentials.go` if deferred), so the end state is enforced rather than hoped for.

## File-size and branching check

`git diff --stat main...review-head` reports 165 insertions and 174 deletions. No file crosses 1000 lines because of this change; the large files in the diff (`test/end2end_test.go`, the `unmarshal_*_test.go` files) were already past that mark and received import-only edits. The diff adds no new conditionals.
