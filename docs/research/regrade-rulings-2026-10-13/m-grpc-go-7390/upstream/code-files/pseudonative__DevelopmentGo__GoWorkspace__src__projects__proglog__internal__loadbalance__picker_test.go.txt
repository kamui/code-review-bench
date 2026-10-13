package loadbalance_test

import (
	"testing"

	"github.com/pseudonative/proglog/internal/loadbalance"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/attributes"
	"google.golang.org/grpc/balancer"
	"google.golang.org/grpc/balancer/base"
	"google.golang.org/grpc/resolver"
)

func TestPickerNoSubConnAvailable(t *testing.T) {
	picker := &loadbalance.Picker{}
	for _, method := range []string{
		"/log.vX.Log/Produce",
		"/log.vX.Log/Consume",
	} {
		info := balancer.PickInfo{
			FullMethodName: method,
		}
		result, err := picker.Pick(info)
		require.Equal(t, balancer.ErrNoSubConnAvailable, err)
		require.Nil(t, result.SubConn)
	}
}

func TestPickerProducesToLeader(t *testing.T) {
	picker, subConns := setupTest()
	info := balancer.PickInfo{
		FullMethodName: "/log.vX.Log/Produce",
	}
	for i := 0; i < 5; i++ {
		gotPick, err := picker.Pick(info)
		require.NoError(t, err)
		require.Equal(t, subConns[0], gotPick.SubConn)
	}
}

func TestPickerConsumesFromFollowers(t *testing.T) {
	picker, subConns := setupTest()
	info := balancer.PickInfo{
		FullMethodName: "/log.vX.Log/Consume",
	}
	for i := 0; i < 5; i++ {
		pick, err := picker.Pick(info)
		require.NoError(t, err)
		require.Equal(t, subConns[i%2+1], pick.SubConn)
	}
}

type subConn struct {
	isLeader bool
}

func (s *subConn) Connect() {}

func (s *subConn) Shutdown() {}

func (s *subConn) UpdateAddresses(addrs []resolver.Address) {
	// TODO: Implement the logic for updating the addresses of the SubConn.
	// The following is a placeholder. Replace it with actual logic.
}

func (s *subConn) GetOrBuildProducer(builder balancer.ProducerBuilder) (balancer.Producer, func()) {
	// TODO: Implement the logic for creating or retrieving a balancer.Producer based on the builder provided.
	// The following is a placeholder implementation.
	return nil, func() { /* cleanup logic here */ }
}

func setupTest() (*loadbalance.Picker, []balancer.SubConn) {
	picker := &loadbalance.Picker{}
	readySCs := make(map[balancer.SubConn]base.SubConnInfo)
	var subConns []balancer.SubConn

	for i := 0; i < 3; i++ {
		sc := &subConn{isLeader: i == 0}
		addr := resolver.Address{
			Attributes: attributes.New("is_leader", sc.isLeader),
		}
		scInfo := base.SubConnInfo{Address: addr}
		readySCs[sc] = scInfo
		subConns = append(subConns, sc)
	}

	picker.Build(base.PickerBuildInfo{ReadySCs: readySCs})

	return picker, subConns
}
