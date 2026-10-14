package loadbalance_test

import (
	"testing"

	"github.com/ahmad-khatib0/go/distributed-services/proglog/internal/loadbalance"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/attributes"
	"google.golang.org/grpc/balancer"
	"google.golang.org/grpc/balancer/base"
	"google.golang.org/grpc/resolver"
)

// TestPickerNoSubConnAvailable() tests that a picker initially returns balancer.ErrNoSubConnAvailable
// before the resolver has discovered servers and updated the picker’s state with available subconnections.
func TestPickerNoSubConnAvailable(t *testing.T) {
	picker := &loadbalance.Picker{}

	for _, method := range []string{
		"/log.vX.Log/Produce",
		"/log.vX.Log/Consume",
	} {

		info := balancer.PickInfo{FullMethodName: method}
		result, err := picker.Pick(info)

		// balancer.ErrNoSubConnAvailable instructs gRPC to block the client’s RPCs
		// until the picker has an available subconnection to handle them
		require.Equal(t, balancer.ErrNoSubConnAvailable, err)
		require.Nil(t, result.SubConn)
	}
}

// TestPickerProducesToLeader() tests that the picker picks the leader
// subconnection for append calls.
func TestPickerProducesToLeader(t *testing.T) {
	picker, subConns := setupTest()
	info := balancer.PickInfo{FullMethodName: "/log.vX.Log/Produce"}

	for i := 0; i < 5; i++ {
		gotPick, err := picker.Pick(info)

		require.NoError(t, err)
		require.Equal(t, subConns[0], gotPick.SubConn)
	}
}

// TestPickerConsumesFromFollowers() tests that the picker picks the
// followers subconnections in a round-robin for consume calls
func TestPickerConsumesFromFollowers(t *testing.T) {
	picker, subConns := setupTest()
	info := balancer.PickInfo{FullMethodName: "/log.vX.Log/Consume"}

	for i := 0; i < 5; i++ {
		pick, err := picker.Pick(info)

		require.NoError(t, err)
		require.Equal(t, subConns[i%2+1], pick.SubConn)
	}
}

func setupTest() (*loadbalance.Picker, []*subConn) {
	var subConns []*subConn

	buildInfo := base.PickerBuildInfo{ReadySCs: make(map[balancer.SubConn]base.SubConnInfo)}

	for i := 0; i < 3; i++ {
		sc := &subConn{}
		addr := resolver.Address{Attributes: attributes.New("is_leader", i == 0)}

		sc.UpdateAddresses([]resolver.Address{addr})
		// FIX:
		// buildInfo.ReadySCs[sc] = base.SubConnInfo{Address: addr}

		subConns = append(subConns, sc)
	}

	picker := &loadbalance.Picker{}
	picker.Build(buildInfo)

	return picker, subConns
}

// subConn implements balancer.SubConn.
type subConn struct {
	addrs []resolver.Address
}

func (s *subConn) UpdateAddresses(addrs []resolver.Address) {
	s.addrs = addrs
}

func (s *subConn) Connect() {}
