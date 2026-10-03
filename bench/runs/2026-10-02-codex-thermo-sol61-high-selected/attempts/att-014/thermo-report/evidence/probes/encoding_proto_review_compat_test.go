package proto_test
import (
 "testing"
 legacy "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 "google.golang.org/grpc/encoding"
 _ "google.golang.org/grpc/encoding/proto"
 oldproto "github.com/golang/protobuf/proto"
)
func TestReviewLegacyCodec(t *testing.T) {
 c := encoding.GetCodec("proto")
 m := &legacy.SearchRequestV3{Query:"legacy"}
 wire, err := oldproto.Marshal(m); if err != nil { t.Fatal(err) }
 t.Run("Marshal",func(t *testing.T){ if _,err:=c.Marshal(m); err!=nil {t.Fatal(err)} })
 t.Run("Unmarshal",func(t *testing.T){ var got legacy.SearchRequestV3; if err:=c.Unmarshal(wire,&got); err!=nil {t.Fatal(err)}; if got.Query!=m.Query {t.Fatalf("got %q",got.Query)} })
}
