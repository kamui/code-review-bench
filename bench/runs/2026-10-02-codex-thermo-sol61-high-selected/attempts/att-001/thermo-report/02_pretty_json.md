# Pretty JSON: remove the obsolete serializer split

This subsystem carries F1, the structural simplification finding. The change removes `jsonpb`, which is useful, but leaves its previous branch structure intact after both protobuf paths have converged on `protojson`.

## Measurements and evidence

`internal/pretty/pretty.go` shrinks from 82 to 81 lines, so this is not a file-size finding. `ToJSON` occupies lines 37–69. Its V1 branch spans lines 39–48 and its V2 branch spans lines 49–61. Each constructs marshal options, marshals a protobuf message, returns the same formatting fallback on error, and converts bytes to a string on success. The same error-fallback comment is repeated verbatim.

In the base, the V1 branch used `jsonpb.Marshaler.MarshalToString` while V2 used `protojson`. Separate serializers then existed. In the head, both branches use `protojson.MarshalOptions.Marshal`. Only adaptation remains generation specific. Keeping generation-specific serialization branches now obscures the invariant that there is exactly one protobuf JSON serializer.

The V1 branch sets `Indent: jsonIndent`; the V2 branch also sets `Multiline: true`. The pinned `google.golang.org/protobuf@v1.32.0/encoding/protojson/encode.go:50–54` documents that nonempty indentation already treats multiline as true. The encoder passes the same indentation to the same internal JSON encoder. Those option differences do not justify separate serialization and error paths.

Most current generated messages implement both interfaces, so they match the first branch. A V2-only implementation can still legitimately match the second branch. Deleting that type support would be a regression; normalize the type boundary and delete the duplicate work instead.

## Verification

`TestReviewEquivalentProtoFormatting` passes on the pinned head. It compares the two option sets on a generated `wrapperspb.StringValue`, and on a wrapper exposing only the `proto.Message` interface through embedding. The test confirms the latter does not implement `protoadapt.MessageV1`. Both option sets produce identical output for both cases, and `ToJSON` returns that output.

The equivalence of the options is also established by the pinned encoder implementation; the two simple runtime cases are a confirmation, not an exhaustive proof of all JSON schemas. The existing package contains no tests. This review does not claim that switching from `jsonpb` to `protojson` preserves every obscure custom legacy JSON-marshaler extension; no such repository call-site dependency was identified.

The relevant commands were the combined head overlay command in [01_protobuf_boundaries.md](01_protobuf_boundaries.md), `git diff main...review-head -- internal/pretty/pretty.go`, and inspection of the pinned `protojson/encode.go`. The probe output is retained in `../review-probes/head-probes.log`.

## Worked code-judo proposal

Normalize the protobuf generation at entry and keep the original value for the documented error fallback. Then select between protobuf JSON and ordinary JSON once:

```go
func ToJSON(e any) string {
    value := e
    if m, ok := e.(protoadapt.MessageV1); ok {
        value = protoadapt.MessageV2Of(m)
    }
    var (
        data []byte
        err  error
    )
    if m, ok := value.(protoadapt.MessageV2); ok {
        data, err = (protojson.MarshalOptions{Indent: jsonIndent}).Marshal(m)
    } else {
        data, err = json.MarshalIndent(value, "", jsonIndent)
    }
    if err != nil {
        return fmt.Sprintf("%+v", e)
    }
    return string(data)
}
```

This illustrative implementation has 20 lines versus the existing function's 33. It removes one whole serializer branch, the duplicated protobuf-error explanation, and two extra success/error handling paths. It replaces the deprecated `protov1.MessageV2` helper with `protoadapt.MessageV2Of`, already used elsewhere in the change. The adaptation is still explicit; there is no configurable serialization mechanism or new pass-through wrapper.

Ordinary generated messages continue to pass through the canonical adapter, and V2-only messages are already in the desired shape. Preserving `e` separately ensures a protobuf marshal failure formats the application object rather than its reflection wrapper. Protobuf nil values and plain nil should retain their existing formatting behavior; add focused cases for those, a normal struct, and an unresolved `Any` fallback when implementing the restructuring. The proposed function has not been applied or run as a production replacement.

## Remediation bar

There is a clear local simplification with no architectural tradeoff: use one protobuf serializer and one fallback. Under the selected skill, leaving both parallel paths is an actionable missed opportunity rather than a naming nit. The source already has a canonical conversion API, so continuing to depend on the deprecated module for this one conversion is unnecessary here. This does not imply that the root dependency can be removed wholesale; other unchanged files still import it.
