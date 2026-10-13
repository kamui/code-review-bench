/*
 * Probe for group M1 of m-grpc-go-7390: SubConn.UpdateAddresses with an empty
 * address list. Copied into the root of a grpc-go checkout and run with
 * `go test -run 'TestM1' .`. It uses only the repository's own test helpers
 * (stub balancer, manual resolver, stub server) and its public balancer API.
 *
 * Environment:
 *   M1_ROUNDS   iterations per case (default 50)
 *   M1_WORKERS  iterations running at once (default 8)
 *   M1_HOLD     how long TestM1LongHold watches after the empty update (default 25s)
 */

package grpc_test

import (
	"context"
	"fmt"
	"net"
	"os"
	"sort"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/balancer"
	"google.golang.org/grpc/balancer/base"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/connectivity"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/internal/balancer/stub"
	"google.golang.org/grpc/internal/channelz"
	"google.golang.org/grpc/internal/stubserver"
	testgrpc "google.golang.org/grpc/interop/grpc_testing"
	testpb "google.golang.org/grpc/interop/grpc_testing"
	"google.golang.org/grpc/resolver"
	"google.golang.org/grpc/resolver/manual"
	"google.golang.org/grpc/status"
)

const (
	m1Quiet      = 300 * time.Millisecond
	m1ShortRPC   = 500 * time.Millisecond
	m1WaitingRPC = 5 * time.Second
)

var (
	m1Harnesses sync.Map
	m1NextID    atomic.Int64
)

func init() {
	channelz.TurnOn()
	for name, reconnect := range map[string]bool{"m1_passive": false, "m1_reconnect": true} {
		reconnect := reconnect
		stub.Register(name, stub.BalancerFuncs{
			Init: func(d *stub.BalancerData) {
				h, _ := m1Harnesses.Load(d.BuildOptions.Target.Endpoint())
				d.Data = h
			},
			UpdateClientConnState: func(d *stub.BalancerData, ccs balancer.ClientConnState) error {
				return d.Data.(*m1Harness).onResolverUpdate(d.ClientConn, ccs.ResolverState.Addresses, reconnect)
			},
		})
	}
}

type m1Picker struct{ sc balancer.SubConn }

func (p *m1Picker) Pick(balancer.PickInfo) (balancer.PickResult, error) {
	return balancer.PickResult{SubConn: p.sc}, nil
}

// m1Harness is one client channel with a one-SubConn balancer, a real server,
// a listener that accepts and never answers, and a port nothing listens on.
type m1Harness struct {
	t  *testing.T
	id string

	passNil         bool
	connectAtCreate bool

	mu     sync.Mutex
	sc     balancer.SubConn
	bcc    balancer.ClientConn
	told   []connectivity.State
	toldAt []time.Time

	created    chan struct{}
	updated    chan int
	resolveNow atomic.Int64
	holeConns  atomic.Int64

	serverAddr  string
	holeAddr    string
	refusedAddr string

	r      *manual.Resolver
	cc     *grpc.ClientConn
	client testgrpc.TestServiceClient
}

func (h *m1Harness) onResolverUpdate(cc balancer.ClientConn, addrs []resolver.Address, reconnect bool) error {
	h.mu.Lock()
	sc := h.sc
	h.mu.Unlock()
	if sc == nil {
		var newSC balancer.SubConn
		newSC, err := cc.NewSubConn(addrs, balancer.NewSubConnOptions{
			StateListener: func(s balancer.SubConnState) { h.onState(cc, newSC, s, reconnect) },
		})
		if err != nil {
			h.t.Errorf("NewSubConn: %v", err)
			return nil
		}
		h.mu.Lock()
		h.sc, h.bcc = newSC, cc
		h.mu.Unlock()
		cc.UpdateState(balancer.State{ConnectivityState: connectivity.Connecting, Picker: base.NewErrPicker(balancer.ErrNoSubConnAvailable)})
		if h.connectAtCreate {
			newSC.Connect()
		}
		h.created <- struct{}{}
		return nil
	}
	if len(addrs) == 0 {
		if h.passNil {
			addrs = nil
		} else {
			addrs = []resolver.Address{}
		}
	}
	cc.UpdateAddresses(sc, addrs)
	h.updated <- len(addrs)
	return nil
}

