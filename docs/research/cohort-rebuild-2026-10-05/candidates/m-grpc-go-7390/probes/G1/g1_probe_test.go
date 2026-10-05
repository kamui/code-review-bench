package grpc

// Probe for candidate G1 on grpc/grpc-go#7390. Copy this file into the root of a grpc-go checkout
// (it is an in-package test, so it can reach addrConn.mu and the unexported withBackoff option) and
// run it with run.sh at the commit before the change and at its head.
//
// Every test moves a sub-connection to a different address through the default pick_first policy,
// which calls ClientConn.UpdateAddresses and so addrConn.updateAddrs.

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net"
	"os"
	"runtime"
	"runtime/pprof"
	"sort"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"google.golang.org/grpc/balancer"
	"google.golang.org/grpc/connectivity"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/resolver"
	"google.golang.org/grpc/resolver/manual"
	"google.golang.org/grpc/status"
)

type g1Codec struct{}

func (g1Codec) Marshal(v any) ([]byte, error) { return *(v.(*[]byte)), nil }
func (g1Codec) Unmarshal(d []byte, v any) error {
	*(v.(*[]byte)) = append([]byte(nil), d...)
	return nil
}
func (g1Codec) Name() string { return "g1raw" }

func g1Echo(_ any, stream ServerStream) error {
	for {
		var b []byte
		if err := stream.RecvMsg(&b); err == io.EOF {
			return nil
		} else if err != nil {
			return err
		}
		if err := stream.SendMsg(&b); err != nil {
			return err
		}
	}
}

// g1TimingCC times each UpdateAddresses call that pick_first makes.
type g1TimingCC struct {
	balancer.ClientConn
	record func(time.Duration)
}

func (c *g1TimingCC) UpdateAddresses(sc balancer.SubConn, addrs []resolver.Address) {
	start := time.Now()
	c.ClientConn.UpdateAddresses(sc, addrs)
	c.record(time.Since(start))
}

type g1Builder struct {
	name   string
	record func(time.Duration)
}

func (b g1Builder) Name() string { return b.name }
func (b g1Builder) Build(cc balancer.ClientConn, o balancer.BuildOptions) balancer.Balancer {
	return balancer.Get("pick_first").Build(&g1TimingCC{ClientConn: cc, record: b.record}, o)
}

// g1SlowBackoff sleeps inside Backoff when armed. resetTransport calls Backoff while it holds ac.mu,
// at both commits, so an armed call stretches the locked start of a connection attempt.
type g1SlowBackoff struct {
	hold    time.Duration
	armed   atomic.Bool
	entered chan struct{}
}

func (b *g1SlowBackoff) Backoff(int) time.Duration {
	if b.armed.CompareAndSwap(true, false) {
		b.entered <- struct{}{}
		time.Sleep(b.hold)
	}
	return time.Second
}

type g1Env struct {
	t     *testing.T
	cc    *ClientConn
	r     *manual.Resolver
	addrs []string

	mu        sync.Mutex
	durations []time.Duration
}

var g1Seq atomic.Int32

func g1EnvInt(name string, def int) int {
	if v, err := strconv.Atoi(os.Getenv(name)); err == nil {
		return v
	}
	return def
}

// g1Blackhole accepts connections and never answers, so a sub-connection stays CONNECTING.
func g1Blackhole(t *testing.T) string {
	lis, err := net.Listen("tcp", "localhost:0")
	if err != nil {
		t.Fatal(err)
	}
	var mu sync.Mutex
	var conns []net.Conn
	t.Cleanup(func() {
		lis.Close()
		mu.Lock()
		defer mu.Unlock()
		for _, c := range conns {
			c.Close()
		}
	})
	go func() {
		for {
			c, err := lis.Accept()
			if err != nil {
				return
			}
			mu.Lock()
			conns = append(conns, c)
			mu.Unlock()
		}
	}()
	return lis.Addr().String()
}

func g1EchoServer(t *testing.T) string {
	lis, err := net.Listen("tcp", "localhost:0")
	if err != nil {
		t.Fatal(err)
	}
	server := NewServer(UnknownServiceHandler(g1Echo), ForceServerCodec(g1Codec{}))
	go server.Serve(lis)
	t.Cleanup(server.Stop)
	return lis.Addr().String()
}

