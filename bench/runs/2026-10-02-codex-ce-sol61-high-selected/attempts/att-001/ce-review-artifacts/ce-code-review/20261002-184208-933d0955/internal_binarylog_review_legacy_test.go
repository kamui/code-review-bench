package binarylog
import ("testing"; oldproto "github.com/golang/protobuf/proto")
type reviewLegacy struct { Value *string `protobuf:"bytes,1,opt,name=value" json:"value,omitempty"` }
func (m *reviewLegacy) Reset() { *m = reviewLegacy{} }
func (m *reviewLegacy) String() string { return oldproto.CompactTextString(m) }
func (*reviewLegacy) ProtoMessage() {}

func TestReviewLegacyBinarylog(t *testing.T) {
 msg:=&reviewLegacy{Value:oldproto.String("legacy payload")}
 wire,err:=oldproto.Marshal(msg); if err!=nil { t.Fatal(err) }
 c:=(&ClientMessage{Message:msg}).toProto().GetMessage()
 s:=(&ServerMessage{Message:msg}).toProto().GetMessage()
 if len(c.Data)!=len(wire) { t.Errorf("client payload lost: length=%d want=%d",len(c.Data),len(wire)) }
 if len(s.Data)!=len(wire) { t.Errorf("server payload lost: length=%d want=%d",len(s.Data),len(wire)) }
}
