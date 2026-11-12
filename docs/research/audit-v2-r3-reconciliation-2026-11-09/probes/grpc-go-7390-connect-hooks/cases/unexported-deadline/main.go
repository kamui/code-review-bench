package main

import (
	"time"

	"google.golang.org/grpc"
)

func main() {
	_ = grpc.withMinConnectDeadline(func() time.Duration { time.Sleep(time.Second); return time.Second })
}