func g1NewEnv(t *testing.T, addrs []string, extra ...DialOption) *g1Env {
	e := &g1Env{t: t, addrs: addrs}
	name := fmt.Sprintf("g1probe%d", g1Seq.Add(1))
	balancer.Register(g1Builder{name: name, record: func(d time.Duration) {
		e.mu.Lock()
		e.durations = append(e.durations, d)
		e.mu.Unlock()
	}})
	e.r = manual.NewBuilderWithScheme(name)
	e.r.InitialState(resolver.State{Addresses: []resolver.Address{{Addr: addrs[0]}}})
	opts := append([]DialOption{
		WithTransportCredentials(insecure.NewCredentials()),
		WithResolvers(e.r),
		WithDefaultServiceConfig(fmt.Sprintf(`{"loadBalancingConfig":[{%q:{}}]}`, name)),
	}, extra...)
	cc, err := NewClient(e.r.Scheme()+":///x", opts...)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { cc.Close() })
	e.cc = cc
	cc.Connect()
	return e
}

func (e *g1Env) addrConn() *addrConn {
	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		e.cc.mu.Lock()
		for ac := range e.cc.conns {
			e.cc.mu.Unlock()
			return ac
		}
		e.cc.mu.Unlock()
		time.Sleep(time.Millisecond)
	}
	e.t.Fatal("no sub-connection was created within 10s")
	return nil
}

// waitState polls two atomics that addrConn publishes for channelz, so the wait itself never takes
// ac.mu and does not add contention to what is being measured.
func (e *g1Env) waitState(ac *addrConn, want connectivity.State, addr string) {
	deadline := time.Now().Add(30 * time.Second)
	for time.Now().Before(deadline) {
		s, target := ac.channelz.ChannelMetrics.State.Load(), ac.channelz.ChannelMetrics.Target.Load()
		if s != nil && *s == want && target != nil && *target == addr {
			return
		}
		time.Sleep(20 * time.Microsecond)
	}
	buf := make([]byte, 1<<20)
	e.t.Fatalf("sub-connection did not reach %v on %s within 30s; goroutines:\n%s", want, addr, buf[:runtime.Stack(buf, true)])
}

func (e *g1Env) switchTo(i int) {
	e.r.UpdateState(resolver.State{Addresses: []resolver.Address{{Addr: e.addrs[i]}}})
}

func (e *g1Env) takeDurations() []time.Duration {
	e.mu.Lock()
	defer e.mu.Unlock()
	d := e.durations
	e.durations = nil
	return d
}

func g1Summary(d []time.Duration) string {
	if len(d) == 0 {
		return "n=0"
	}
	s := append([]time.Duration(nil), d...)
	sort.Slice(s, func(i, j int) bool { return s[i] < s[j] })
	at := func(q float64) time.Duration { return s[int(q*float64(len(s)-1))] }
	over := func(limit time.Duration) int {
		return len(s) - sort.Search(len(s), func(i int) bool { return s[i] > limit })
	}
	return fmt.Sprintf("n=%d min=%v p50=%v p90=%v p99=%v max=%v over100us=%d over1ms=%d over10ms=%d",
		len(s), s[0], at(0.5), at(0.9), at(0.99), s[len(s)-1], over(100*time.Microsecond), over(time.Millisecond), over(10*time.Millisecond))
}

func g1Spinners(t *testing.T) {
	n := g1EnvInt("G1_SPINNERS", 0)
	var stop atomic.Bool
	for i := 0; i < n; i++ {
		go func() {
			for !stop.Load() {
			}
		}()
	}
	t.Cleanup(func() { stop.Store(true) })
	t.Logf("GOMAXPROCS=%d NumCPU=%d busy goroutines=%d", runtime.GOMAXPROCS(0), runtime.NumCPU(), n)
}

// g1Parked reads the block profile and returns, for the entries whose stack passes through every
// named function, how many times a goroutine parked there, for how long in total, and the stack.
func g1Parked(t *testing.T, needles ...string) (int64, time.Duration, string) {
	var buf bytes.Buffer
	if err := pprof.Lookup("block").WriteTo(&buf, 1); err != nil {
		t.Fatal(err)
	}
	cyclesPerSecond := 1e9
	var count, cycles int64
	var sample string
	for _, block := range strings.Split(buf.String(), "\n\n") {
		for _, line := range strings.Split(block, "\n") {
			if rest, ok := strings.CutPrefix(line, "cycles/second="); ok {
				if v, err := strconv.ParseFloat(rest, 64); err == nil {
					cyclesPerSecond = v
				}
			}
		}
		matches := true
		for _, n := range needles {
			matches = matches && strings.Contains(block, n)
		}
		if !matches {
			continue
		}
		for _, line := range strings.Split(block, "\n") {
			var c, n int64
			if _, err := fmt.Sscanf(line, "%d %d @", &c, &n); err == nil {
				cycles += c
				count += n
				if sample == "" {
					sample = block
				}
			}
		}
	}
	var frames []string
	for _, line := range strings.Split(sample, "\n") {
		if fields := strings.Fields(line); len(fields) >= 4 && fields[0] == "#" {
			name := fields[2]
			if i := strings.LastIndex(name, "+0x"); i > 0 {
				name = name[:i]
			}
			frames = append(frames, name)
		}
	}
	return count, time.Duration(float64(cycles) / cyclesPerSecond * 1e9), strings.Join(frames, " <- ")
}

