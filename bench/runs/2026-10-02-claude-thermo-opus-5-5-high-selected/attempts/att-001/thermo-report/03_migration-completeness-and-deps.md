# 03 — Migration completeness, dependency manifests, and the missing ratchet

Scope: `go.mod`, `test/tools/go.mod`, `test/tools/go.sum`, `test/tools/tools.go`, `vet.sh`, and the packages the change did not reach.

## Finding A — `test/tools/go.mod` downgrades `golang.org/x/tools`

```
$ git diff main...review-head -- test/tools/go.mod
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

`test/tools/go.sum` moves with it: `golang.org/x/tools v0.17.0` → `v0.14.0`, `golang.org/x/sync v0.6.0` → `v0.4.0`, and new `golang.org/x/sys v0.13.0` lines.

Verification status: confirmed from the committed diff.

Replacing `github.com/golang/protobuf/protoc-gen-go` with `google.golang.org/protobuf/cmd/protoc-gen-go` in `test/tools/tools.go` is the intended change, and it matches `regenerate.sh:34-35`, which already installs the new path. Promoting `google.golang.org/protobuf` from indirect to direct follows from that. Nothing in the migration requires moving `x/tools` backwards by three minor versions or adding an `x/sys` requirement. The tip commit is titled "resolve conflicts and add changes"; this has the signature of a conflict resolved in favour of a stale side.

This module pins the versions of `goimports` and `staticcheck`'s dependencies that `vet.sh` runs. A silent downgrade here changes what CI lints with.

Remedy: restore `golang.org/x/tools v0.17.0`, drop the `x/sys` line, and regenerate `go.sum` with `go mod tidy` in `test/tools`. The only lines that should differ from `main` in this file are the `golang/protobuf` removal and the `google.golang.org/protobuf` promotion.

## Finding B — the migration stops short and nothing stops it sliding back

The change is described as migrating protobuf imports and call sites "across the repository". What remains at head:

```
$ grep -rn "github.com/golang/protobuf" --include='*.go' .
./internal/pretty/pretty.go:27:	protov1 "github.com/golang/protobuf/proto"
./channelz/service/service.go:26:	"github.com/golang/protobuf/ptypes"
./channelz/service/service.go:27:	wrpb "github.com/golang/protobuf/ptypes/wrappers"
./channelz/service/service_test.go:30:	"github.com/golang/protobuf/proto"
./channelz/service/service_test.go:31:	"github.com/golang/protobuf/ptypes"
./channelz/service/service_sktopt_test.go:34:	"github.com/golang/protobuf/ptypes"
./channelz/service/service_sktopt_test.go:40:	durpb "github.com/golang/protobuf/ptypes/duration"
./channelz/service/func_linux.go:24:	"github.com/golang/protobuf/ptypes"
./channelz/service/func_linux.go:25:	durpb "github.com/golang/protobuf/ptypes/duration"
./xds/internal/xdsclient/bootstrap/bootstrap.go:32:	"github.com/golang/protobuf/jsonpb"
./reflection/grpc_testing_not_regenerate/testv3.go:41:	proto "github.com/golang/protobuf/proto"
./credentials/credentials.go:31:	"github.com/golang/protobuf/proto"
```

and the root manifest still lists it as a direct requirement:

```
$ sed -n 11p go.mod
	github.com/golang/protobuf v1.5.3
```

`channelz/service/service.go` and `func_linux.go` together hold thirteen `ptypes.`/`wrpb.`/`durpb.` call sites of exactly the kinds the PR converted elsewhere (nine `ptypes.TimestampProto`, one `ptypes.DurationProto`, two `wrpb.Int64Value` literals, one `durpb.Duration`). `bootstrap.go:473` is a single `jsonpb.Unmarshaler{AllowUnknownFields: true}`, which is `protojson.UnmarshalOptions{DiscardUnknown: true}`. `reflection/grpc_testing_not_regenerate/testv3.go` is deliberately frozen legacy generated code and should stay. `credentials/credentials.go:290` exposes `Value proto.Message` on an exported struct, so it is a public-API question and reasonably left alone — but then it should be converted to `protoadapt.MessageV1` the way `WithDetails` was, since that is a type-identical alias and removes the import.

Verification status: confirmed by grep at head.

The consequence is a repository in which the same operation is spelled two ways depending on which directory you are in, with no rule telling a contributor which is correct. `vet.sh` was not touched. It still carries:

```
vet.sh:96  # - Ensure all ptypes proto packages are renamed when importing.
vet.sh:97  not git grep "\(import \|^\s*\)\"github.com/golang/protobuf/ptypes/" -- "*.go"
```

which polices the *style* of the old import rather than forbidding it, and the SA1019 allowlist at `vet.sh:151-157` still waves through `"github.com/golang/protobuf`, `: ptypes.` and `proto.RegisterType` deprecation warnings repository-wide. A new file importing `github.com/golang/protobuf/ptypes` with an alias passes CI today exactly as it did before this PR.

A 68-file mechanical migration is only worth its review cost if it ends in an enforced invariant. This one ends in a convention.

Remedy, in order of preference:

1. Finish it: convert `channelz/service`, `bootstrap.go`, and `pretty.go` (see `01_message-boundary.md`, Finding D), switch `credentials.go` to `protoadapt.MessageV1`, and let `go mod tidy` demote `github.com/golang/protobuf` in the root `go.mod`.
2. Replace the `vet.sh:97` rule with one that forbids the import outright, with an explicit path allowlist for whatever is intentionally left:

   ```sh
   # - Do not import the deprecated golang/protobuf module.
   git grep -l '"github.com/golang/protobuf/' -- "*.go" 2>&1 | not grep -v '\.pb\.go\|grpc_testing_not_regenerate'
   ```

   and delete the `"github.com/golang/protobuf` and `: ptypes.` lines from the SA1019 allowlist so that staticcheck does the enforcing.
3. If the remaining packages are being deferred to a follow-up, say that in the PR description and still land the `vet.sh` rule now with those paths allowlisted, so the allowlist is the to-do list.

## File-size check

No file crosses the 1,000-line boundary because of this PR. Line counts for every changed `.go` file at or above 1,000 lines are identical at base and head (for example `test/end2end_test.go` 6389 → 6389, `internal/transport/http2_server.go` 1446 → 1446). The diff is 165 insertions and 174 deletions across 68 files.

```
for f in $(git diff --name-only main...review-head -- '*.go'); do
  h=$(git show review-head:$f | wc -l); b=$(git show main:$f | wc -l)
  [ $h -ge 1000 ] || [ $b -ge 1000 ] && echo "$f base=$b head=$h"
done
```

No finding under this rule.
