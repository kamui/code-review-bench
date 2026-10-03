# Production migration review

## Scope and measurements

Reviewed the production changes in `internal/status/status.go`, `internal/pretty/pretty.go`, `balancer/rls/config.go`, `internal/binarylog/method_logger.go`, and the xDS resource parsing files, together with the remaining production call-site substitutions in the diff. The review range is `main...review-head` (`5051eeae537cb2839dd499e1a63a141098a3a03a..b8374114d485b6957b15d8769d7d5d96ddeaafc6`). The full change touches 68 files (165 insertions, 174 deletions); no changed production file grows beyond a small handful of lines.

## Evidence and design assessment

The changes have the shape of a mechanical migration rather than a new abstraction layer. `ptypes.Is` and `ptypes.UnmarshalAny` become `Any.MessageIs` and `Any.UnmarshalTo` in `xds/internal/xdsclient/xdsresource/unmarshal_lds.go`; the exact message-type checks and existing error paths remain local to the parsing logic. The same direct substitution appears in `filter_chain.go`. Duration and timestamp conversions in `balancer/rls/config.go` and `internal/binarylog/method_logger.go` use the corresponding `durationpb` and `timestamppb` APIs without adding branch modes or wrappers.

`internal/status/status.go` keeps `WithDetails` accepting `protoadapt.MessageV1`, preserving acceptance of legacy message implementations while allowing conversion at the boundary through `protoadapt.MessageV2Of`. `Details` now uses `Any.UnmarshalNew` and returns the resolved v2 message. The updated tests compare via reflection rather than relying on a specific legacy concrete implementation. This is a deliberate API migration boundary, not incidental optionality or an opaque cast.

`internal/pretty/pretty.go` retains separate v1 and v2 cases. Although the branches look superficially duplicative, they cover distinct accepted message interfaces: the v1 branch needs the adapter, while a v2-only reflective message can use the second branch. Collapsing them would either narrow the helper's accepted inputs or move interface detection and adaptation into a less obvious helper. The PR's direct handling is the simpler maintainable shape for these contracts.

## Code-judo proposals considered

A single generic helper for all replacements would hide meaningful differences between converting a duration, marshaling `Any`, and matching an `Any` type. It would add a new layer without deleting concepts, so keeping the standard v2 calls at their existing ownership points is preferable.

The status boundary could be rewritten to use only v2 `proto.Message`, but that would remove support for legacy messages accepted by the public status API. The adapter is the narrowest bridge and preserves the compatibility contract. Likewise, merging the two `ToJSON` cases would trade away support for one of the message interfaces rather than simplify the underlying model.

## Verification status

Static inspection only; no tests were run. `git diff --check main...review-head` produced no output. Review evidence came from the committed diff and the changed call-site/test context. No code-quality finding met the bar for an actionable report item.
