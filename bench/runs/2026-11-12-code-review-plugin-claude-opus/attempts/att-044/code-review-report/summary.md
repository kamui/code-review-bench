# Code review: grpc/grpc-go#6919

Range reviewed: `5051eeae537cb2839dd499e1a63a141098a3a03a..b8374114d485b6957b15d8769d7d5d96ddeaafc6` (`main...review-head`).

Found 2 issues. Checked for bugs and CLAUDE.md compliance; the repository has no CLAUDE.md files.

## 1. Invalid `load_reporting_interval` error reports the wrong (always nil) error

- **Reason flagged:** bug
- **File:** `xds/internal/xdsclient/transport/loadreport.go`, lines 173-177 (in `recvFirstLoadStatsResponse`)
- **Description:** The migration replaced `interval, err := ptypes.Duration(resp.GetLoadReportingInterval())` with `rInterval := resp.GetLoadReportingInterval(); if rInterval.CheckValid() != nil { return nil, 0, fmt.Errorf("invalid load_reporting_interval: %v", err) }`. The result of `CheckValid()` is thrown away, and the `err` being formatted is the one from `resp, err := stream.Recv()` at line 165.
- **Why it fails:** Line 166 returns early whenever that `stream.Recv()` error is non-nil, so `err` is always nil at line 175. Every invalid or missing interval therefore produces `invalid load_reporting_interval: <nil>`, and the real validation reason is lost. Before this PR, `err` was rebound to the `ptypes.Duration` error, so the message carried the cause. Control flow is unchanged because a non-nil error is still returned. Only the diagnostic is wrong. Fix: `if err := rInterval.CheckValid(); err != nil { ... }`.

## 2. Default gRPC proto codec now rejects messages that implement only the legacy APIv1 interface

- **Reason flagged:** bug
- **File:** `encoding/proto/proto.go`, lines 27-54 (the import at line 27, `Marshal` at lines 40-46, `Unmarshal` at lines 48-54)
- **Description:** The PR changes only the import from `github.com/golang/protobuf/proto` to `google.golang.org/protobuf/proto`. The `Marshal` and `Unmarshal` bodies, which do `vv, ok := v.(proto.Message)` and then call `proto.Marshal` or `proto.Unmarshal`, are left as they were.
- **Why it fails:** In `github.com/golang/protobuf` v1.5.3, `proto.Message` is `protoiface.MessageV1` (`Reset`/`String`/`ProtoMessage`), and its `Marshal`/`Unmarshal` wrap such values with `protoimpl.X.ProtoMessageV2Of`. In `google.golang.org/protobuf`, `proto.Message` is `protoreflect.ProtoMessage`, which requires `ProtoReflect()`. Messages that implement only the APIv1 interface used to work, for example output from older protoc-gen-go versions or gogo-style generators. They now fail the type assertion at lines 41 and 49, so every RPC using them through gRPC's default codec fails with "failed to marshal/unmarshal, message is %T, want proto.Message". Fix: accept `protoadapt.MessageV1` values as well and convert them with `protoadapt.MessageV2Of` before marshalling or unmarshalling.