// onState is the SubConn's StateListener. It records every state it is told and
// publishes the picker a one-SubConn policy such as pick_first publishes for it.
func (h *m1Harness) onState(cc balancer.ClientConn, sc balancer.SubConn, s balancer.SubConnState, reconnect bool) {
	h.mu.Lock()
	h.told = append(h.told, s.ConnectivityState)
	h.toldAt = append(h.toldAt, time.Now())
	h.mu.Unlock()
	switch s.ConnectivityState {
	case connectivity.Ready:
		cc.UpdateState(balancer.State{ConnectivityState: connectivity.Ready, Picker: &m1Picker{sc: sc}})
	case connectivity.Connecting:
		cc.UpdateState(balancer.State{ConnectivityState: connectivity.Connecting, Picker: base.NewErrPicker(balancer.ErrNoSubConnAvailable)})
	case connectivity.Idle:
		cc.UpdateState(balancer.State{ConnectivityState: connectivity.Idle, Picker: base.NewErrPicker(balancer.ErrNoSubConnAvailable)})
		if reconnect {
			sc.Connect()
		}
	case connectivity.TransientFailure:
		cc.UpdateState(balancer.State{ConnectivityState: connectivity.TransientFailure, Picker: base.NewErrPicker(fmt.Errorf("sub-connection failed: %v", s.ConnectionError))})
	}
}

func (h *m1Harness) count() int {
	h.mu.Lock()
	defer h.mu.Unlock()
	return len(h.told)
}

func (h *m1Harness) since(n int) []connectivity.State {
	h.mu.Lock()
	defer h.mu.Unlock()
	return append([]connectivity.State(nil), h.told[n:]...)
}

func (h *m1Harness) last() connectivity.State {
	h.mu.Lock()
	defer h.mu.Unlock()
	if len(h.told) == 0 {
		return connectivity.Idle
	}
	return h.told[len(h.told)-1]
}

// settle waits until the balancer has been told nothing for `quiet`, and
// returns the states it was told from index n on.
func (h *m1Harness) settle(n int, quiet time.Duration) []connectivity.State {
	start := time.Now()
	for {
		h.mu.Lock()
		mark := start
		if len(h.toldAt) > n {
			mark = h.toldAt[len(h.toldAt)-1]
		}
		h.mu.Unlock()
		if time.Since(mark) >= quiet && time.Since(start) >= quiet {
			return h.since(n)
		}
		if time.Since(start) > 10*time.Second {
			return h.since(n)
		}
		time.Sleep(5 * time.Millisecond)
	}
}

func (h *m1Harness) waitTold(n int, want connectivity.State, d time.Duration) bool {
	deadline := time.Now().Add(d)
	for time.Now().Before(deadline) {
		for _, s := range h.since(n) {
			if s == want {
				return true
			}
		}
		time.Sleep(2 * time.Millisecond)
	}
	return false
}

// internalState reads the sub-channel's own state and socket count from channelz.
func (h *m1Harness) internalState() string {
	h.mu.Lock()
	sc := h.sc
	h.mu.Unlock()
	text := fmt.Sprint(sc)
	id, err := strconv.ParseInt(strings.TrimSuffix(strings.TrimPrefix(text, "SubConn(id:"), ")"), 10, 64)
	if err != nil {
		return "unknown(" + text + ")"
	}
	scm := channelz.GetSubChannel(id)
	if scm == nil {
		return "unknown(no channelz entry)"
	}
	state := "IDLE(never changed)"
	if s := scm.ChannelMetrics.State.Load(); s != nil {
		state = s.String()
	}
	return fmt.Sprintf("%s/sockets=%d", state, len(scm.Sockets()))
}

// gap returns the time between the balancer being told IDLE and being told
// CONNECTING, when those are the two states told from index n on.
func (h *m1Harness) gap(n int) (time.Duration, bool) {
	h.mu.Lock()
	defer h.mu.Unlock()
	if len(h.told) < n+2 || h.told[n] != connectivity.Idle || h.told[n+1] != connectivity.Connecting {
		return 0, false
	}
	return h.toldAt[n+1].Sub(h.toldAt[n]), true
}

