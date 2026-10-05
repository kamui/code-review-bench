package zzprobeu1

import (
	"context"
	"fmt"
	"io"
	"testing"
	"time"

	"google.golang.org/genproto/googleapis/rpc/errdetails"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/encoding"
	_ "google.golang.org/grpc/encoding/proto"
	"google.golang.org/grpc/internal/stubserver"
	testgrpc "google.golang.org/grpc/interop/grpc_testing"
	testpb "google.golang.org/grpc/interop/grpc_testing"
	"google.golang.org/grpc/status"
)

func TestU1CodecTypedNil(t *testing.T) {
	c := encoding.GetCodec("proto")
	if c == nil {
		fmt.Printf("1-3 encoding.GetCodec(\"proto\") returns nil at this version (the codec moved to a newer interface); see lines 5-9 for the same path through real calls\n")
		return
	}
	b, err := c.Marshal((*testpb.SimpleResponse)(nil))
	fmt.Printf("1 codec.Marshal(typed-nil *SimpleResponse): bytes=%v (nil=%v, len=%d) err=%v\n", b, b == nil, len(b), err)
	b, err = c.Marshal(nil)
	fmt.Printf("2 codec.Marshal(untyped nil): bytes=%v err=%v\n", b, err)
	b, err = c.Marshal(&testpb.SimpleResponse{})
	fmt.Printf("3 codec.Marshal(empty non-nil *SimpleResponse): bytes=%v (len=%d) err=%v\n", b, len(b), err)
}

func TestU1WithDetailsTypedNil(t *testing.T) {
	st, err := status.New(codes.InvalidArgument, "bad input").WithDetails((*errdetails.ErrorInfo)(nil))
	fmt.Printf("4 WithDetails(typed-nil *ErrorInfo): status-is-nil=%v err=%v\n", st == nil, err)
	if st != nil {
		for i, d := range st.Details() {
			fmt.Printf("4 detail[%d]: Go type %T, value %v\n", i, d, d)
		}
	}
}

func TestU1EndToEnd(t *testing.T) {
	var serverGot *testpb.SimpleRequest
	var serverCalled bool
	replyNil := false
	ss := &stubserver.StubServer{
		UnaryCallF: func(ctx context.Context, in *testpb.SimpleRequest) (*testpb.SimpleResponse, error) {
			serverCalled, serverGot = true, in
			if replyNil {
				return nil, nil
			}
			return &testpb.SimpleResponse{Username: "real-reply"}, nil
		},
		FullDuplexCallF: func(stream testgrpc.TestService_FullDuplexCallServer) error {
			for {
				in, err := stream.Recv()
				if err == io.EOF {
					return nil
				}
				if err != nil {
					return err
				}
				fmt.Printf("7 server stream received: %q\n", in.String())
				var out *testpb.StreamingOutputCallResponse
				if err := stream.Send(out); err != nil {
					fmt.Printf("8 server stream.Send(typed-nil) error: %v\n", err)
					return err
				}
				fmt.Printf("8 server stream.Send(typed-nil) returned nil error\n")
			}
		},
	}
	if err := ss.Start(nil); err != nil {
		t.Fatalf("start: %v", err)
	}
	defer ss.Stop()
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	replyNil = true
	resp, err := ss.Client.UnaryCall(ctx, &testpb.SimpleRequest{})
	fmt.Printf("5 unary handler returns (nil, nil): client sees resp=%q resp-is-nil=%v err=%v\n", resp.String(), resp == nil, err)

	replyNil, serverCalled, serverGot = false, false, nil
	resp, err = ss.Client.UnaryCall(ctx, nil)
	fmt.Printf("6 client UnaryCall(ctx, nil): handler-called=%v handler-got=%q client sees resp=%q err=%v\n", serverCalled, serverGot.String(), resp.String(), err)

	stream, err := ss.Client.FullDuplexCall(ctx)
	if err != nil {
		t.Fatalf("stream: %v", err)
	}
	var req *testpb.StreamingOutputCallRequest
	err = stream.Send(req)
	fmt.Printf("7 client stream.Send(typed-nil): err=%v\n", err)
	if err == nil {
		out, rerr := stream.Recv()
		fmt.Printf("8 client stream.Recv after server sent typed-nil: msg=%q msg-is-nil=%v err=%v\n", out.String(), out == nil, rerr)
	}
	stream.CloseSend()
	_, rerr := stream.Recv()
	fmt.Printf("9 client stream final Recv: err=%v\n", rerr)
}
