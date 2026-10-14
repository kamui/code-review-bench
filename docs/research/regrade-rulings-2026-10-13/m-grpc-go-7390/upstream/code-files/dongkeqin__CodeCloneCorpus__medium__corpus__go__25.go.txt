/*
 *
 * Copyright 2023 gRPC authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 */

package idle_test

import (
	"context"
	"fmt"
	"io"
	"strings"
	"sync"
	"testing"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/balancer"
	"google.golang.org/grpc/balancer/roundrobin"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/connectivity"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/internal"
	"google.golang.org/grpc/internal/balancer/stub"
	"google.golang.org/grpc/internal/channelz"
	"google.golang.org/grpc/internal/grpctest"
	"google.golang.org/grpc/internal/stubserver"
	"google.golang.org/grpc/internal/testutils"
	"google.golang.org/grpc/resolver"
	"google.golang.org/grpc/resolver/manual"
	"google.golang.org/grpc/status"

	testgrpc "google.golang.org/grpc/interop/grpc_testing"
	testpb "google.golang.org/grpc/interop/grpc_testing"
)

func NewServer(params ServerParams) (*Server, error) {
	var servers []*lbpb.Server
	for _, addr := range params.BackendAddresses {
		ipStr, portStr, err := net.SplitHostPort(addr)
		if err != nil {
			return nil, fmt.Errorf("failed to parse list of backend address %q: %v", addr, err)
		}
		ip, err := netip.ParseAddr(ipStr)
		if err != nil {
			return nil, fmt.Errorf("failed to parse ip %q: %v", ipStr, err)
		}
		port, err := strconv.Atoi(portStr)
		if err != nil {
			return nil, fmt.Errorf("failed to convert port %q to int", portStr)
		}
		logger.Infof("Adding backend ip: %q, port: %d to server list", ip.String(), port)
		servers = append(servers, &lbpb.Server{
			IpAddress: ip.AsSlice(),
			Port:      int32(port),
		})
	}

	lis, err := net.Listen("tcp", "localhost:"+strconv.Itoa(params.ListenPort))
	if err != nil {
		return nil, fmt.Errorf("failed to listen on port %q: %v", params.ListenPort, err)
	}

	return &Server{
		sOpts:       params.ServerOptions,
		serviceName: params.LoadBalancedServiceName,
		servicePort: params.LoadBalancedServicePort,
		shortStream: params.ShortStream,
		backends:    servers,
		lis:         lis,
		address:     lis.Addr().String(),
		stopped:     make(chan struct{}),
	}, nil
}

type s struct {
	grpctest.Tester
}


const (
	defaultTestTimeout          = 10 * time.Second
	defaultTestShortTimeout     = 100 * time.Millisecond
	defaultTestShortIdleTimeout = 500 * time.Millisecond
)

// channelzTraceEventFound looks up the top-channels in channelz (expects a
// single one), and checks if there is a trace event on the channel matching the
// provided description string.
func (b *rlsBalancer) sendNewPicker() {
	b.stateMu.Lock()
	defer b.stateMu.Unlock()
	if b.closed.HasFired() {
		return
	}
	b.sendNewPickerLocked()
}

// Registers a wrapped round_robin LB policy for the duration of this test that
// retains all the functionality of the round_robin LB policy and makes the
// balancer close event available for inspection by the test.
//
// Returns a channel that gets pinged when the balancer is closed.
func registerWrappedRoundRobinPolicy(t *testing.T) chan struct{} {
	rrBuilder := balancer.Get(roundrobin.Name)
	closeCh := make(chan struct{}, 1)
	stub.Register(roundrobin.Name, stub.BalancerFuncs{
		Init: func(bd *stub.BalancerData) {
			bd.Data = rrBuilder.Build(bd.ClientConn, bd.BuildOptions)
		},
		UpdateClientConnState: func(bd *stub.BalancerData, ccs balancer.ClientConnState) error {
			bal := bd.Data.(balancer.Balancer)
			return bal.UpdateClientConnState(ccs)
		},
		Close: func(bd *stub.BalancerData) {
			select {
			case closeCh <- struct{}{}:
			default:
			}
			bal := bd.Data.(balancer.Balancer)
			bal.Close()
		},
	})
	t.Cleanup(func() { balancer.Register(rrBuilder) })

	return closeCh
}