func (h *m1Harness) rpc(timeout time.Duration) string {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	_, err := h.client.EmptyCall(ctx, &testpb.Empty{})
	return status.Code(err).String()
}

func newM1Harness(t *testing.T, lb string, start connectivity.State, passNil bool) *m1Harness {
	h := &m1Harness{
		t:               t,
		id:              fmt.Sprintf("m1-%d", m1NextID.Add(1)),
		passNil:         passNil,
		connectAtCreate: start != connectivity.Idle,
		created:         make(chan struct{}, 1),
		updated:         make(chan int, 16),
	}
	m1Harnesses.Store(h.id, h)

	lis, err := net.Listen("tcp", "localhost:0")
	if err != nil {
		t.Fatal(err)
	}
	server := grpc.NewServer()
	testgrpc.RegisterTestServiceServer(server, &stubserver.StubServer{
		EmptyCallF: func(context.Context, *testpb.Empty) (*testpb.Empty, error) { return &testpb.Empty{}, nil },
	})
	go server.Serve(lis)
	h.serverAddr = lis.Addr().String()

	hole, err := net.Listen("tcp", "localhost:0")
	if err != nil {
		t.Fatal(err)
	}
	h.holeAddr = hole.Addr().String()
	var held []net.Conn
	var heldMu sync.Mutex
	go func() {
		for {
			c, err := hole.Accept()
			if err != nil {
				return
			}
			heldMu.Lock()
			held = append(held, c)
			heldMu.Unlock()
			h.holeConns.Add(1)
		}
	}()

	refused, err := net.Listen("tcp", "localhost:0")
	if err != nil {
		t.Fatal(err)
	}
	h.refusedAddr = refused.Addr().String()
	refused.Close()

	first := h.serverAddr
	switch start {
	case connectivity.Connecting:
		first = h.holeAddr
	case connectivity.TransientFailure:
		first = h.refusedAddr
	}
	h.r = manual.NewBuilderWithScheme("m1")
	h.r.ResolveNowCallback = func(resolver.ResolveNowOptions) { h.resolveNow.Add(1) }
	h.r.InitialState(resolver.State{Addresses: []resolver.Address{{Addr: first}}})
	opts := []grpc.DialOption{grpc.WithTransportCredentials(insecure.NewCredentials()), grpc.WithResolvers(h.r)}
	if lb != "" {
		opts = append(opts, grpc.WithDefaultServiceConfig(fmt.Sprintf(`{"loadBalancingConfig":[{"%s":{}}]}`, lb)))
	}
	h.cc, err = grpc.NewClient("m1:///"+h.id, opts...)
	if err != nil {
		t.Fatal(err)
	}
	h.client = testgrpc.NewTestServiceClient(h.cc)
	h.cc.Connect()
	t.Cleanup(func() {
		h.cc.Close()
		server.Stop()
		hole.Close()
		heldMu.Lock()
		for _, c := range held {
			c.Close()
		}
		heldMu.Unlock()
		m1Harnesses.Delete(h.id)
	})
	return h
}

// reach brings the SubConn to the wanted starting state and reports whether it got there.
func (h *m1Harness) reach(start connectivity.State) error {
	select {
	case <-h.created:
	case <-time.After(10 * time.Second):
		return fmt.Errorf("balancer never created its SubConn")
	}
	switch start {
	case connectivity.Ready:
		if !h.waitTold(0, connectivity.Ready, 10*time.Second) {
			return fmt.Errorf("never READY; told %v", h.since(0))
		}
		if code := h.rpc(5 * time.Second); code != codes.OK.String() {
			return fmt.Errorf("first RPC: %s", code)
		}
	case connectivity.Connecting:
		if !h.waitTold(0, connectivity.Connecting, 10*time.Second) {
			return fmt.Errorf("never CONNECTING; told %v", h.since(0))
		}
		deadline := time.Now().Add(10 * time.Second)
		for h.holeConns.Load() == 0 && time.Now().Before(deadline) {
			time.Sleep(2 * time.Millisecond)
		}
		if h.holeConns.Load() == 0 {
			return fmt.Errorf("no connection attempt reached the silent listener")
		}
	case connectivity.TransientFailure:
		if !h.waitTold(0, connectivity.TransientFailure, 10*time.Second) {
			return fmt.Errorf("never TRANSIENT_FAILURE; told %v", h.since(0))
		}
	case connectivity.Idle:
		// The SubConn was created and Connect() was never called.
	}
	if got := h.last(); start != connectivity.Idle && got != start {
		return fmt.Errorf("start state is %v, want %v; told %v", got, start, h.since(0))
	}
	return nil
}

