package grpc_test

import (
	"net"
	"testing"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/balancer"
	"google.golang.org/grpc/connectivity"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/internal/balancer/stub"
	"google.golang.org/grpc/resolver"
	"google.golang.org/grpc/resolver/manual"
)

// Repeatedly move a READY sub-connection to a different address, so updateAddrs runs with a live
// transport: it defers the old transport's GracefulClose and hands the locked mutex to a goroutine.
func TestArenaUpdateAddrsWhileReady(t *testing.T) {
	addrs := make([]string, 2)
	for i := range addrs {
		lis, err := net.Listen("tcp", "localhost:0")
		if err != nil {
			t.Fatal(err)
		}
		server := grpc.NewServer()
		go server.Serve(lis)
		defer server.Stop()
		addrs[i] = lis.Addr().String()
	}
	ready := make(chan struct{}, 10000)
	var sc balancer.SubConn
	stub.Register("arena_probe", stub.BalancerFuncs{
		UpdateClientConnState: func(d *stub.BalancerData, ccs balancer.ClientConnState) error {
			if sc == nil {
				var err error
				sc, err = d.ClientConn.NewSubConn(ccs.ResolverState.Addresses, balancer.NewSubConnOptions{
					StateListener: func(s balancer.SubConnState) {
						switch s.ConnectivityState {
						case connectivity.Ready:
							ready <- struct{}{}
						case connectivity.Idle:
							sc.Connect()
						}
					},
				})
				if err != nil {
					return err
				}
				sc.Connect()
				return nil
			}
			d.ClientConn.UpdateAddresses(sc, ccs.ResolverState.Addresses)
			return nil
		},
	})
	r := manual.NewBuilderWithScheme("arena")
	r.InitialState(resolver.State{Addresses: []resolver.Address{{Addr: addrs[0]}}})
	cc, err := grpc.NewClient(r.Scheme()+":///x", grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithResolvers(r), grpc.WithDefaultServiceConfig(`{"loadBalancingConfig":[{"arena_probe":{}}]}`))
	if err != nil {
		t.Fatal(err)
	}
	defer cc.Close()
	cc.Connect()
	wait := func(i int) {
		select {
		case <-ready:
		case <-time.After(10 * time.Second):
			t.Fatalf("iteration %d: sub-connection did not become READY within 10s", i)
		}
	}
	wait(-1)
	start := time.Now()
	const rounds = 500
	for i := 0; i < rounds; i++ {
		r.UpdateState(resolver.State{Addresses: []resolver.Address{{Addr: addrs[(i+1)%2]}}})
		wait(i)
	}
	t.Logf("%d address switches on a READY sub-connection in %v (%.2f ms each)", rounds, time.Since(start), float64(time.Since(start).Milliseconds())/rounds)
}
