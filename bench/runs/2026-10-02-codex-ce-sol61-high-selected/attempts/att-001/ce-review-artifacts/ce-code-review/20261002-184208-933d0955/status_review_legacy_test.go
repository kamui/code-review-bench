package status_test
import ("testing"; oldproto "github.com/golang/protobuf/proto"; "google.golang.org/grpc/status"; "google.golang.org/grpc/codes")
type reviewLegacy struct { Value *string `protobuf:"bytes,1,opt,name=value" json:"value,omitempty"` }
func (m *reviewLegacy) Reset() { *m = reviewLegacy{} }
func (m *reviewLegacy) String() string { return oldproto.CompactTextString(m) }
func (*reviewLegacy) ProtoMessage() {}

func TestReviewLegacyStatus(t *testing.T) {
 oldproto.RegisterType((*reviewLegacy)(nil),"review.LegacyStatus")
 msg := &reviewLegacy{Value:oldproto.String("legacy detail")}
 s,err:=status.New(codes.Internal,"test").WithDetails(msg); if err!=nil { t.Fatal(err) }
 details:=s.Details(); if len(details)!=1 { t.Fatalf("details count: %d",len(details)) }
 if _,ok:=details[0].(*reviewLegacy); !ok { t.Errorf("legacy detail type changed to %T, want *reviewLegacy",details[0]) }
}
