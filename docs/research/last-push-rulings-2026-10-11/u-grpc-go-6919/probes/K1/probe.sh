#!/usr/bin/env bash
# K1: does anything a contributor gets differ between the two golang.org/x/tools pins of grpc-go's lint-tools module?
#   probe.sh <scratch> <side>...
# <scratch> holds one git worktree of grpc-go per side (base = 5051eeae, head = b8374114, fix-tools = head with the
# pin put back to v0.17.0 and `go mod tidy` run, committed in the scratch clone), the Go 1.21.6 toolchain and the
# module cache. For each side it installs the lint tools the way the project's vet.sh does (`go install` inside
# test/tools, default flags), prints the x/tools version built into each binary, runs each tool on the head tree,
# and then runs the project's documented check in that side's tree: `./vet.sh -install` and `./vet.sh`, both with
# VET_SKIP_PROTO=1 as CONTRIBUTING.md and the CI vet job have it (no protoc is installed here).
set -uo pipefail
S=$1; shift
GOROOT_BIN="$S/gomodcache/golang.org/toolchain@v0.0.1-go1.21.6.linux-amd64/bin"
export PATH="$GOROOT_BIN:/usr/local/bin:/usr/bin:/bin" GOTOOLCHAIN=local GOMODCACHE="$S/gomodcache" GOCACHE="$S/gocache"
unset GOFLAGS GOBIN GOROOT
mkdir -p "$S/k1-out"
go version
for side in "$@"; do
  export HOME="$S/k1-home-$side" GOPATH="$S/k1-home-$side/go"
  mkdir -p "$HOME"
  tree="$S/$side"
  echo
  echo "## $side: $(git -C "$tree" log -1 --format='%h %s')"
  echo "pin: $(grep 'golang.org/x/tools' "$tree/test/tools/go.mod" | tr -s '\t ' ' ')"
  (cd "$tree/test/tools" && go install golang.org/x/tools/cmd/goimports honnef.co/go/tools/cmd/staticcheck \
    github.com/client9/misspell/cmd/misspell) 2>&1 | grep -v '^go: downloading' | sed 's/^/install: /'
  echo "install exit status: ${PIPESTATUS[0]}"
  for tool in goimports staticcheck misspell; do
    echo "$tool binary built with: $(go version -m "$GOPATH/bin/$tool" | awk '($1=="dep" || $1=="mod") && $2=="golang.org/x/tools"{print $2, $3}' | grep . || echo 'no golang.org/x/tools packages')"
  done
  echo "x/tools packages linked into goimports and staticcheck: $(cd "$tree/test/tools" && go list -deps golang.org/x/tools/cmd/goimports honnef.co/go/tools/cmd/staticcheck | grep -c '^golang.org/x/tools')"
  (cd "$S/head" && PATH="$GOPATH/bin:$PATH" goimports -l . > "$S/k1-out/goimports-$side.txt" 2>&1
   PATH="$GOPATH/bin:$PATH" staticcheck -go 1.19 -checks all ./... > "$S/k1-out/staticcheck-$side.txt" 2>&1
   PATH="$GOPATH/bin:$PATH" misspell -error . > "$S/k1-out/misspell-$side.txt" 2>&1)
  for tool in goimports staticcheck misspell; do
    echo "$tool from the $side tools, run on the head tree: $(wc -l < "$S/k1-out/$tool-$side.txt") lines, sha256 $(sha256sum < "$S/k1-out/$tool-$side.txt" | cut -c1-64)"
  done
  (cd "$tree" && VET_SKIP_PROTO=1 ./vet.sh -install > "$S/k1-out/vet-install-$side.log" 2>&1 && VET_SKIP_PROTO=1 ./vet.sh > "$S/k1-out/vet-$side.log" 2>&1)
  echo "the project's check in the $side tree (VET_SKIP_PROTO=1 ./vet.sh -install && VET_SKIP_PROTO=1 ./vet.sh): exit status $?, lines reading SUCCESS: $(grep -c '^SUCCESS$' "$S/k1-out/vet-$side.log" 2>/dev/null)"
done

echo
echo "## The two x/tools versions compared"
mkdir -p "$S/k1-synth"
cat > "$S/k1-synth/a.go" <<'GO'
package a

func f() {
	_ = rand.N(10)
	_ = slices.Contains([]int{1}, 1)
	_ = version.Lang("go1.21")
	fmt.Println(cmp.Compare(1, 2))
}
GO
printf 'package a\n\nimport (\n\t"fmt"\n\t"fmt"\n\t"os"\n\t"os"\n)\n\nfunc f() { fmt.Println() }\n' > "$S/k1-synth/b.go"
for side in base head; do
  echo "goimports from the $side tools on a file with missing imports: sha256 $(cd "$S/k1-synth" && "$S/k1-home-$side/go/bin/goimports" a.go 2>&1 | sha256sum | cut -c1-64)"
  echo "goimports from the $side tools on a file with duplicate and unused imports (the one goimports fix between the versions is 'remove duplicate unused imports'): sha256 $(cd "$S/k1-synth" && "$S/k1-home-$side/go/bin/goimports" b.go 2>&1 | sha256sum | cut -c1-64)"
done
changed=0 total=0
for pkg in $(cd "$S/head/test/tools" && go list -deps golang.org/x/tools/cmd/goimports honnef.co/go/tools/cmd/staticcheck | grep '^golang.org/x/tools'); do
  rel=${pkg#golang.org/x/tools}
  total=$((total + 1))
  d=$(diff -q -x '*_test.go' "$S/gomodcache/golang.org/x/tools@v0.14.0$rel" "$S/gomodcache/golang.org/x/tools@v0.17.0$rel" 2>/dev/null | grep -v '^Common subdirectories\|^Only in' | wc -l)
  if [ "$d" -gt 0 ]; then changed=$((changed + 1)); echo "  source differs between v0.14.0 and v0.17.0: $pkg ($d files)"; fi
done
echo "x/tools packages built into goimports or staticcheck: $total; with source that differs between the two versions: $changed"