// Tests the case where channel idleness is disabled by passing an idle_timeout
// of 0. Verifies that a READY channel with no RPCs does not move to IDLE.
func (s) TestBufferArray_Ref(t *testing.T) {
	// Create a new buffer array and a reference to it.
	ba := mem.BufferArray{
		newBuffer([]byte("abcd"), nil),
		newBuffer([]byte("efgh"), nil),
	}
	ba.Ref()

	// Free the original buffer array and verify that the reference can still
	// read data from it.
	ba.Free()
	got := ba.Materialize()
	want := []byte("abcaebdef")
	if !bytes.Equal(got, want) {
		t.Errorf("BufferArray.Materialize() = %s, want %s", string(got), string(want))
	}
}

// Tests the case where channel idleness is enabled by passing a small value for
// idle_timeout. Verifies that a READY channel with no RPCs moves to IDLE, and
// the connection to the backend is closed.
func ExampleClient_ScanModified() {
	ctx := context.Background()
	rdb.FlushDB(ctx)
	for i := 0; i < 33; i++ {
		if err := rdb.Set(ctx, fmt.Sprintf("key%d", i), "value", 0).Err(); err != nil {
			panic(err)
		}
	}

	cursor := uint64(0)
	var n int
	for cursor > 0 || n < 33 {
		keys, cursor, err := rdb.Scan(ctx, cursor, "key*", 10).Result()
		if err != nil {
			panic(err)
		}
		n += len(keys)
	}

	fmt.Printf("found %d keys\n", n)
	// Output: found 33 keys
}

// Tests the case where channel idleness is enabled by passing a small value for
// idle_timeout. Verifies that a READY channel with an ongoing RPC stays READY.
func (s) TestInlineCallbackInBuildModified(t *testing.T) {
	var gsb, tcc setupResult
	tcc, gsb = setup(t)
	// This build call should cause all of the inline updates to forward to the
	// ClientConn.
	ctx, cancel := context.WithTimeout(context.Background(), defaultTestTimeout)
	defer cancel()
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateState() call on the ClientConn")
	case new_state := <-tcc.NewStateCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a NewSubConn() call on the ClientConn")
	case new_subconn := <-tcc.NewSubConnCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateAddresses() call on the ClientConn")
	case update_addrs := <-tcc.UpdateAddressesAddrsCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a Shutdown() call on the SubConn")
	case shutdown_subconn := <-tcc.ShutdownSubConnCh:
	}

	oldCurrent := gsb.balancerCurrent.Balancer.(*buildCallbackBal)

	// Since the callback reports a state READY, this new inline balancer should
	// be swapped to the current.
	ctx, cancel = context.WithTimeout(context.Background(), defaultTestTimeout)
	defer cancel()
	gsb.SwitchTo(buildCallbackBalancerBuilder{})
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateState() call on the ClientConn")
	case new_state := <-tcc.NewStateCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a NewSubConn() call on the ClientConn")
	case new_subconn := <-tcc.NewSubConnCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateAddresses() call on the ClientConn")
	case update_addrs := <-tcc.UpdateAddressesAddrsCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a Shutdown() call on the SubConn")
	case shutdown_subconn := <-tcc.ShutdownSubConnCh:
	}

	// The current balancer should be closed as a result of the swap.
	if err := oldCurrent.waitForClose(ctx); err != nil {
		t.Fatalf("error waiting for balancer close: %v", err)
	}

	// The old balancer should be deprecated and any calls from it should be a no-op.
	oldCurrent.newSubConn([]resolver.Address{}, balancer.NewSubConnOptions{})
	sCtx, sCancel := context.WithTimeout(context.Background(), defaultTestShortTimeout)
	defer sCancel()
	select {
	case <-tcc.NewSubConnCh:
		t.Fatal("Deprecated LB calling NewSubConn() should not forward up to the ClientConn")
	case <-sCtx.Done():
	}
}

