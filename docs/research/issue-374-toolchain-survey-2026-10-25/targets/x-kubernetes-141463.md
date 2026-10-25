# x-kubernetes-141463 (kubernetes PR 141463, scheduler Event note length constant)

Desk survey, nothing run. Labels: read / reported / inferred as in the brief.

## 1. Toolchain and versions at review-head

| Fact | Value | Label |
|---|---|---|
| Go | `go 1.27.0` and `godebug default=go1.27` in go.work; host mise Go is 1.26.5, too old, hence the private toolchain | read: go.work; reported: target.json |
| Private toolchain | `go1.27.0.linux-amd64.tar.gz` (sha256 675c26c4...) extracted to `<cache>/toolchain`; has `bin/go`, `bin/gofmt`, `pkg/tool/linux_amd64/{compile,link,asm,cgo,vet,cover,fix,preprofile}` | reported: toolchain-provenance.json; read: archive listing |
| Workspace | `go.work` with 35 `use` entries: `.` plus `./staging/src/k8s.io/*` modules | read: go.work |
| Dependencies | `vendor/` at repo root, `vendor/modules.txt` begins with `## workspace` (workspace vendoring), `GOFLAGS=-mod=vendor` | read: vendor/modules.txt; reported: target.json |
| Env | `GOROOT=<cache>/toolchain`, `PATH=<cache>/toolchain/bin:...`, `GOMODCACHE=<cache>/gomodcache`, `GOCACHE=<cache>/gocache`, `GOFLAGS=-mod=vendor`, `GOPROXY=off`, `GOSUMDB=off`, `GOTOOLCHAIN=local` | reported |
| Cache built by | curl of the Go tarball, sha256 check, extraction, `go version`, `go test -run '^$' ./pkg/scheduler` (29.7 s) | reported: caches/x-kubernetes-141463.json |
| Cache contents | toolchain (src, test, bin, pkg) and gocache for `pkg/scheduler` and its imports. gomodcache is essentially empty (everything is vendored). No golangci-lint, no staticcheck. | read: archive listing |

### import-boss at review-head (read from the pinned mirror)

