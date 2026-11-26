package status_test

import (
 "bytes"
 "testing"
 oldproto "github.com/golang/protobuf/proto"
 "google.golang.org/grpc/codes"
 "google.golang.org/grpc/encoding"
 _ "google.golang.org/grpc/encoding/proto"
 "google.golang.org/grpc/internal/binarylog"
 legacy "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 "google.golang.org/grpc/status"
)

func TestReviewLegacyCodec(t *testing.T) {
 m := &legacy.SearchRequestV3{Query: "legacy payload"}
 want, err := oldproto.Marshal(m)
 if err != nil { t.Fatal(err) }
 c := encoding.GetCodec("proto")
 got, err := c.Marshal(m)
 if err != nil || !bytes.Equal(got, want) { t.Errorf("Marshal = %x, %v; want %x", got, err, want) }
 out := new(legacy.SearchRequestV3)
 if err := c.Unmarshal(want, out); err != nil || out.Query != m.Query { t.Errorf("Unmarshal = %v, %v; want %v", out, err, m) }
}

func TestReviewLegacyStatusDetails(t *testing.T) {
 m := &legacy.SearchRequestV3{Query: "legacy detail"}
 s, err := status.New(codes.Internal, "failed").WithDetails(m)
 if err != nil { t.Fatal(err) }
 details := s.Details()
 if len(details) != 1 { t.Fatalf("details = %v", details) }
 out, ok := details[0].(*legacy.SearchRequestV3)
 if !ok { t.Fatalf("Details()[0] has type %T, want *legacy.SearchRequestV3", details[0]) }
 if !oldproto.Equal(m, out) { t.Fatalf("detail = %v, want %v", out, m) }
}

func TestReviewLegacyBinaryLog(t *testing.T) {
 m := &legacy.SearchRequestV3{Query: "legacy log payload"}
 want, err := oldproto.Marshal(m)
 if err != nil { t.Fatal(err) }
 logger := binarylog.NewTruncatingMethodLogger(^uint64(0), ^uint64(0))
 for name, event := range map[string]binarylog.LogEntryConfig{
  "client": &binarylog.ClientMessage{Message: m},
  "server": &binarylog.ServerMessage{Message: m},
 } {
  t.Run(name, func(t *testing.T) {
   got := logger.Build(event).GetMessage()
   if got.GetLength() != uint32(len(want)) || !bytes.Equal(got.GetData(), want) {
    t.Fatalf("logged length=%d data=%x; want length=%d data=%x", got.GetLength(), got.GetData(), len(want), want)
   }
  })
 }
}
