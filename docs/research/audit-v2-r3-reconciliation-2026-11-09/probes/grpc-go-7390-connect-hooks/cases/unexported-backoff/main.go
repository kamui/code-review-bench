package main

import "google.golang.org/grpc"

func main() {
	_ = grpc.withBackoff(nil)
}
