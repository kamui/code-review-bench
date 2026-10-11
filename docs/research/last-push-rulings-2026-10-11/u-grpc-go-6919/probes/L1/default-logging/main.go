// What a program with grpc-go's standard logger prints. Not part of grpc-go.
//
// Copied to xds/zzl1probe/main.go of a checkout and run as a plain program, outside the test harness, so that
// grpc-go's own default logger is in use. The fake management server answers each load-report stream with an
// interval of PROBE_SECONDS seconds. Everything the client writes goes to stderr, which run.sh captures.
package main

import (
	"fmt"
	"os"
	"strconv"
	"time"

	v3corepb "github.com/envoyproxy/go-control-plane/envoy/config/core/v3"
	v3lrspb "github.com/envoyproxy/go-control-plane/envoy/service/load_stats/v3"
	"google.golang.org/grpc/internal/testutils/xds/fakeserver"
	"google.golang.org/grpc/xds/internal/xdsclient/bootstrap"
	"google.golang.org/grpc/xds/internal/xdsclient/transport"
	"google.golang.org/protobuf/types/known/durationpb"
)

func main() {
	seconds, _ := strconv.ParseInt(os.Getenv("PROBE_SECONDS"), 10, 64)
	server, cleanup, err := fakeserver.StartServer(nil)
	if err != nil {
		panic(err)
	}
	defer cleanup()
	go func() {
		for {
			server.LRSResponseChan <- &fakeserver.Response{Resp: &v3lrspb.LoadStatsResponse{
				SendAllClusters:       true,
				LoadReportingInterval: &durationpb.Duration{Seconds: seconds},
			}}
		}
	}()
	cfg, err := bootstrap.ServerConfigFromJSON([]byte(fmt.Sprintf(`{"server_uri": %q, "channel_creds": [{"type": "insecure"}], "server_features": ["xds_v3"]}`, server.Address)))
	if err != nil {
		panic(err)
	}
	tr, err := transport.New(transport.Options{
		ServerCfg:      *cfg,
		NodeProto:      &v3corepb.Node{Id: "probe"},
		OnRecvHandler:  func(transport.ResourceUpdate) error { return nil },
		OnErrorHandler: func(error) {},
		OnSendHandler:  func(*transport.ResourceSendInfo) {},
		Backoff:        func(int) time.Duration { return 500 * time.Millisecond },
	})
	if err != nil {
		panic(err)
	}
	_, stop := tr.ReportLoad()
	time.Sleep(2200 * time.Millisecond)
	fmt.Println("PROBE the program ran 2.2 s and is stopping")
	stop()
	tr.Close()
}
