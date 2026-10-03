package transport
import (
 "strings"
 "testing"
 v3lrspb "github.com/envoyproxy/go-control-plane/envoy/service/load_stats/v3"
 "google.golang.org/protobuf/types/known/durationpb"
)
type thermoLRSStream struct { lrsStream; response *v3lrspb.LoadStatsResponse }
func (s thermoLRSStream) Recv() (*v3lrspb.LoadStatsResponse, error) { return s.response, nil }
func TestThermoLRSInvalidDuration(t *testing.T) {
 tr := &Transport{}
 stream := thermoLRSStream{response: &v3lrspb.LoadStatsResponse{LoadReportingInterval:&durationpb.Duration{Nanos:1000000000}}}
 _, _, err := tr.recvFirstLoadStatsResponse(stream)
 t.Logf("invalid duration error: %v", err)
 if err == nil || strings.Contains(err.Error(), "<nil>") { t.Errorf("duration validation error lost: %v", err) }
}
func TestThermoLRSDurationOverflow(t *testing.T) {
 tr := &Transport{}
 stream := thermoLRSStream{response: &v3lrspb.LoadStatsResponse{LoadReportingInterval:&durationpb.Duration{Seconds:10000000000}}}
 _, got, err := tr.recvFirstLoadStatsResponse(stream)
 t.Logf("overflow duration: interval=%v error=%v", got, err)
 if err == nil { t.Errorf("unrepresentable reporting interval accepted") }
}
