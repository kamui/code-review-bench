# Formatting and deletion of incidental complexity

F1 is an obvious simplification enabled by this migration. It is a structural
finding under the selected skill’s approval bar, rather than a demonstrated
behavioral regression. The relevant module is internal/pretty/pretty.go; the
base and head contain 82 and 81 lines respectively.

## Evidence

Before the change, ToJSON had two serializer families: jsonpb for legacy messages
and protojson for modern-only messages. The change removes jsonpb, then migrates
the first branch at lines 39–48 to protojson with legacy MessageV2 conversion.
The second branch at lines 49–61 still has a separate options object, marshal
call, error branch, identical explanatory comment, fallback, and string return.
There are now two protobuf marshal sites and three fallback sites in this small
function. The protobuf branch duplication is no longer required by different
serialization implementations.

The first options object sets Indent to two spaces. The second sets that same
Indent plus Multiline true. The pinned protojson MarshalOptions documents that
nonempty Indent treats Multiline as true; its encoder uses Indent directly.
These options therefore do not establish distinct output policies.

Most normal generated modern messages also satisfy the legacy interface, so
the earlier branch wins. A modern-only object uses the second branch. That fact
should affect normalization of input shape, not ownership of an entire duplicate
serialization pipeline. Leaving this structure makes future option or fallback
changes require edits in both cases and retains the old protobuf package solely
for compatibility adaptation that protoadapt already supplies.

## Worked code-judo proposal

Normalize the message shape in the existing module, then marshal and fall back
once. Keep ordinary objects on encoding/json and preserve fallback on the
original object. This needs no new service, public abstraction, or shared generic
serialization framework.

```go
func ToJSON(e any) string {
    var m protoadapt.MessageV2
    isProto := true
    switch v := e.(type) {
    case protoadapt.MessageV1:
        m = protoadapt.MessageV2Of(v)
    case protoadapt.MessageV2:
        m = v
    default:
        isProto = false
    }

    var b []byte
    var err error
    if isProto {
        b, err = (protojson.MarshalOptions{Indent: jsonIndent}).Marshal(m)
    } else {
        b, err = json.MarshalIndent(e, "", jsonIndent)
    }
    if err != nil {
        // Unknown Any types may not be registered; preserve readable fallback.
        return fmt.Sprintf("%+v", e)
    }
    return string(b)
}
```

The explicit isProto choice preserves dispatch semantics even when a supported
message adapts to a nil message interface; using `m != nil` as the discriminator
could accidentally switch such inputs to the generic JSON path. The proposal
reduces two protobuf marshal sites to one and three error/fallback blocks to one.
It replaces the two old/new proto imports with one protoadapt import. FormatJSON
continues to own raw JSON indentation and needs no change.

An alternative retaining a separate generic-object early return can still delete
the duplicated protobuf branch, but the shown proposal also removes repeated
fallback policy. No identity wrapper or new configuration option is needed.
This is deletion of obsolete implementation structure, not simply relocating it.

## Verification and limits

Source inspection used `git diff main...review-head -- internal/pretty`, `nl -ba
internal/pretty/pretty.go`, and a targeted read of the pinned protojson options
and marshal implementation. The two branches and their equivalent indentation
options were verified statically. The focused test command compiled the package;
it has no existing tests. The worked refactor was not applied or executed, and
this report does not claim empirical output equivalence for every message shape.

Before implementing the proposal, retain representative checks for legacy
messages, dual-interface generated messages, a modern-only message object,
unregistered Any fallback, ordinary maps, nil values, and generic JSON failure.
Assert JSON content and indentation policy without relying on exact whitespace
stability promised by neither protobuf serializer. Validate that fallback uses
the original input object. This verification supports behavior preservation
while removing the entire duplicate serialization branch.
