package proto_test

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
 input := &legacy.SearchRequestV3{Query: "legacy payload"}
 wire, err := oldproto.Marshal(input)
 if err != nil { t.Fatal(err) }
 codec := encoding.GetCodec("proto")
 got, err := codec.Marshal(input)
 if err != nil { t.Errorf("Marshal legacy message: %v", err) } else if !bytes.Equal(got,wire) { t.Errorf("wire bytes %x, want %x",got,wire) }
 output := new(legacy.SearchRequestV3)
 if err := codec.Unmarshal(wire,output); err != nil { t.Errorf("Unmarshal legacy message: %v",err) } else if output.Query != input.Query { t.Errorf("decoded %v",output) }
}

func TestReviewLegacyStatusDetails(t *testing.T) {
 input := &legacy.SearchRequestV3{Query: "legacy detail"}
 st, err := status.New(codes.Internal,"test").WithDetails(input)
 if err != nil { t.Fatal(err) }
 // Exercise the public FromProto path as well as WithDetails and Details.
 st = status.FromProto(st.Proto())
 details := st.Details()
 if len(details) != 1 { t.Fatalf("details: %v",details) }
 output, ok := details[0].(*legacy.SearchRequestV3)
 if !ok { t.Fatalf("detail has type %T, want *grpc_testing_not_regenerate.SearchRequestV3",details[0]) }
 if output.Query != input.Query { t.Errorf("decoded %v",output) }
}

func TestReviewLegacyBinaryLog(t *testing.T) {
 input := &legacy.SearchRequestV3{Query: "legacy log"}
 wire, err := oldproto.Marshal(input)
 if err != nil { t.Fatal(err) }
 logger := binarylog.NewTruncatingMethodLogger(^uint64(0),^uint64(0))
 for _, c := range []binarylog.LogEntryConfig{
  &binarylog.ClientMessage{Message:input},
  &binarylog.ServerMessage{Message:input},
 } {
  got := logger.Build(c).GetMessage()
  if !bytes.Equal(got.GetData(),wire) || got.GetLength() != uint32(len(wire)) {
   t.Errorf("%T recorded data=%x length=%d, want data=%x length=%d",c,got.GetData(),got.GetLength(),wire,len(wire))
  }
 }
}
