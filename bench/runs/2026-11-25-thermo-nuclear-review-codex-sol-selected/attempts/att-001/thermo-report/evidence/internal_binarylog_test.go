package binarylog_test
import (
 "bytes"
 "testing"
 oldproto "github.com/golang/protobuf/proto"
 legacy "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 "google.golang.org/grpc/internal/binarylog"
)

func TestReviewLegacyBinaryLog(t *testing.T) {
 input := &legacy.SearchResponseV3{State: legacy.SearchResponseV3_FRESH}
 wire, err := oldproto.Marshal(input)
 if err != nil { t.Fatal(err) }
 logger := binarylog.NewTruncatingMethodLogger(^uint64(0), ^uint64(0))
 entry := logger.Build(&binarylog.ServerMessage{Message: input})
 got := entry.GetMessage()
 if got.Length != uint32(len(wire)) || !bytes.Equal(got.Data, wire) { t.Fatalf("logged length=%d data=%x, want length=%d data=%x", got.Length, got.Data, len(wire), wire) }
}