// oneRound runs the whole case once and returns one line describing it.
func m1OneRound(t *testing.T, lb string, start connectivity.State, passNil bool, quiet time.Duration) (string, time.Duration) {
	h := newM1Harness(t, lb, start, passNil)
	defer h.cc.Close()
	if err := h.reach(start); err != nil {
		return "SETUP FAILED: " + err.Error(), -1
	}

	n := h.count()
	resolves := h.resolveNow.Load()
	h.r.UpdateState(resolver.State{Addresses: []resolver.Address{}})
	select {
	case <-h.updated:
	case <-time.After(10 * time.Second):
		return "UpdateAddresses(empty) did not return within 10s", -1
	}
	afterEmpty := h.settle(n, quiet)
	gap, ok := h.gap(n)
	if !ok {
		gap = -1
	}
	line := fmt.Sprintf("after UpdateAddresses(empty): told %v, last told %v, sub-channel itself %s, re-resolutions requested %d",
		afterEmpty, h.last(), h.internalState(), h.resolveNow.Load()-resolves)

	n = h.count()
	h.mu.Lock()
	sc := h.sc
	h.mu.Unlock()
	sc.Connect()
	line += fmt.Sprintf(" | then Connect(): told %v, sub-channel itself %s", h.settle(n, m1Quiet), h.internalState())

	line += fmt.Sprintf(" | RPC with %v deadline: %s, channel %v", m1ShortRPC, h.rpc(m1ShortRPC), h.cc.GetState())

	waiting := make(chan string, 1)
	go func() { waiting <- h.rpc(m1WaitingRPC) }()
	time.Sleep(100 * time.Millisecond)
	n = h.count()
	h.r.UpdateState(resolver.State{Addresses: []resolver.Address{{Addr: h.serverAddr}}})
	select {
	case <-h.updated:
	case <-time.After(10 * time.Second):
		return line + " | UpdateAddresses(server) did not return within 10s", gap
	}
	h.waitTold(n, connectivity.Ready, 3*time.Second)
	line += fmt.Sprintf(" | addresses put back: told %v, waiting RPC %s, new RPC %s", h.settle(n, m1Quiet), <-waiting, h.rpc(2*time.Second))
	return line, gap
}

func m1Env(name string, fallback int) int {
	if v, err := strconv.Atoi(os.Getenv(name)); err == nil && v > 0 {
		return v
	}
	return fallback
}

// m1Tally runs a case M1_ROUNDS times, M1_WORKERS at once, and logs each distinct outcome with its count.
func m1Tally(t *testing.T, lb string, start connectivity.State, passNil bool, quiet time.Duration) {
	rounds, workers := m1Env("M1_ROUNDS", 50), m1Env("M1_WORKERS", 8)
	lines := make([]string, rounds)
	gaps := make([]time.Duration, rounds)
	var wg sync.WaitGroup
	next := atomic.Int64{}
	for w := 0; w < workers; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for {
				i := int(next.Add(1)) - 1
				if i >= rounds {
					return
				}
				lines[i], gaps[i] = m1OneRound(t, lb, start, passNil, quiet)
			}
		}()
	}
	wg.Wait()
	counts := map[string]int{}
	for _, l := range lines {
		counts[l]++
	}
	keys := make([]string, 0, len(counts))
	for k := range counts {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		t.Logf("%d of %d rounds: %s", counts[k], rounds, k)
	}
	var measured []time.Duration
	for _, g := range gaps {
		if g >= 0 {
			measured = append(measured, g)
		}
	}
	if len(measured) > 0 {
		sort.Slice(measured, func(i, j int) bool { return measured[i] < measured[j] })
		t.Logf("time from told IDLE to told CONNECTING in %d rounds: shortest %v, median %v, longest %v",
			len(measured), measured[0], measured[len(measured)/2], measured[len(measured)-1])
	}
}

