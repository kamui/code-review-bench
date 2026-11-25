package transport
import (
 "strings"
 "testing"
 v3lrspb "github.com/envoyproxy/go-control-plane/envoy/service/load_stats/v3"
 "google.golang.org/protobuf/types/known/durationpb"
)
type reviewLRSStream struct {
 lrsStream
 response *v3lrspb.LoadStatsResponse
}
func (s reviewLRSStream) Recv() (*v3lrspb.LoadStatsResponse, error) { return s.response, nil }
func TestReviewInvalidLoadIntervalDiagnostic(t *testing.T) {
 for _, interval := range []*durationpb.Duration{nil, {Nanos: 1000000000}} {
  _, _, err := new(Transport).recvFirstLoadStatsResponse(reviewLRSStream{response: &v3lrspb.LoadStatsResponse{LoadReportingInterval: interval}})
  if err == nil { t.Errorf("interval %v was accepted", interval); continue }
  if strings.Contains(err.Error(), "<nil>") { t.Errorf("interval %v: validation cause was discarded: %v", interval, err) }
 }
}
