# Scheduler import restrictions

## Scope and judgment

This subsystem review covers the deletion in `pkg/scheduler/.import-restrictions`, the actual scheduler import set, and the import-boss implementation used to enforce the remaining rules. There are no actionable findings in this subsystem.

The deleted rule previously allowed exactly `k8s.io/kubernetes/pkg/apis/core/validation`. It is no longer required after the matching source import removal. The remaining explicit allowances, scheduler-internal allowance, and Kubernetes catch-all restriction are unchanged. This tightens the architecture boundary without adding a rule type, special case, or alternate enforcement path.

## Evidence and enforcement behavior

At head, `.import-restrictions:24–26` forbids other imports matching `^k8s[.]io/kubernetes`. None of the remaining earlier selectors allows core validation. The similarly named core-v1 helper exception at lines 5–6 is anchored to a different exact package and cannot accidentally allow core validation. The scheduler-internal allowance at lines 20–22 also cannot match it.

`cmd/import-boss/main.go:330–409` computes direct and indirect imports, skips indirect imports for rules without `Transitive`, matches selectors in order, and stops once a rule allows or forbids the import. `Rule.Evaluate` at lines 253–273 matches forbidden prefixes before allowed prefixes. The scheduler rules do not set `Transitive`, so the removed exception and remaining catch-all govern direct imports. With the exception gone, a direct core-validation import reaches the existing catch-all and is forbidden. This is a deduction from the unchanged enforcement implementation and the committed rule set; no source mutation or synthetic forbidden import was introduced to test it.

`loadPkgs` at lines 82–113 uses `Tests: true`. The command therefore checks test package representations as well as production packages, which matters because a stale test import could otherwise make the restriction removal fail. A source search across `pkg/scheduler` found no remaining quoted import of `k8s.io/kubernetes/pkg/apis/core/validation`; mentions in comments are not imports. The focused `go list` output independently excludes it from the scheduler's production direct import set.

The restriction file shrinks from 28 to 26 lines. It retains its existing allow-specific-then-forbid-rest shape. `rg --files pkg/scheduler -g '.import-restrictions'` found only this scheduler restriction file. The actual import-boss command successfully checked the scheduler subtree and its test package variants, so the deletion does not leave the checked package set with an unsatisfied restriction.

This review does not claim that core validation disappears from the scheduler's entire transitive dependency closure. That would be a different architectural question from the direct import boundary enforced by these rules.

## Worked code-judo analysis

The simplest policy implementation is the existing deny-by-default mechanism with the obsolete exception removed:

```yaml
# Existing scheduler-internal allowance remains in place.
- selectorRegexp: ^k8s[.]io/kubernetes/pkg/scheduler($|/)
  allowedPrefixes: [ "" ]

# Existing catch-all also rejects the removed validation dependency.
- selectorRegexp: ^k8s[.]io/kubernetes
  forbiddenPrefixes: [ "" ]
```

This abbreviated excerpt illustrates why no explicit core-validation deny rule is needed; the actual file retains the other established allowances. Adding a new deny selector for this package would duplicate what the catch-all already does. Leaving the deleted allowance in place would permit accidental recoupling without improving current behavior. Expanding the rule to enforce all transitive imports would introduce a different policy affecting unchanged dependencies. None is a simpler, behavior-preserving remedy to the reviewed change.

The coordinated deletion of the source import and its policy exception is the code-judo move: one dependency and one exception disappear together. No policy wrapper, new configuration, or tooling change is warranted. These alternatives are analysis, not actionable findings.

## Commands and verification status

The exact diff and policy were inspected using `git diff main...review-head`, `cat pkg/scheduler/.import-restrictions`, numbered source reads, and reads of `cmd/import-boss/main.go`. `hack/verify-import-boss.sh` was inspected as source to identify the canonical checker; the full workspace script was not executed.

The actual focused commands were executed from the clone root under the supplied toolchain environment. As in the scheduler detail, `CACHE` below abbreviates the absolute supplied clone-cache directory for reproduction only.

```sh
timeout 300s env \
  GOROOT="$CACHE/toolchain" \
  GOMODCACHE="$CACHE/gomodcache" \
  GOCACHE="$CACHE/gocache" \
  GOFLAGS=-mod=vendor GOPROXY=off GOSUMDB=off GOTOOLCHAIN=local \
  "$CACHE/toolchain/bin/go" run ./cmd/import-boss -v 2 ./pkg/scheduler/...
```

The checker exited zero. Its output reported 147 loaded package representations, showed verification of scheduler production and test packages, and ended with `Completed successfully.` There were no package-loading errors in the observed output. This verifies the focused subtree; the full repository import-boss check was not run.

```sh
timeout 300s env \
  GOROOT="$CACHE/toolchain" \
  GOMODCACHE="$CACHE/gomodcache" \
  GOCACHE="$CACHE/gocache" \
  GOFLAGS=-mod=vendor GOPROXY=off GOSUMDB=off GOTOOLCHAIN=local \
  "$CACHE/toolchain/bin/go" list -f '{{join .Imports "\n"}}' ./pkg/scheduler
```

The listing exited zero and contained no direct core-validation import. `git diff --check main...review-head` also exited zero. The initial `git status --porcelain=v1` was empty; subsequent `git diff --exit-code HEAD` and `git status --porcelain=v1` were empty and successful. `git rev-parse HEAD main review-head` matched the packet revisions, and `git rev-parse 'HEAD^{tree}'` returned `f352e4a83a44eebca7557612450597686a4ddb4d`.

All checks were read-only with respect to the checkout, used the existing workspace and vendored modules, and required no network fetching. No additional remediation or question remains for this subsystem.