func m1Variants(t *testing.T, start connectivity.State, quiet time.Duration, lbs ...string) {
	for _, lb := range lbs {
		for _, passNil := range []bool{false, true} {
			arg := "empty-slice"
			if passNil {
				arg = "nil"
			}
			t.Run(lb+"/"+arg, func(t *testing.T) { m1Tally(t, lb, start, passNil, quiet) })
		}
	}
}

// The balancer m1_passive never calls Connect() on IDLE; m1_reconnect calls it
// at once, as the SubConn documentation tells balancers to.

func TestM1Ready(t *testing.T) {
	m1Variants(t, connectivity.Ready, m1Quiet, "m1_passive", "m1_reconnect")
}

func TestM1Connecting(t *testing.T) {
	m1Variants(t, connectivity.Connecting, m1Quiet, "m1_passive", "m1_reconnect")
}

// Neighbours: the SubConn is IDLE (never connected) or in TRANSIENT_FAILURE
// (backing off) when the empty list arrives.
func TestM1Idle(t *testing.T) {
	m1Variants(t, connectivity.Idle, m1Quiet, "m1_passive")
}

func TestM1TransientFailure(t *testing.T) {
	m1Variants(t, connectivity.TransientFailure, 1600*time.Millisecond, "m1_passive", "m1_reconnect")
}

// TestM1ControlNonEmptyWhileConnecting replaces the address of a CONNECTING
// SubConn with the real server's, to show what an ordinary address change does
// to the re-resolution count and the states told.
func TestM1ControlNonEmptyWhileConnecting(t *testing.T) {
	rounds := m1Env("M1_ROUNDS", 50)
	if rounds > 50 {
		rounds = 50
	}
	counts := map[string]int{}
	for i := 0; i < rounds; i++ {
		h := newM1Harness(t, "m1_passive", connectivity.Connecting, false)
		if err := h.reach(connectivity.Connecting); err != nil {
			t.Fatal(err)
		}
		n, resolves := h.count(), h.resolveNow.Load()
		h.r.UpdateState(resolver.State{Addresses: []resolver.Address{{Addr: h.serverAddr}}})
		<-h.updated
		h.waitTold(n, connectivity.Ready, 3*time.Second)
		counts[fmt.Sprintf("after UpdateAddresses(server address): told %v, sub-channel itself %s, re-resolutions requested %d, RPC %s",
			h.settle(n, m1Quiet), h.internalState(), h.resolveNow.Load()-resolves, h.rpc(2*time.Second))]++
		h.cc.Close()
	}
	for k, c := range counts {
		t.Logf("%d of %d rounds: %s", c, rounds, k)
	}
}

// TestM1LongHold watches one SubConn for M1_HOLD seconds after the empty
// update, longer than the 20-second minimum connect timeout.
func TestM1LongHold(t *testing.T) {
	hold := time.Duration(m1Env("M1_HOLD", 25)) * time.Second
	for _, start := range []connectivity.State{connectivity.Ready, connectivity.Connecting} {
		start := start
		t.Run(start.String(), func(t *testing.T) {
			t.Parallel()
			h := newM1Harness(t, "m1_passive", start, false)
			if err := h.reach(start); err != nil {
				t.Fatal(err)
			}
			n := h.count()
			h.r.UpdateState(resolver.State{Addresses: []resolver.Address{}})
			<-h.updated
			first := h.settle(n, m1Quiet)
			n = h.count()
			time.Sleep(hold)
			t.Logf("after UpdateAddresses(empty): told %v; in the next %v: told %v; sub-channel itself %s; channel %v",
				first, hold, h.since(n), h.internalState(), h.cc.GetState())
		})
	}
}

