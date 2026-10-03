package main
import (
 "fmt"
 oldproto "github.com/golang/protobuf/proto"
 "github.com/golang/protobuf/ptypes"
 "google.golang.org/grpc/codes"
 "google.golang.org/grpc/encoding"
 _ "google.golang.org/grpc/encoding/proto"
 legacy "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 "google.golang.org/grpc/status"
 "google.golang.org/protobuf/protoadapt"
 "google.golang.org/protobuf/types/known/durationpb"
)
func main() {
 m := &legacy.SearchRequestV3{Query: "legacy request"}
 b, err := oldproto.Marshal(m)
 fmt.Printf("old marshal: bytes=%x err=%v\n", b, err)
 c := encoding.GetCodec("proto")
 _, err = c.Marshal(m)
 fmt.Printf("head codec marshal: %v\n", err)
 err = c.Unmarshal(b, new(legacy.SearchRequestV3))
 fmt.Printf("head codec unmarshal: %v\n", err)
 s, err := status.New(codes.Internal, "legacy detail").WithDetails(m)
 fmt.Printf("WithDetails: %v\n", err)
 got := s.Details()[0]
 _, ok := got.(*legacy.SearchRequestV3)
 fmt.Printf("head Details: type=%T original_type=%v\n", got, ok)
 adapted := protoadapt.MessageV1Of(got.(protoadapt.MessageV2))
 _, ok = adapted.(*legacy.SearchRequestV3)
 fmt.Printf("adapted Details: type=%T original_type=%v value=%v\n", adapted, ok, adapted)
 originalAny, err := ptypes.MarshalAny(m)
 originalResult := new(ptypes.DynamicAny)
 err = ptypes.UnmarshalAny(originalAny, originalResult)
 fmt.Printf("base detail operation: type=%T err=%v\n", originalResult.Message, err)
 for _, d := range []*durationpb.Duration{{Seconds:10000000000},{Seconds:-10000000000},{Seconds:9223372036,Nanos:854775807},{Seconds:9223372036,Nanos:854775808}} {
  old, err := ptypes.Duration(d)
  fmt.Printf("duration %v: base=%v err=%v head=%v valid=%v\n", d, old, err, d.AsDuration(), d.CheckValid())
 }
}
