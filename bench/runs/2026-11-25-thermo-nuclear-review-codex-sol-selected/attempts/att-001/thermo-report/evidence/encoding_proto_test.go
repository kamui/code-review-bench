package proto_test
import (
 "bytes"
 "testing"
 oldproto "github.com/golang/protobuf/proto"
 legacy "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 "google.golang.org/grpc/encoding"
 _ "google.golang.org/grpc/encoding/proto"
)

func TestReviewLegacyCodec(t *testing.T) {
 input := &legacy.SearchRequestV3{Query: "legacy"}
 wire, err := oldproto.Marshal(input)
 if err != nil { t.Fatal(err) }
 c := encoding.GetCodec("proto")
 got, err := c.Marshal(input)
 if err != nil { t.Errorf("marshal legacy generated request: %v", err) } else if !bytes.Equal(got, wire) { t.Errorf("got %x want %x", got, wire) }
 output := new(legacy.SearchRequestV3)
 if err := c.Unmarshal(wire, output); err != nil { t.Errorf("unmarshal legacy generated request: %v", err) } else if output.Query != input.Query { t.Errorf("got %v want %v", output, input) }
}