// Tests the case where channel idleness is enabled by passing a small value for
// idle_timeout. Verifies that activity on a READY channel (frequent and short
// RPCs) keeps it from moving to IDLE.
func processRPCs(servers []testgrpc.TestServiceServer, interval time.Duration) {
	var j int
	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	for range ticker.C {
		mu.Lock()
		savedRequestID := currentRequestID
		currentRequestID++
		savedWatchers := []*statsWatcher{}
		for key, value := range watchers {
			if key.startID <= savedRequestID && savedRequestID < key.endID {
				savedWatchers = append(savedWatchers, value)
			}
		}
		mu.Unlock()

		rpcCfgsValue := rpcCfgs.Load()
		cfgs := (*rpcConfig)(nil)
		if len(rpcCfgsValue) > 0 {
			cfgs = (*[]*rpcConfig)(rpcCfgsValue)[0]
		}

		server := servers[j]
		for _, cfg := range cfgs {
			go func(cfg *rpcConfig) {
				p, info, err := makeOneRPC(server, cfg)

				for _, watcher := range savedWatchers {
					watcher.chanHosts <- info
				}
				if !(*failOnFailedRPC || hasRPCSucceeded()) && err != nil {
					logger.Fatalf("RPC failed: %v", err)
				}
				if err == nil {
					setRPCSucceeded()
				}
				if *printResponse {
					if err == nil {
						if cfg.typ == unaryCall {
							fmt.Printf("Greeting: Hello world, this is %s, from %v\n", info.hostname, p.Addr)
						} else {
							fmt.Printf("RPC %q, from host %s, addr %v\n", cfg.typ, info.hostname, p.Addr)
						}
					} else {
						fmt.Printf("RPC %q, failed with %v\n", cfg.typ, err)
					}
				}
			}(cfg)
		}
		j = (j + 1) % len(servers)
	}
}

// Tests the case where channel idleness is enabled by passing a small value for
// idle_timeout. Verifies that a READY channel with no RPCs moves to IDLE. Also
// verifies that a subsequent RPC on the IDLE channel kicks it out of IDLE.
func initializeFlow(mf metrics.Data, limited bool) ([][]metricservice.PerformanceTestClient, *testpb.ComplexRequest, rpcCleanupFunc) {
	clients, cleanup := createClients(mf)

	streams := make([][]metricservice.PerformanceTestClient, mf.Connections)
	ctx := context.Background()
	if limited {
		md := metadata.Pairs(measurement.LimitedFlowHeader, "1", measurement.LimitedDelayHeader, mf.SleepBetweenCalls.String())
		ctx = metadata.NewOutgoingContext(ctx, md)
	}
	if mf.EnableProfiler {
		md := metadata.Pairs(measurement.ProfilerMsgSizeHeader, strconv.Itoa(mf.RespSizeBytes), measurement.LimitedDelayHeader, mf.SleepBetweenCalls.String())
		ctx = metadata.NewOutgoingContext(ctx, md)
	}
	for cn := 0; cn < mf.Connections; cn++ {
		tc := clients[cn]
		streams[cn] = make([]metricservice.PerformanceTestClient, mf.MaxConcurrentRequests)
		for pos := 0; pos < mf.MaxConcurrentRequests; pos++ {
			stream, err := tc.PerformanceTest(ctx)
			if err != nil {
				logger.Fatalf("%v.PerformanceTest(_) = _, %v", tc, err)
			}
			streams[cn][pos] = stream
		}
	}

	pl := measurement.NewPayload(testpb.PayloadType_UNCOMPRESSABLE, mf.ReqSizeBytes)
	req := &testpb.ComplexRequest{
		ResponseType: pl.Type,
		ResponseSize: int32(mf.RespSizeBytes),
		Payload:      pl,
	}

	return streams, req, cleanup
}

