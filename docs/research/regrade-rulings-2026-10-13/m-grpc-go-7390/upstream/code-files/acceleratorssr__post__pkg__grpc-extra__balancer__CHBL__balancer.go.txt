package CHBL

import (
	"google.golang.org/grpc/attributes"
	"google.golang.org/grpc/balancer"
	"google.golang.org/grpc/balancer/base"
	"google.golang.org/grpc/resolver"
	"sync"
)

//func init() {
//	balancer.Register(&MyCustomBalancer{})
//}

type MyCustomBalancer struct {
	name          string
	pickerBuilder base.PickerBuilder
	config        base.Config
}

func (b *MyCustomBalancer) Build(cc balancer.ClientConn, opts balancer.BuildOptions) balancer.Balancer {
	return &myBalancer{
		cc:            cc,
		pickerBuilder: b.pickerBuilder,
		config:        b.config,
	}
}

func (b *MyCustomBalancer) Name() string {
	return b.name
}

type myBalancer struct {
	pickerBuilder base.PickerBuilder
	config        base.Config
	cc            balancer.ClientConn
	subConns      map[resolver.Address]balancer.SubConn
	mu            sync.Mutex
}

func (b *myBalancer) ResolverError(err error) {
	//TODO implement me
	panic("ResolverError")
}

func (b *myBalancer) UpdateSubConnState(conn balancer.SubConn, state balancer.SubConnState) {
	//TODO implement me
	panic("UpdateSubConnState")
}

func (b *myBalancer) Close() {
	//TODO implement me
	panic("Close")
}

func (b *myBalancer) UpdateClientConnState(s balancer.ClientConnState) error {
	//b.mu.Lock()
	//defer b.mu.Unlock()
	//
	//for _, addr := range s.ResolverState.Addresses {
	//	sc, ok := b.subConns[addr]
	//	if ok {
	//		// 如果连接已经存在，检查 metadata 是否变化
	//		newMetadata := addr.BalancerAttributes
	//		//currentMetadata := sc.GetAttributes()
	//
	//		// 如果 metadata 有变化，调用 UpdateAddresses 更新 metadata
	//		//if !metadataEqual(currentMetadata, newMetadata) {
	//		//	sc.UpdateAddresses([]resolver.Address{addr})
	//		//}
	//	} else {
	//		// 如果连接不存在，创建新的 SubConn
	//		sc, err := b.cc.NewSubConn([]resolver.Address{addr}, balancer.NewSubConnOptions{})
	//		if err != nil {
	//			return err
	//		}
	//		sc.Connect()
	//		b.subConns[addr] = sc
	//	}
	//}
	panic("UpdateClientConnState")
	return nil
}

func metadataEqual(a, b *attributes.Attributes) bool {
	// 实现 metadata 的比较逻辑
	return a == b
}
