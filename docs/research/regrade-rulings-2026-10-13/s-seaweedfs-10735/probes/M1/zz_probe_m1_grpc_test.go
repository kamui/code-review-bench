package redis2

// One ListEntries request to a running filer over gRPC, as the S3 gateway, the mount and the
// shell send it. Used by end-to-end.sh. E2E_FILER_GRPC is the filer's gRPC address, E2E_DIR the
// directory, E2E_LIMIT the request's limit (0 lets the server use its own).

import (
	"context"
	"io"
	"os"
	"strconv"
	"testing"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

	"github.com/seaweedfs/seaweedfs/weed/pb/filer_pb"
)

func TestProbeM1GrpcListEntries(t *testing.T) {
	addr := os.Getenv("E2E_FILER_GRPC")
	if addr == "" {
		t.Skip("set E2E_FILER_GRPC to a running filer's gRPC address")
	}
	limit, _ := strconv.Atoi(os.Getenv("E2E_LIMIT"))
	conn, err := grpc.NewClient(addr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		t.Fatal(err)
	}
	defer conn.Close()
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	stream, err := filer_pb.NewSeaweedFilerClient(conn).ListEntries(ctx, &filer_pb.ListEntriesRequest{Directory: os.Getenv("E2E_DIR"), Limit: uint32(limit)})
	if err != nil {
		t.Fatal(err)
	}
	count, first, last := 0, "", ""
	for {
		resp, recvErr := stream.Recv()
		if recvErr == io.EOF {
			break
		}
		if recvErr != nil {
			t.Fatalf("after %d entries: %v", count, recvErr)
		}
		if count == 0 {
			first = resp.Entry.Name
		}
		last = resp.Entry.Name
		count++
	}
	t.Logf("GRPC ListEntries(directory=%s, limit=%d): %d entries, first=%q last=%q", os.Getenv("E2E_DIR"), limit, count, first, last)
}