func g1ParkedSummary(t *testing.T, needles ...string) string {
	count, total, stack := g1Parked(t, needles...)
	if count == 0 {
		return "parked 0 times"
	}
	return fmt.Sprintf("parked %d times, %v in total, %v on average; stack, innermost first: %s", count, total, total/time.Duration(count), stack)
}

// TestG1Natural switches a READY sub-connection between two servers and reports how long each
// UpdateAddresses call took, and how often and how long the caller parked on ac.mu inside the
// deferred GracefulClose. Nothing is slowed down artificially.
func TestG1Natural(t *testing.T) {
	g1Spinners(t)
	rounds := g1EnvInt("G1_ROUNDS", 2000)
	e := g1NewEnv(t, []string{g1EchoServer(t), g1EchoServer(t)})
	ac := e.addrConn()
	e.waitState(ac, connectivity.Ready, e.addrs[0])
	runtime.SetBlockProfileRate(1)
	defer runtime.SetBlockProfileRate(0)
	start := time.Now()
	onClose := []string{"updateAddrs", "GracefulClose", "createTransport.func1", "Mutex"}
	var perSwitch []time.Duration
	var before time.Duration
	for i := 0; i < rounds; i++ {
		e.switchTo((i + 1) % 2)
		e.waitState(ac, connectivity.Ready, e.addrs[(i+1)%2])
		_, total, _ := g1Parked(t, onClose...)
		perSwitch = append(perSwitch, total-before)
		before = total
	}
	t.Logf("%d switches of a READY sub-connection in %v, none stuck", rounds, time.Since(start).Round(time.Millisecond))
	t.Logf("time inside each UpdateAddresses call: %s", g1Summary(e.takeDurations()))
	t.Logf("caller parked on ac.mu under updateAddrs -> GracefulClose -> onClose: %s", g1ParkedSummary(t, onClose...))
	t.Logf("that park, per switch (0s when the caller did not park): %s", g1Summary(perSwitch))
	t.Logf("for comparison, caller parked under updateAddrs -> GracefulClose -> Close, waiting for the old connection's writer to finish: %s", g1ParkedSummary(t, "updateAddrs", "GracefulClose", "http2Client).Close"))
}

// TestG1Widened stretches the locked start of the new connection attempt to `hold` and, while it
// lasts, times four things: the UpdateAddresses call, a call that needs the old connection's own
// lock (t.mu), a call that needs ac.mu, and one message round trip on an RPC stream that was already
// open on the old connection.
func TestG1Widened(t *testing.T) {
	const hold = 300 * time.Millisecond
	const settle = 30 * time.Millisecond
	sb := &g1SlowBackoff{hold: hold, entered: make(chan struct{}, 1)}
	e := g1NewEnv(t, []string{g1EchoServer(t), g1EchoServer(t)}, withBackoff(sb))
	ac := e.addrConn()
	e.waitState(ac, connectivity.Ready, e.addrs[0])
	e.takeDurations()
	t.Logf("locked start of the new attempt stretched to %v; the three probes start %v after it begins", hold, settle)
	for round := 0; round < g1EnvInt("G1_ROUNDS", 5); round++ {
		next := (round + 1) % 2
		ac.mu.Lock()
		oldTransport := ac.transport
		ac.mu.Unlock()
		if oldTransport == nil {
			t.Fatal("READY sub-connection has no transport")
		}
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		stream, err := e.cc.NewStream(ctx, &StreamDesc{ClientStreams: true, ServerStreams: true}, "/g1/Echo", ForceCodec(g1Codec{}))
		if err != nil {
			t.Fatal(err)
		}
		ping := func() error {
			in, out := []byte("ping"), []byte(nil)
			if err := stream.SendMsg(&in); err != nil {
				return err
			}
			return stream.RecvMsg(&out)
		}
		if err := ping(); err != nil {
			t.Fatalf("round %d: first ping on the old connection: %v", round, err)
		}

		sb.armed.Store(true)
		go e.switchTo(next)
		select {
		case <-sb.entered:
		case <-time.After(10 * time.Second):
			t.Fatal("the new connection attempt did not start within 10s")
		}
		time.Sleep(settle)

		var wg sync.WaitGroup
		var transportLock, addrConnLock, roundTrip time.Duration
		var pingErr error
		timed := func(d *time.Duration, f func()) {
			wg.Add(1)
			go func() {
				defer wg.Done()
				start := time.Now()
				f()
				*d = time.Since(start)
			}()
		}
		timed(&transportLock, func() { oldTransport.GetGoAwayReason() })
		timed(&addrConnLock, func() { ac.getReadyTransport() })
		timed(&roundTrip, func() { pingErr = ping() })
		wg.Wait()
		e.waitState(ac, connectivity.Ready, e.addrs[next])
		calls := e.takeDurations()
		if len(calls) != 1 {
			t.Fatalf("round %d: expected one UpdateAddresses call, saw %d", round, len(calls))
		}
		t.Logf("round %d: UpdateAddresses call=%v | wait for old connection's lock (t.mu)=%v | wait for ac.mu=%v | round trip on the already-open stream=%v (err=%v)",
			round, calls[0].Round(time.Millisecond), transportLock.Round(time.Millisecond), addrConnLock.Round(time.Millisecond), roundTrip.Round(time.Millisecond), pingErr)
		stream.CloseSend()
		cancel()
	}
}

