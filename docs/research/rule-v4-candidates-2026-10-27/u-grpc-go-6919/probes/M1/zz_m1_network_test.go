package transport_test

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"sync/atomic"
	"testing"
	"time"

	v3corepb "github.com/envoyproxy/go-control-plane/envoy/config/core/v3"
	v3lrspb "github.com/envoyproxy/go-control-plane/envoy/service/load_stats/v3"
	"github.com/google/uuid"
	"google.golang.org/grpc/internal/testutils/xds/fakeserver"
	"google.golang.org/grpc/xds/internal/testutils"
	"google.golang.org/grpc/xds/internal/xdsclient/transport"
	"google.golang.org/protobuf/types/known/durationpb"
)

func probeEnvInt(name string, fallback int64) int64 {
	v, err := strconv.ParseInt(os.Getenv(name), 10, 64)
	if err != nil {
		return fallback
	}
	return v
}

func (s) TestProbeM1(t *testing.T) {
	seconds := probeEnvInt("PROBE_SECONDS", 0)
	nanos := int32(probeEnvInt("PROBE_NANOS", 0))
	window := time.Duration(probeEnvInt("PROBE_WINDOW_MS", 3000)) * time.Millisecond
	retryDelay := 500 * time.Millisecond

	mgmtServer, cleanup := startFakeManagementServer(t)
	defer cleanup()

	tr, err := transport.New(transport.Options{
		ServerCfg:      *testutils.ServerConfigForAddress(t, mgmtServer.Address),
		NodeProto:      &v3corepb.Node{Id: uuid.New().String()},
		OnRecvHandler:  func(transport.ResourceUpdate) error { return nil },
		OnErrorHandler: func(error) {},
		OnSendHandler:  func(*transport.ResourceSendInfo) {},
		Backoff:        func(int) time.Duration { return retryDelay },
	})
	if err != nil {
		t.Fatalf("Failed to create xDS transport: %v", err)
	}
	defer tr.Close()

	interval := &durationpb.Duration{Seconds: seconds, Nanos: nanos}
	fmt.Printf("PROBE interval sent by the server: seconds=%d nanos=%d; watching for %v; client retry delay %v\n", seconds, nanos, window, retryDelay)

	ctx, cancel := context.WithTimeout(context.Background(), window)
	defer cancel()

	var opened, closed, firstRequests, loadReports int64
	start := time.Now()
	done := make(chan struct{}, 3)

	go func() {
		defer func() { done <- struct{}{} }()
		for {
			if _, err := mgmtServer.LRSStreamOpenChan.Receive(ctx); err != nil {
				return
			}
			n := atomic.AddInt64(&opened, 1)
			fmt.Printf("PROBE t=+%dms server: load-report stream %d opened\n", time.Since(start).Milliseconds(), n)
			select {
			case mgmtServer.LRSResponseChan <- &fakeserver.Response{Resp: &v3lrspb.LoadStatsResponse{SendAllClusters: true, LoadReportingInterval: interval}}:
			case <-ctx.Done():
				return
			}
		}
	}()
	go func() {
		defer func() { done <- struct{}{} }()
		for {
			if _, err := mgmtServer.LRSStreamCloseChan.Receive(ctx); err != nil {
				return
			}
			n := atomic.AddInt64(&closed, 1)
			fmt.Printf("PROBE t=+%dms server: a load-report stream closed (%d so far)\n", time.Since(start).Milliseconds(), n)
		}
	}()
	go func() {
		defer func() { done <- struct{}{} }()
		for {
			r, err := mgmtServer.LRSRequestChan.Receive(ctx)
			if err != nil {
				return
			}
			req, ok := r.(*fakeserver.Request).Req.(*v3lrspb.LoadStatsRequest)
			if !ok || req == nil {
				continue
			}
			if req.GetNode() != nil {
				atomic.AddInt64(&firstRequests, 1)
				continue
			}
			n := atomic.AddInt64(&loadReports, 1)
			if n <= 3 {
				fmt.Printf("PROBE t=+%dms server: load report %d received, clusters=%d\n", time.Since(start).Milliseconds(), n, len(req.GetClusterStats()))
			}
		}
	}()

	store, cancelLRS := tr.ReportLoad()
	store.PerCluster("cluster1", "eds1").CallDropped("test")

	<-ctx.Done()
	for i := 0; i < 3; i++ {
		<-done
	}
	fmt.Printf("PROBE RESULT after %v: streams opened=%d, streams closed=%d, first requests=%d, load reports received=%d\n",
		window, atomic.LoadInt64(&opened), atomic.LoadInt64(&closed), atomic.LoadInt64(&firstRequests), atomic.LoadInt64(&loadReports))

	stopped := make(chan struct{})
	go func() { cancelLRS(); close(stopped) }()
	select {
	case <-stopped:
		fmt.Println("PROBE load reporting stopped cleanly when cancelled")
	case <-time.After(5 * time.Second):
		fmt.Println("PROBE load reporting did not stop within 5s of being cancelled")
	}
}
