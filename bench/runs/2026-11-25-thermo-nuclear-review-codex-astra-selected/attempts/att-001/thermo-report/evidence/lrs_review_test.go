package transport

import (
 "strings"
 "testing"
 lrspb "github.com/envoyproxy/go-control-plane/envoy/service/load_stats/v3"
 "google.golang.org/grpc/internal/grpclog"
 "google.golang.org/protobuf/types/known/durationpb"
)

type reviewLRSStream struct { lrsStream; response *lrspb.LoadStatsResponse }
func (s reviewLRSStream) Recv() (*lrspb.LoadStatsResponse,error) { return s.response,nil }
func TestReviewLRSValidationError(t *testing.T) {
 tr := &Transport{logger:grpclog.NewPrefixLogger(nil,"review")}
 for _, duration := range []*durationpb.Duration{nil,{Nanos:1000000000}} {
  _, _, err := tr.recvFirstLoadStatsResponse(reviewLRSStream{response:&lrspb.LoadStatsResponse{LoadReportingInterval:duration}})
  if err == nil { t.Fatal("invalid interval was accepted") }
  t.Logf("validation error: %v",err)
  if strings.Contains(err.Error(),"<nil>") { t.Errorf("validation failure lost its cause: %v",err) }
 }
}
