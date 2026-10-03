package transport
import (
 "context"
 "fmt"
 "math"
 "strings"
 "testing"
 "time"
 "github.com/golang/protobuf/ptypes"
 lrspb "github.com/envoyproxy/go-control-plane/envoy/service/load_stats/v3"
 "google.golang.org/protobuf/types/known/durationpb"
)
type validatorLRSStream struct {
 lrsStream
 resp *lrspb.LoadStatsResponse
}
func (s validatorLRSStream) Recv() (*lrspb.LoadStatsResponse, error) { return s.resp, nil }
func TestValidatorDurationOverflow(t *testing.T) {
 tr := new(Transport)
 for _, secs := range []int64{-315576000000, 315576000000} {
  intervalPB := &durationpb.Duration{Seconds:secs}
  if err := intervalPB.CheckValid(); err != nil { t.Fatalf("protobuf validity: %v",err) }
  _, oldErr := ptypes.Duration(intervalPB)
  if oldErr == nil { t.Fatal("old converter accepted overflow; claim refuted") }
  stream := validatorLRSStream{resp:&lrspb.LoadStatsResponse{LoadReportingInterval:intervalPB}}
  _, interval, err := tr.recvFirstLoadStatsResponse(stream)
  if err != nil { t.Fatalf("reviewed receive rejected interval; claim refuted: %v",err) }
  expected := time.Duration(math.MaxInt64)
  if secs < 0 { expected = time.Duration(math.MinInt64) }
  if interval != expected { t.Fatalf("interval=%v want saturation %v",interval,expected) }
  t.Logf("seconds=%d: old converter rejected (%v); reviewed receive accepted duration %v",secs,oldErr,interval)
  if secs < 0 {
   ctx,cancel := context.WithCancel(context.Background()); cancel()
   var panicValue any
   func() { defer func(){panicValue=recover()}(); tr.sendLoads(ctx,stream,nil,interval) }()
   if panicValue == nil || !strings.Contains(fmt.Sprint(panicValue),"non-positive interval") { t.Fatalf("sendLoads panic=%v; expected ticker panic",panicValue) }
   t.Logf("sendLoads reached time.NewTicker and panicked: %v",panicValue)
  }
 }
}
func TestValidatorDurationDiagnostic(t *testing.T) {
 invalid := &durationpb.Duration{Nanos:1000000000}
 validationErr := invalid.CheckValid()
 if validationErr == nil { t.Fatal("trigger is valid") }
 tr := new(Transport)
 _,_,err := tr.recvFirstLoadStatsResponse(validatorLRSStream{resp:&lrspb.LoadStatsResponse{LoadReportingInterval:invalid}})
 if err == nil || err.Error() != "invalid load_reporting_interval: <nil>" { t.Fatalf("diagnostic=%v; expected discarded reason",err) }
 t.Logf("reviewed diagnostic=%q; discarded validation reason=%q",err.Error(),validationErr.Error())
}
