package status_test
import (
 "bytes"
 "testing"
 oldproto "github.com/golang/protobuf/proto"
 legacy "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 "google.golang.org/grpc/status"
 "google.golang.org/grpc/codes"
)

func TestReviewLegacyStatusDetails(t *testing.T) {
 input := &legacy.SearchRequestV3{Query: "legacy"}
 s, err := status.New(codes.Internal, "test").WithDetails(input)
 if err != nil { t.Fatal(err) }
 details := s.Details()
 if len(details) != 1 { t.Fatalf("got %v", details) }
 output, ok := details[0].(*legacy.SearchRequestV3)
 if !ok { t.Fatalf("detail concrete type = %T, want *grpc_testing_not_regenerate.SearchRequestV3", details[0]) }
 if !oldproto.Equal(input, output) { t.Fatalf("got %v want %v", output, input) }
 _ = bytes.Equal
}
