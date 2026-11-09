package main

import "google.golang.org/grpc"

type slowHook struct{}

func main() {
	var _ grpc.DialOption = slowHook{}
}