// TestG1UnderRPCLoad keeps unary RPCs running while the sub-connection is switched repeatedly, and
// reports how many completed, how many failed and how long they took.
func TestG1UnderRPCLoad(t *testing.T) {
	g1Spinners(t)
	rounds := g1EnvInt("G1_ROUNDS", 300)
	e := g1NewEnv(t, []string{g1EchoServer(t), g1EchoServer(t)})
	ac := e.addrConn()
	e.waitState(ac, connectivity.Ready, e.addrs[0])
	var stop atomic.Bool
	var wg sync.WaitGroup
	var mu sync.Mutex
	var latencies []time.Duration
	failures := map[string]int{}
	for w := 0; w < 8; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for !stop.Load() {
				ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
				in, out := []byte("ping"), []byte(nil)
				start := time.Now()
				err := e.cc.Invoke(ctx, "/g1/Echo", &in, &out, ForceCodec(g1Codec{}), WaitForReady(true))
				elapsed := time.Since(start)
				cancel()
				mu.Lock()
				if err != nil {
					failures[status.Code(err).String()]++
				} else {
					latencies = append(latencies, elapsed)
				}
				mu.Unlock()
			}
		}()
	}
	start := time.Now()
	for i := 0; i < rounds; i++ {
		e.switchTo((i + 1) % 2)
		e.waitState(ac, connectivity.Ready, e.addrs[(i+1)%2])
	}
	elapsed := time.Since(start)
	stop.Store(true)
	wg.Wait()
	t.Logf("%d switches in %v with 8 callers: %d RPCs succeeded, failures by code=%v", rounds, elapsed.Round(time.Millisecond), len(latencies), failures)
	t.Logf("RPC latency: %s", g1Summary(latencies))
	t.Logf("time inside each UpdateAddresses call: %s", g1Summary(e.takeDurations()))
}

// TestG1LockAfterReturn covers the case with no old connection to close: the sub-connection is still
// CONNECTING (to a server that never answers) when its address changes. It asks whether ac.mu is
// still locked at the instant updateAddrs returns, and how long the lock stays taken when the caller
// keeps running without ever yielding the processor.
func TestG1LockAfterReturn(t *testing.T) {
	g1Spinners(t)
	rounds := g1EnvInt("G1_ROUNDS", 500)
	e := g1NewEnv(t, []string{g1Blackhole(t), g1Blackhole(t)})
	ac := e.addrConn()
	e.waitState(ac, connectivity.Connecting, e.addrs[0])
	var heldOnReturn int
	var busyWaits []time.Duration
	for i := 0; i < rounds; i++ {
		next := (i + 1) % 2
		ac.updateAddrs([]resolver.Address{{Addr: e.addrs[next]}})
		start := time.Now()
		if !ac.mu.TryLock() {
			heldOnReturn++
			for !ac.mu.TryLock() {
			}
		}
		busyWaits = append(busyWaits, time.Since(start))
		ac.mu.Unlock()
		e.waitState(ac, connectivity.Connecting, e.addrs[next])
	}
	t.Logf("ac.mu was still locked when updateAddrs returned in %d of %d calls", heldOnReturn, rounds)
	t.Logf("time until a caller that never yields could take ac.mu: %s", g1Summary(busyWaits))
}