- Source is in-tree: `cmd/import-boss/main.go` (with README, `main_test.go`, `testdata/`). It is no longer a vendored `k8s.io/code-generator` tool.
- Imports: `github.com/spf13/pflag`, `golang.org/x/tools/go/packages`, `k8s.io/klog/v2`, `sigs.k8s.io/yaml`. All are in `vendor/modules.txt` (`golang.org/x/tools v0.48.0` with `go/packages` and `internal/gocommand`; pflag, klog/v2, sigs.k8s.io/yaml also listed) and the vendor tree has `golang.org/x/tools/go/packages` and `internal/gocommand` etc. So `go run ./cmd/import-boss` builds fully offline under `-mod=vendor` (read: ls-tree, modules.txt).
- Script: `hack/verify-import-boss.sh` does `kube::golang::setup_env`, `kube::util::require-jq`, then builds the pattern list from `go work edit -json | jq -r '.Use[].DiskPath + "/..."'` (35 patterns, e.g. `./...`, `./staging/src/k8s.io/api/...`) and runs `GOPROXY=off go run ./cmd/import-boss -v "${KUBE_VERBOSE:-0}" "${packages[@]}"`. Needs bash, jq, and `hack/lib/init.sh` (read).
- Arguments: `-v <klog verbosity>` plus package patterns. Rules come from `.import-restrictions` files (71 in the tree) found in the package directory and every parent up to the directory holding a `go.mod`. They are not passed as arguments. The tool is read-only: it parses files, prints `ERROR: ...` and exits 1 on a violation, 0 otherwise (read: main.go).
- How it loads: `packages.Load` with `NeedName|NeedFiles|NeedImports|NeedDeps|NeedModule` and `Tests: true`. x/tools shells out to `go list` (using the process environment, so `PATH` must contain the toolchain's `go`, and GOFLAGS/GOROOT apply). No type-checking, no compile (inferred from the mode bits and x/tools behaviour).
- The PR's relevant edit: `pkg/scheduler/.import-restrictions` loses `^k8s[.]io/kubernetes/pkg/apis/core/validation$ allowedPrefixes ['']`, the only change in that file; the other file touched is `schedule_one.go`. The only `.import-restrictions` under `pkg/scheduler` is that one (read: git diff, ls-tree). The rule set is non-transitive for the removed entry, so a violation would be a direct import of `k8s.io/kubernetes/pkg/apis/core/validation` from any package (including test files) under `pkg/scheduler/...`.

## 2. Commands the graded claims needed

6 rows in the refusal file (2 first-grade, 4 second-grade). Three of them wanted import-boss.

| Check | Argv a grader would run from the clone root | Wanted by | Example claim |
|---|---|---|---|
| import-boss over the changed restriction tree | `<cache>/toolchain/bin/go run ./cmd/import-boss -v 0 ./pkg/scheduler/...` (the CI form is the same command with 35 workspace patterns) | 2 first-grade (+1 second-grade that argued around it) | `blind-ac742c/3/c3` (unproven), `blind-c6ddf9/3/c4` (unproven); first-grade evidence: "go run ./cmd/import-boss was refused by the allowance, so the full verification was not run". Second-grade `blind-e69f2f/3/c3` (same wording, refuted by reading `test/integration/scheduler_perf` is outside the restriction's ancestry) |
| (not tool) API compatibility policy for truncation limits (needs a policy document, "fetching external policy is outside the allowance") | none | 1 | `blind-114e99/3/c3` (unresolved) |
| (not tool) UTF-8 cut in `truncateMessage`; constant divergence | none | 2 | `blind-dd388c/1/c1`, `blind-44ddf6/1/c2` |

No grader asked for `go vet`, `go build`, `gofmt`, `golangci-lint` or `staticcheck` on this target. The existing `go list` allowance (flags only `-json`, `-tags`, ...) cannot answer "who imports package X" because `-deps`, `-test` and `-f` are not allowed; those three flags would give a cheaper partial substitute for import-boss (inferred).

## 3. Can it run offline in the sandbox now?

| Command | Result | Label |
|---|---|---|
| `go run ./cmd/import-boss -v 0 ./pkg/scheduler/...` | Compiles offline: import-boss and its four imports are vendored, std and `pkg/scheduler` objects are in GOCACHE, only `x/tools/go/packages`, klog, pflag, yaml and a few siblings need compiling (seconds, inferred). It then runs `go list -e -json -test -deps ...` inside, with the same env; the smoke `go test ./pkg/scheduler` proves that vendored workspace loading of `pkg/scheduler` works in this cache. **Blocked today only by the profile**: `go run` is not an allowed verb. | inferred; smoke reported |
| Whole workspace as in `hack/verify-import-boss.sh` | Technically offline too, but loading 35 modules with `-test -deps` and Warshall transitive closure is memory-heavy and probably longer than five minutes (CI-scale job). Not recommended. | inferred |
| `hack/verify-import-boss.sh` itself | Needs bash, jq (`/usr/bin/jq` and `/usr/bin/bash` exist on the host), `hack/lib/init.sh` that sets GOPATH-style env, and `go work edit -json`; sources a lot of shell and may probe `git`/`kube::golang::setup_env` for a Go version check and install tools. Not worth enabling; use `go run` directly. | read (script) + inferred |
| `go test|list|version ./pkg` on one package (existing profile) | Works (smoke at both revisions: `go test -count=1 -run '^TestSchedulerScheduleOne$' ./pkg/scheduler`). | reported |
| `go vet ./pkg/scheduler`, `gofmt -l pkg/scheduler` | `vet` and `gofmt` binaries are in the private toolchain, so they run offline. Not requested. | read (archive listing) |
| Network at run time | None. `go run` does not fetch with `GOFLAGS=-mod=vendor GOPROXY=off GOTOOLCHAIN=local`. | inferred |

### Provisioning options for import-boss
1. Allow `go run ./cmd/import-boss` with argv fixed to: package `./cmd/import-boss`, optional `-v <0-9>`, then one or more patterns that match `^\./(pkg|cmd|plugin|test|staging/src/k8s\.io/[a-z-]+)(/[A-Za-z0-9._/-]*)?/\.\.\.$` or a single directory. Set `TMPDIR` to `tmp/` (go run writes the built binary there; needs an exec-capable tmp, inferred). No extra mount: the private toolchain is under `<cache>/toolchain`.
2. Prebuild: `go build -o <cache>/bin/import-boss ./cmd/import-boss` once online/at cache build, and allow only `<cache>/bin/import-boss` with the same restricted args. Avoids allowing `go run` of arbitrary packages. The binary still needs `go` on PATH at run time (it execs `go list`).

## 4. Service-only checks

None.

## 5. Risks for a focused profile

- `go run` of any package executes arbitrary code in the sandbox; if enabled, pin the package path to `./cmd/import-boss` exactly (or use the prebuilt binary).
- Scoped runs miss `InverseRules` violations whose importing packages are outside the loaded patterns (only apiextensions-apiserver restriction files and test data contain `inverseRules`, read via git grep). For this PR's claim the scope `./pkg/scheduler/...` is complete because the removed rule is a direct-import rule on that tree.
- `test/integration/scheduler_perf` and other trees are outside the `pkg/scheduler/.import-restrictions` ancestry; checking them needs their own pattern.
- Five-minute limit: whole-workspace import-boss, `go test ./pkg/scheduler/...` full (large; integration-style tests) and any `test/integration` tests (need etcd/apiserver binaries, absent) are out. Use `-run` on one package. The first cold compile of an uncached package tree (vendored client-go etc.) can itself run for minutes; only `pkg/scheduler` and its imports are warm.
- `GOFLAGS=-mod=vendor` makes `go` refuse when `vendor/modules.txt` and `go.work` disagree; the grader must not modify either (the clone is read-only anyway).
