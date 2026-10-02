# Pretty-printing simplification and migration coverage

## F1: the JSON-engine distinction has disappeared

`internal/pretty/pretty.go` shrinks from 82 to 81 lines, but `ToJSON` still has two protobuf serialization paths. In the base, V1 messages used `jsonpb` while V2 messages used `protojson`; their bodies had different engine calls. The migration at lines 39–48 changes V1 to the V2 engine without deleting the now-redundant policy path at lines 49–61.

Both branches now construct `protojson.MarshalOptions`, marshal a V2 message, repeat the same three-line unresolved-Any explanation, format a failed input with the same `%+v`, and convert successful bytes to string. The first branch requires only a V1-to-V2 adapter. `Indent` being nonempty already enables multiline output: the pinned v1.32.0 `encoding/protojson/encode.go` documents that relationship. Keeping `Multiline: true` in the shared options is also harmless and makes the intended formatting explicit.

This is a specific opportunity arising from the changed engine, not a demand to refactor unrelated logger state. A shared serialization and error path makes fallback policy harder to drift, and `protoadapt` replaces the only remaining old protobuf dependency in this file. Ordinary modern generated messages satisfy both interfaces and typically enter the first branch, reinforcing that protobuf generation should be a normalization detail rather than a formatting mode.

## Worked code-judo proposal

Normalize V1 inputs before choosing the serializer, then give all inputs one success/error tail. Retain the original object for fallback formatting:

```go
func ToJSON(e any) string {
    normalized := e
    if m, ok := e.(protoadapt.MessageV1); ok {
        normalized = protoadapt.MessageV2Of(m)
    }

    var ret []byte
    var err error
    switch m := normalized.(type) {
    case protoadapt.MessageV2:
        ret, err = (protojson.MarshalOptions{
            Multiline: true,
            Indent: jsonIndent,
        }).Marshal(m)
    default:
        ret, err = json.MarshalIndent(e, "", jsonIndent)
    }
    if err != nil {
        // This includes Anys whose message type is not registered.
        return fmt.Sprintf("%+v", e)
    }
    return string(ret)
}
```

This reduces the function from 33 physical lines to 23 in the displayed form, removes one protobuf marshal policy and two repeated success/error tails, and requires no new pass-through helper or logger module. Imports become the V2 `protoadapt`/`protojson` packages alongside the existing standard-library JSON formatter. `FormatJSON` remains unchanged.

The adapter accepts V1-only generated messages and passes through dual-interface modern messages. A message implementing V2 alone still reaches the V2 serializer. Non-protobuf values still use `json.MarshalIndent`, and the error path still formats the original input. Avoid routing protobuf failures into ordinary JSON encoding: that would change the existing fallback behavior instead of simplifying it.

Verification status: structural equivalence was inspected against the pinned serializer/adapter implementations. `internal/pretty` compiled in the focused head command and has no package tests. The proposed replacement was not applied or run. Remediation should add small behavior checks for a retained V1-only generated message, a V2 message, an ordinary Go object, and a protobuf Any that cannot resolve its embedded type. Formatting comparisons should account for the serializer's documented output stability limits while checking the same formatting options and fallback value.

## Remaining migration coverage

All 68 changed files were inspected through `git diff main...review-head`, grouped by imports, changed call sites, contracts, tests, and module metadata. Most changes replace aliases to known protobuf types or use the V2 API on modern generated messages. Internal test Any helpers remove an obsolete input adaptation now that their declared contract is V2. This is appropriate for those internal typed helpers and should not be copied to the untyped public codec boundary.

The HTTP-filter and cluster-specifier contracts move to V2 within internal packages. The known builders and test implementations were migrated consistently. `Any.UnmarshalTo` and `MessageIs` keep parsing and type checks in protobuf's canonical implementation; creating bespoke decoder wrappers would add complexity without helping this migration. The changed resource parsing paths do not add configuration modes or unrelated conditional branches.

Binary logging now uses `timestamppb.Now` and `durationpb.New`, while the previous timestamp-construction error was already ignored. Go durations supplied to `durationpb.New` are representable by construction. No additional validation branch is required for that direction of conversion. Transport status marshaling and cloning use modern generated status messages.

The examples module moves its old protobuf dependency from direct to indirect. The tools module directly requires the V2 generator. `regenerate.sh` already installs that generator and the separate grpc generator; its generator contract is consistent with the dependency migration. The tools diff also downgrades `x/tools` from v0.17.0 to v0.14.0. No verified failure was established from that metadata change, and the review did not download dependencies or execute regeneration. The old demonstration `codegen.sh` is unchanged and still explicitly describes installation of the older generator; this is not presented as a new regression.

Remaining legacy imports in reflection-related source and dependency metadata are compatible with retaining V1 support. This review does not equate eliminating every import with a successful migration or recommend narrowing public APIs to achieve that cosmetic goal.

## Size and branching measurements

The commands were `git diff --stat main...review-head`, `git diff --numstat main...review-head`, and a Python read-only loop comparing newline counts in `git show main:<file>` with the corresponding head files. The diff has 165 inserted and 174 removed lines, for a net reduction of nine. No file crosses 1,000 lines.

The largest touched file is `test/end2end_test.go`, unchanged at 6,389 lines. Other existing files above the threshold include transport implementation, interop utilities, and numerous tests. They do not grow due to this PR. The touched implementation measurements are:

| File | Base lines | Head lines |
| --- | ---: | ---: |
| `encoding/proto/proto.go` | 58 | 58 |
| `internal/status/status.go` | 204 | 205 |
| `internal/pretty/pretty.go` | 82 | 81 |
| `balancer/rls/config.go` | 312 | 311 |
| `xds/internal/xdsclient/transport/loadreport.go` | 257 | 258 |

No new feature-specific flag, parallel-work dependency, or non-atomic update was introduced. The status append behavior remains based on a cloned status. The identified structural feedback is the redundant serializer policy, and the boundary remedies are deliberately local or shared only where their strict contract is identical.

## Verification status

Focused head tests passed for fault filtering, the RLS cluster specifier, and resource parsing. RBAC/router filter packages compiled with no tests. Targeted handler-transport error-details and status-equality checks passed. Binary logger log/truncation checks passed after the first filter matched no binary-log tests; both command outcomes are recorded explicitly in the verification ledger.

`git diff main...review-head --check` passed. Initial checkout status was clean. The review uses a tracked-content SHA-256 plus `git status` and the head tree identity to verify that scratch overlays and test execution leave the checkout unchanged. The ledger carries the exact identities and final comparison.
