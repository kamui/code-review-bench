package algorithm

import (
	"context"
	"math/rand"
	"testing"

	"google.golang.org/grpc/balancer"
	"google.golang.org/grpc/balancer/base"
	"google.golang.org/grpc/resolver"
)

type testSubConn struct {
	name string
}

func (t *testSubConn) UpdateAddresses([]resolver.Address) {
}

func (t *testSubConn) Connect() {
}

func (t *testSubConn) GetOrBuildProducer(balancer.ProducerBuilder) (p balancer.Producer, close func()) {
	return nil, nil
}

func (t *testSubConn) Shutdown() {
}

func TestRandomPicker(t *testing.T) {
	picker := &randomPicker{subConns: make([]balancer.SubConn, 2), rand: rand.New(rand.NewSource(100))}
	picker.subConns[0] = &testSubConn{}
	picker.subConns[1] = &testSubConn{}
	res, err := picker.Pick(balancer.PickInfo{Ctx: context.Background(), FullMethodName: "test.Test"})
	if err != nil {
		t.Fatalf("test picker failure, error: %s", err)
	}

	if res.SubConn == nil {
		t.Fatalf("test picker failure, sub conn is nil")
	}
}

func TestRandom(t *testing.T) {
	builder := &randomBuilder{}
	subConns := make(map[balancer.SubConn]base.SubConnInfo)
	subConns[&testSubConn{name: "testSub0"}] = base.SubConnInfo{Address: resolver.Address{Addr: "test.test", ServerName: "test"}}
	subConns[&testSubConn{name: "testSub1"}] = base.SubConnInfo{Address: resolver.Address{Addr: "test.test", ServerName: "test"}}
	picker := builder.Build(base.PickerBuildInfo{ReadySCs: subConns})
	res, err := picker.Pick(balancer.PickInfo{Ctx: context.Background(), FullMethodName: "test.Test"})
	if err != nil {
		t.Fatalf("test random builder failure, error: %s", err)
	}

	sub, ok := res.SubConn.(*testSubConn)
	if !ok {
		t.Fatalf("test random builder failure, conn not *testSubConn")
	}
	t.Logf("sub conn name is %s", sub.name)
}
