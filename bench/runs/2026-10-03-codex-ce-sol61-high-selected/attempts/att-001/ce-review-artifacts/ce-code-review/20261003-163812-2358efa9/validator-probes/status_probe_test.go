package status_test
import (
 "testing"
 "github.com/golang/protobuf/ptypes"
 "google.golang.org/grpc/codes"
 "google.golang.org/grpc/status"
 fixture "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 "google.golang.org/protobuf/protoadapt"
 modern "google.golang.org/genproto/googleapis/rpc/errdetails"
)
func TestValidatorLegacyDetails(t *testing.T) {
 msg := &fixture.SearchRequestV3{Query: "legacy probe"}
 s, err := status.New(codes.Internal, "probe").WithDetails(msg, &modern.ResourceInfo{ResourceName:"modern"})
 if err != nil { t.Fatalf("WithDetails: %v", err) }
 details := s.Details()
 if len(details) != 2 { t.Fatalf("Details length %d", len(details)) }
 if _, ok := details[0].(*fixture.SearchRequestV3); ok { t.Fatalf("reviewed Details returned concrete legacy type; claim refuted") }
 t.Logf("reviewed legacy detail type: %T", details[0])
 converted := protoadapt.MessageV1Of(details[0].(protoadapt.MessageV2))
 recovered, ok := converted.(*fixture.SearchRequestV3)
 if !ok || recovered.Query != msg.Query { t.Fatalf("adapted detail: %T %v", converted, converted) }
 old := &ptypes.DynamicAny{}
 if err := ptypes.UnmarshalAny(s.Proto().Details[0], old); err != nil { t.Fatalf("old Details: %v", err) }
 oldRecovered, ok := old.Message.(*fixture.SearchRequestV3)
 if !ok || oldRecovered.Query != msg.Query { t.Fatalf("old Details type/value: %T %v", old.Message, old.Message) }
 modernRecovered, ok := details[1].(*modern.ResourceInfo)
 if !ok || modernRecovered.ResourceName != "modern" { t.Fatalf("modern detail: %T %v", details[1], details[1]) }
 t.Logf("old Details and V1 adapter preserve concrete type %T and field value %q; modern detail remains %T",old.Message,oldRecovered.Query,details[1])
}