// TestM1NewSubConnEmpty records what NewSubConn does with an empty list.
func TestM1NewSubConnEmpty(t *testing.T) {
	h := newM1Harness(t, "m1_passive", connectivity.Ready, false)
	if err := h.reach(connectivity.Ready); err != nil {
		t.Fatal(err)
	}
	h.mu.Lock()
	bcc := h.bcc
	h.mu.Unlock()
	for _, addrs := range [][]resolver.Address{nil, {}} {
		sc, err := bcc.NewSubConn(addrs, balancer.NewSubConnOptions{StateListener: func(balancer.SubConnState) {}})
		t.Logf("NewSubConn(%#v): SubConn=%v, error=%v", addrs, sc, err)
	}
}

// TestM1PickFirst uses the default pick_first policy. A resolver update that
// holds one endpoint with no addresses reaches pick_first's UpdateAddresses call
// with an empty list.
func TestM1PickFirst(t *testing.T) {
	rounds := m1Env("M1_ROUNDS", 50)
	counts := map[string]int{}
	var mu sync.Mutex
	var wg sync.WaitGroup
	sem := make(chan struct{}, m1Env("M1_WORKERS", 8))
	for i := 0; i < rounds; i++ {
		wg.Add(1)
		sem <- struct{}{}
		go func() {
			defer wg.Done()
			defer func() { <-sem }()
			line := m1PickFirstRound(t)
			mu.Lock()
			counts[line]++
			mu.Unlock()
		}()
	}
	wg.Wait()
	keys := make([]string, 0, len(counts))
	for k := range counts {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		t.Logf("%d of %d rounds: %s", counts[k], rounds, k)
	}
}

func m1PickFirstSubChannels(target string) string {
	chans, _ := channelz.GetTopChannels(0, 0)
	for _, c := range chans {
		if tp := c.ChannelMetrics.Target.Load(); tp == nil || *tp != target {
			continue
		}
		var parts []string
		for id := range c.SubChans() {
			scm := channelz.GetSubChannel(id)
			if scm == nil {
				continue
			}
			state := "nil"
			if s := scm.ChannelMetrics.State.Load(); s != nil {
				state = s.String()
			}
			parts = append(parts, fmt.Sprintf("%s/sockets=%d", state, len(scm.Sockets())))
		}
		sort.Strings(parts)
		return fmt.Sprintf("%v", parts)
	}
	return "channel not found in channelz"
}

func m1PickFirstRound(t *testing.T) string {
	h := newM1Harness(t, "", connectivity.Ready, false)
	defer h.cc.Close()
	if code := h.rpc(5 * time.Second); code != codes.OK.String() {
		return "SETUP FAILED: first RPC " + code
	}
	target := "m1:///" + h.id
	before := m1PickFirstSubChannels(target)

	var seen []connectivity.State
	var seenMu sync.Mutex
	watchCtx, stopWatch := context.WithCancel(context.Background())
	defer stopWatch()
	go func() {
		s := h.cc.GetState()
		for h.cc.WaitForStateChange(watchCtx, s) {
			s = h.cc.GetState()
			seenMu.Lock()
			seen = append(seen, s)
			seenMu.Unlock()
		}
	}()

	resolves := h.resolveNow.Load()
	h.r.UpdateState(resolver.State{Endpoints: []resolver.Endpoint{{}}})
	time.Sleep(m1Quiet)
	line := fmt.Sprintf("sub-channels before %s; after an update of one endpoint with no addresses: sub-channels %s, channel %v, re-resolutions requested %d",
		before, m1PickFirstSubChannels(target), h.cc.GetState(), h.resolveNow.Load()-resolves)
	line += fmt.Sprintf(" | RPC with %v deadline: %s", m1ShortRPC, h.rpc(m1ShortRPC))
	waiting := make(chan string, 1)
	go func() { waiting <- h.rpc(m1WaitingRPC) }()
	time.Sleep(100 * time.Millisecond)
	h.r.UpdateState(resolver.State{Addresses: []resolver.Address{{Addr: h.serverAddr}}})
	line += fmt.Sprintf(" | addresses put back: waiting RPC %s, new RPC %s, sub-channels %s", <-waiting, h.rpc(2*time.Second), m1PickFirstSubChannels(target))
	seenMu.Lock()
	// The watcher can miss a state that is replaced before it reads it, so only the last state is reported.
	last := "none"
	if len(seen) > 0 {
		last = seen[len(seen)-1].String()
	}
	seenMu.Unlock()
	return line + ", channel last seen " + last
}
