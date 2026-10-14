/*
 * Second probe file for group M1 of m-grpc-go-7390. It runs the ways the
 * project itself handles "all addresses were removed", and needs
 * m1_probe_test.go beside it.
 */

package grpc_test

import (
	"errors"
	"fmt"
	"sync"
	"testing"
	"time"

	"google.golang.org/grpc/balancer"
	"google.golang.org/grpc/balancer/base"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/connectivity"
	"google.golang.org/grpc/internal/balancer/stub"
	"google.golang.org/grpc/resolver"
)

// m1_recreate follows the deprecation notice of UpdateAddresses ("Create new
// SubConns for new addresses instead") and what pick_first and grpclb do with
// an empty list: it shuts the SubConn down and creates a new one when
// addresses return.
type m1Recreate struct {
	mu   sync.Mutex
	sc   balancer.SubConn
	told []string
}

func init() {
	stub.Register("m1_recreate", stub.BalancerFuncs{
		Init: func(d *stub.BalancerData) { d.Data = &m1Recreate{} },
		UpdateClientConnState: func(d *stub.BalancerData, ccs balancer.ClientConnState) error {
			r := d.Data.(*m1Recreate)
			r.mu.Lock()
			defer r.mu.Unlock()
			addrs := ccs.ResolverState.Addresses
			if len(addrs) == 0 {
				if r.sc != nil {
					r.sc.Shutdown()
					r.sc = nil
				}
				d.ClientConn.UpdateState(balancer.State{ConnectivityState: connectivity.TransientFailure, Picker: base.NewErrPicker(errors.New("no addresses"))})
				return nil
			}
			if r.sc != nil {
				r.sc.Shutdown()
			}
			var sc balancer.SubConn
			sc, err := d.ClientConn.NewSubConn(addrs, balancer.NewSubConnOptions{StateListener: func(s balancer.SubConnState) {
				r.mu.Lock()
				current := r.sc == sc
				r.told = append(r.told, fmt.Sprintf("%v(current=%v)", s.ConnectivityState, current))
				r.mu.Unlock()
				if !current {
					return
				}
				switch s.ConnectivityState {
				case connectivity.Ready:
					d.ClientConn.UpdateState(balancer.State{ConnectivityState: connectivity.Ready, Picker: &m1Picker{sc: sc}})
				case connectivity.Idle:
					sc.Connect()
				}
			}})
			if err != nil {
				return err
			}
			r.sc = sc
			d.ClientConn.UpdateState(balancer.State{ConnectivityState: connectivity.Connecting, Picker: base.NewErrPicker(balancer.ErrNoSubConnAvailable)})
			sc.Connect()
			return nil
		},
	})
}

func TestM1DocumentedWayRecreate(t *testing.T) {
	rounds := m1Env("M1_ROUNDS", 50)
	counts := map[string]int{}
	for i := 0; i < rounds; i++ {
		h := newM1Harness(t, "m1_recreate", connectivity.Ready, false)
		line := "first RPC " + h.rpc(5*time.Second)
		h.r.UpdateState(resolver.State{Addresses: []resolver.Address{}})
		time.Sleep(m1Quiet)
		line += fmt.Sprintf(" | after an update with no addresses: channel %v, RPC with %v deadline: %s", h.cc.GetState(), m1ShortRPC, h.rpc(m1ShortRPC))
		h.r.UpdateState(resolver.State{Addresses: []resolver.Address{{Addr: h.serverAddr}}})
		line += fmt.Sprintf(" | addresses put back: RPC %s, channel %v", h.rpc(5*time.Second), h.cc.GetState())
		counts[line]++
		h.cc.Close()
	}
	for k, c := range counts {
		t.Logf("%d of %d rounds: %s", c, rounds, k)
	}
}

// TestM1DocumentedWayPickFirst gives the default pick_first policy an update
// with no addresses and no endpoints, the case its own code handles.
func TestM1DocumentedWayPickFirst(t *testing.T) {
	rounds := m1Env("M1_ROUNDS", 50)
	counts := map[string]int{}
	for i := 0; i < rounds; i++ {
		h := newM1Harness(t, "", connectivity.Ready, false)
		line := "first RPC " + h.rpc(5*time.Second)
		target := "m1:///" + h.id
		h.r.UpdateState(resolver.State{Addresses: []resolver.Address{}})
		time.Sleep(m1Quiet)
		code := h.rpc(m1ShortRPC)
		line += fmt.Sprintf(" | after an update with no addresses: sub-channels %s, channel %v, RPC with %v deadline: %s",
			m1PickFirstSubChannels(target), h.cc.GetState(), m1ShortRPC, code)
		if code == codes.OK.String() {
			t.Errorf("RPC succeeded with no addresses")
		}
		h.r.UpdateState(resolver.State{Addresses: []resolver.Address{{Addr: h.serverAddr}}})
		line += fmt.Sprintf(" | addresses put back: RPC %s, sub-channels %s, channel %v", h.rpc(5*time.Second), m1PickFirstSubChannels(target), h.cc.GetState())
		counts[line]++
		h.cc.Close()
	}
	for k, c := range counts {
		t.Logf("%d of %d rounds: %s", c, rounds, k)
	}
}
