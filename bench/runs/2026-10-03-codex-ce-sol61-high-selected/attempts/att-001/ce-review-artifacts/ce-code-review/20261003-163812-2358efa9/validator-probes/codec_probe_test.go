package proto_test
import (
 "testing"
 legacy "github.com/golang/protobuf/proto"
 "google.golang.org/grpc/encoding"
 _ "google.golang.org/grpc/encoding/proto"
 fixture "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 modern "google.golang.org/protobuf/proto"
)
func TestValidatorLegacyCodec(t *testing.T) {
 msg := &fixture.SearchRequestV3{Query: "legacy probe"}
 if _, ok := any(msg).(modern.Message); ok { t.Fatal("fixture unexpectedly implements V2") }
 wire, err := legacy.Marshal(msg)
 if err != nil { t.Fatalf("old codec marshal: %v", err) }
 oldDest := new(fixture.SearchRequestV3)
 if err := legacy.Unmarshal(wire, oldDest); err != nil || oldDest.Query != msg.Query { t.Fatalf("old codec unmarshal: %v, %v", err, oldDest) }
 c := encoding.GetCodec("proto")
 _, err = c.Marshal(msg)
 if err == nil { t.Fatal("reviewed codec accepted legacy fixture; claim refuted") }
 t.Logf("reviewed codec Marshal: %v; old codec round trip succeeded", err)
 err = c.Unmarshal(wire, new(fixture.SearchRequestV3))
 if err == nil { t.Fatal("reviewed codec decoded legacy fixture; claim refuted") }
 t.Logf("reviewed codec Unmarshal: %v", err)
}
