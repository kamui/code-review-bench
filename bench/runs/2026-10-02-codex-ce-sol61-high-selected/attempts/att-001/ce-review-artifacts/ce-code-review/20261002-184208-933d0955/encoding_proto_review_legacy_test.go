package proto
import ("testing"; oldproto "github.com/golang/protobuf/proto")
type reviewLegacy struct { Value *string `protobuf:"bytes,1,opt,name=value" json:"value,omitempty"` }
func (m *reviewLegacy) Reset() { *m = reviewLegacy{} }
func (m *reviewLegacy) String() string { return oldproto.CompactTextString(m) }
func (*reviewLegacy) ProtoMessage() {}

func TestReviewLegacyCodec(t *testing.T) {
 msg := &reviewLegacy{Value:oldproto.String("legacy payload")}
 wire,err := oldproto.Marshal(msg); if err!=nil { t.Fatal(err) }
 c := codec{}
 if _,err:=c.Marshal(msg); err!=nil { t.Errorf("legacy Marshal rejected: %v",err) }
 out:=new(reviewLegacy)
 if err:=c.Unmarshal(wire,out); err!=nil { t.Errorf("legacy Unmarshal rejected: %v",err) }
}