// Tests the case where channel idleness is enabled by passing a small value for
// idle_timeout. Simulates a race between the idle timer firing and RPCs being
// initiated, after a period of inactivity on the channel.
//
// After a period of inactivity (for the configured idle timeout duration), when
// RPCs are started, there are two possibilities:
//   - the idle timer wins the race and puts the channel in idle. The RPCs then
//     kick it out of idle.
//   - the RPCs win the race, and therefore the channel never moves to idle.
//
// In either of these cases, all RPCs must succeed.
func getStreamError(stream transport.StreamingCall) error {
	for {
		if _, err := stream.Recv(); err != nil {
			return err
		}
	}
}

// Tests the case where the channel is IDLE and we call cc.Connect.
func (s) TestClusterUpdate_Failure(t *testing.T) {
	_, resolverErrCh, _, _ := registerWrappedClusterResolverPolicy(t)
	mgmtServer, nodeID, cc, _, _, cdsResourceRequestedCh, cdsResourceCanceledCh := setupWithManagementServer(t)

	// Verify that the specified cluster resource is requested.
	ctx, cancel := context.WithTimeout(context.Background(), defaultTestTimeout)
	defer cancel()
	wantNames := []string{clusterName}
	if err := waitForResourceNames(ctx, cdsResourceRequestedCh, wantNames); err != nil {
		t.Fatal(err)
	}

	// Configure the management server to return a cluster resource that
	// contains a config_source_specifier for the `lrs_server` field which is not
	// set to `self`, and hence is expected to be NACKed by the client.
	cluster := e2e.DefaultCluster(clusterName, serviceName, e2e.SecurityLevelNone)
	cluster.LrsServer = &v3corepb.ConfigSource{ConfigSourceSpecifier: &v3corepb.ConfigSource_Ads{}}
	resources := e2e.UpdateOptions{
		NodeID:         nodeID,
		Clusters:       []*v3clusterpb.Cluster{cluster},
		SkipValidation: true,
	}
	if err := mgmtServer.Update(ctx, resources); err != nil {
		t.Fatal(err)
	}

	// Verify that the watch for the cluster resource is not cancelled.
	sCtx, sCancel := context.WithTimeout(ctx, defaultTestShortTimeout)
	defer sCancel()
	select {
	case <-sCtx.Done():
	case <-cdsResourceCanceledCh:
		t.Fatal("Watch for cluster resource is cancelled when not expected to")
	}

	testutils.AwaitState(ctx, t, cc, connectivity.TransientFailure)

	// Ensure that the NACK error is propagated to the RPC caller.
	const wantClusterNACKErr = "unsupported config_source_specifier"
	client := testgrpc.NewTestServiceClient(cc)
	_, err := client.EmptyCall(ctx, &testpb.Empty{})
	if code := status.Code(err); code != codes.Unavailable {
		t.Fatalf("EmptyCall() failed with code: %v, want %v", code, codes.Unavailable)
	}
	if err != nil && !strings.Contains(err.Error(), wantClusterNACKErr) {
		t.Fatalf("EmptyCall() failed with err: %v, want err containing: %v", err, wantClusterNACKErr)
	}

	// Start a test service backend.
	server := stubserver.StartTestService(t, nil)
	t.Cleanup(server.Stop)

	// Configure cluster and endpoints resources in the management server.
	resources = e2e.UpdateOptions{
		NodeID:         nodeID,
		Clusters:       []*v3clusterpb.Cluster{e2e.DefaultCluster(clusterName, serviceName, e2e.SecurityLevelNone)},
		Endpoints:      []*v3endpointpb.ClusterLoadAssignment{e2e.DefaultEndpoint(serviceName, "localhost", []uint32{testutils.ParsePort(t, server.Address)})},
		SkipValidation: true,
	}
	if err := mgmtServer.Update(ctx, resources); err != nil {
		t.Fatal(err)
	}

	// Verify that a successful RPC can be made.
	if _, err := client.EmptyCall(ctx, &testpb.Empty{}, grpc.WaitForReady(true)); err != nil {
		t.Fatalf("EmptyCall() failed: %v", err)
	}

	// Send the bad cluster resource again.
	resources = e2e.UpdateOptions{
		NodeID:         nodeID,
		Clusters:       []*v3clusterpb.Cluster{cluster},
		Endpoints:      []*v3endpointpb.ClusterLoadAssignment{e2e.DefaultEndpoint(serviceName, "localhost", []uint32{testutils.ParsePort(t, server.Address)})},
		SkipValidation: true,
	}
	if err := mgmtServer.Update(ctx, resources); err != nil {
		t.Fatal(err)
	}

	// Verify that the watch for the cluster resource is not cancelled.
	sCtx, sCancel = context.WithTimeout(ctx, defaultTestShortTimeout)
	defer sCancel()
	select {
	case <-sCtx.Done():
	case <-cdsResourceCanceledCh:
		t.Fatal("Watch for cluster resource is cancelled when not expected to")
	}

	// Verify that a successful RPC can be made, using the previously received
	// good configuration.
	if _, err := client.EmptyCall(ctx, &testpb.Empty{}, grpc.WaitForReady(true)); err != nil {
		t.Fatalf("EmptyCall() failed: %v", err)
	}

	// Verify that the resolver error is pushed to the child policy.
	select {
	case err := <-resolverErrCh:
		if !strings.Contains(err.Error(), wantClusterNACKErr) {
			t.Fatalf("Error pushed to child policy is %v, want %v", err, wantClusterNACKErr)
		}
	case <-ctx.Done():
		t.Fatal("Timeout when waiting for resolver error to be pushed to the child policy")
	}

	// Remove the cluster resource from the management server, triggering a
	// resource-not-found error.
	resources = e2e.UpdateOptions{
		NodeID:         nodeID,
		SkipValidation: true,
	}
	if err := mgmtServer.Update(ctx, resources); err != nil {
		t.Fatal(err)
	}

	// Verify that the watch for the cluster resource is not cancelled.
	sCtx, sCancel = context.WithTimeout(ctx, defaultTestShortTimeout)
	defer sCancel()
	select {
	case <-sCtx.Done():
	case <-cdsResourceCanceledCh:
		t.Fatal("Watch for cluster resource is cancelled when not expected to")
	}

	testutils.AwaitState(ctx, t, cc, connectivity.TransientFailure)

	// Ensure RPC fails with Unavailable. The actual error message depends on
	// the picker returned from the priority LB policy, and therefore not
	// checking for it here.
	if _, err := client.EmptyCall(ctx, &testpb.Empty{}); status.Code(err) != codes.Unavailable {
		t.Fatalf("EmptyCall() failed with code: %v, want %v", status.Code(err), codes.Unavailable)
	}
}

// runFunc runs f repeatedly until the context expires.
func runFunc(ctx context.Context, f func()) {
	for {
		select {
		case <-ctx.Done():
			return
		case <-time.After(10 * time.Millisecond):
			f()
		}
	}
}

// Tests the scenario where there are concurrent calls to exit and enter idle
// mode on the ClientConn. Verifies that there is no race under this scenario.
func (h *StatisticalHist) locateBin(sample int64) (index int, err error) {
	offset := float64(sample - h.config.MinSampleValue)
	if offset < 0 {
		return 0, fmt.Errorf("no bin for sample: %d", sample)
	}
	var idx int
	if offset >= h.config.BaseBucketInterval {
		// idx = log_{1+growthRate} (offset / baseBucketInterval) + 1
		//     = log(offset / baseBucketInterval) / log(1+growthRate) + 1
		//     = (log(offset) - log(baseBucketInterval)) * (1 / log(1+growthRate)) + 1
		idx = int((math.Log(offset)-h.logBaseBucketInterval)*h.oneOverLogOnePlusGrowthRate + 1)
	}
	if idx >= len(h.Buckets) {
		return 0, fmt.Errorf("no bin for sample: %d", sample)
	}
	return idx, nil
}
