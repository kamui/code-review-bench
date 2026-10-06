package main

import (
 "fmt"
 oldproto "github.com/golang/protobuf/proto"
 "google.golang.org/grpc/codes"
 legacy "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 "google.golang.org/grpc/status"
 "google.golang.org/protobuf/proto"
 "google.golang.org/protobuf/protoadapt"
 "google.golang.org/protobuf/reflect/protoregistry"
 "google.golang.org/protobuf/types/known/wrapperspb"
)

type Unregistered struct { Value string `protobuf:"bytes,1,opt,name=value,proto3"` }
func (m *Unregistered) Reset() { *m = Unregistered{} }
func (m *Unregistered) String() string { return m.Value }
func (*Unregistered) ProtoMessage() {}

func check(label string, msg oldproto.Message) {
 s, err := status.New(codes.InvalidArgument, "probe").WithDetails(msg)
 fmt.Printf("%s WithDetails error=%v\n", label, err)
 if err != nil { return }
 s = status.FromProto(s.Proto())
 for _, d := range s.Details() {
  e, isError := d.(error)
  fmt.Printf("%s Details type=%T is_error=%v error=%v\n", label, d, isError, e)
  if label == "registered-legacy" {
   v, ok := d.(*legacy.SearchRequestV3)
   fmt.Printf("legacy original_type_matches=%v query=%q\n", ok, v.GetQuery())
   if modern, ok := d.(proto.Message); ok {
    restored := protoadapt.MessageV1Of(modern)
    v, ok := restored.(*legacy.SearchRequestV3)
    fmt.Printf("legacy after_MessageV1Of type=%T original_type_matches=%v query=%q\n", restored, ok, v.GetQuery())
   }
  }
 }
}

func main() {
 msg := &legacy.SearchRequestV3{Query:"hello"}
 name := oldproto.MessageName(msg)
 mt, err := protoregistry.GlobalTypes.FindMessageByName(protoadapt.MessageV2Of(msg).ProtoReflect().Descriptor().FullName())
 fmt.Printf("legacy name=%s v2_registry_found=%v registry_error=%v\n", name, mt != nil, err)
 check("registered-legacy", msg)
 check("unregistered-legacy", &Unregistered{Value:"hello"})
 check("current-generator", wrapperspb.String("hello"))
}
