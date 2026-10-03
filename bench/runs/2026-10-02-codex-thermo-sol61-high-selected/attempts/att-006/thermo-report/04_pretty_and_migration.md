# Pretty printing and repository-wide migration structure

Most changed files replace an import or an equivalent known-type constructor. The strongest pure maintainability opportunity is in `pretty.ToJSON`: both supported protobuf versions now use the same renderer, but the implementation still treats them as different rendering policies.

## JSON rendering evidence

`git diff main...review-head -- internal/pretty/pretty.go` removes `jsonpb`, replaces the v1 branch with `protojson`, and retains both protobuf cases. The file shrinks from 82 to 81 lines. `ToJSON` still occupies lines 37–69.

The v1 branch at lines 39–48 uses `protojson.MarshalOptions{Indent: jsonIndent}` and `protov1.MessageV2(ee)`. The v2 branch at lines 49–61 sets `Multiline: true` in addition to the same indentation and calls the same marshaller. Both return `string(ret)`, and both contain identical error fallback and Any-resolution commentary.

The pinned protobuf v1.32.0 source at `encoding/protojson/encode.go:50–53` specifies that a nonempty `Indent` treats `Multiline` as true. Thus the explicit multiline flag does not justify a second rendering implementation. The prior two implementations used different JSON libraries; that distinction disappears in this PR.

The type cases are not disjoint: normal modern generated messages also implement the v1 methods and are handled by the first case. A v2-only value can use the second case. Preserve both accepted interfaces, but stop using interface membership to select copies of the same renderer.

This is a static maintainability finding. `internal/pretty` has no package tests, though it compiled in the focused check and is exercised indirectly by several of the tested subsystems. No JSON behavior regression or byte-for-byte formatting stability is asserted by this review.

## Worked normalization proposal

Adapt input membership once, then use one protobuf rendering policy and one fallback:

```go
func ToJSON(e any) string {
    value := e
    if m, ok := e.(protoadapt.MessageV1); ok {
        value = protoadapt.MessageV2Of(m)
    }

    var data []byte
    var err error
    switch m := value.(type) {
    case protoadapt.MessageV2:
        data, err = (protojson.MarshalOptions{Indent: jsonIndent}).Marshal(m)
    default:
        data, err = json.MarshalIndent(e, "", jsonIndent)
    }
    if err != nil {
        return fmt.Sprintf("%+v", e)
    }
    return string(data)
}
```

The first operation normalizes v1 inputs; the second selects protobuf or ordinary JSON rendering. The original input remains available for the same error fallback. No persistent mode or classification flag is needed. The proposal retains v1-first adaptation and uses the same canonical converter as the status package. Check typed-nil forms when implementing this simplification; do not replace interface membership with a non-nil test or change the ordinary nil JSON behavior.

This eliminates one marshalling branch, one repeated error branch, and one duplicated explanatory comment. Keep the Any-resolution explanation once next to the shared fallback if it remains useful. The renderer remains a small direct function, with no generic callback registry, strategy object, custom options API, or identity wrapper. The proposal has not been applied; compare ordinary JSON, v1-only, v2-only, overlapping-interface, typed-nil, and failed Any rendering when implementing it.

## Other migration changes examined

The repository-wide committed diff contains 68 files and 165 insertions versus 174 deletions. The file measurements were produced by counting `git show main:<path>` and the checked-out head contents for every path returned by `git diff --name-only main...review-head`. [file-measurements.json](file-measurements.json) retains all rows.

No changed file crosses from fewer than 1,000 lines to at least 1,000 lines. Twelve changed files already exceed 1,000 lines, and every one keeps its line count. Their pre-existing size is not a new decomposition blocker for this migration. The largest changed file, `test/end2end_test.go`, remains 6,389 lines.

The internal xDS filter, cluster-specifier, and resource interfaces move consistently to v2 messages. Their concrete generated resources and `Any` wrappers implement the new interface. `Any.MessageIs` and `Any.UnmarshalTo` replace corresponding `ptypes` operations within the same owning packages. The old and new TypedStruct branches are existing protocol distinctions, not newly bolted-on exceptions. No actionable structural regression was established in those replacements.

The testutils marshaling helpers remove now-unnecessary v1-to-v2 adapters after their typed parameters become v2 messages. Those helpers still add test failure context or fixture construction behavior, so they are not identity-wrapper findings. The route-guide example continues to unmarshal into a generated v2-capable message. Known-type alias migrations to `durationpb`, `timestamppb`, `wrapperspb`, `structpb`, and `anypb` do not by themselves create new data models.

The binary-log timestamp change replaces ignored timestamp-construction error handling with `timestamppb.Now`, and timeout/load-report interval construction uses `durationpb.New`. Those constructor changes add no new branches. The behavioral findings concern the message interface boundary and checked duration consumption, not these constructors.

Some old imports remain in unchanged source, including credentials, channelz, and xDS bootstrap. They were not treated as newly introduced defects or as a mandate to expand this migration. The review reads source evidence only; it did not load ambient guidance files or infer instructions from them.

No new state model, async orchestration, or partial mutation flow was introduced. Status cloning and detail construction remain local and preserve the immutability structure. A request for parallel orchestration or broader decomposition would not be justified by this diff.

## Separate tools question

`test/tools/tools.go` correctly changes the generator import to `google.golang.org/protobuf/cmd/protoc-gen-go`. The nested manifest changes protobuf from the old direct dependency to the new direct dependency. Independently, `test/tools/go.mod:7` downgrades `golang.org/x/tools` from v0.17.0 to v0.14.0; its sum file replaces the tool and related module checksums accordingly.

The selected generator uses the same protobuf v1.32.0 version already present indirectly at base. No requirement for downgrading the independent `goimports` dependency is evident in the reviewed manifest changes. Ask for the requirement or retain the base tools version. This remains a question, not a finding of broken tooling: the nested tooling module was not built or installed, and no dependency fetch was allowed.

## Actionable remediation

Collapse protobuf rendering after input adaptation and remove the helper's old runtime import. Keep internal xDS parameter contracts direct rather than reintroducing v1 adaptation at every already-typed call site. Resolve the tools question separately. The report's demonstrated behavioral repairs should take place at external serialization boundaries, with the canonical runtime conversions doing the compatibility work.
