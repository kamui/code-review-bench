package redis2

import (
	"context"
	"errors"
	"fmt"
	"testing"

	"github.com/redis/go-redis/v9"
	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

// This fake supplies deterministic command outcomes, not a Redis server.
// Embedding the interface makes unexpected calls fail instead of reaching a network.
type reviewCleanupClient struct {
	redis.UniversalClient
	mode string
	live, indexed bool
	value string
	commands []string
}

func (c *reviewCleanupClient) ZRangeByLex(ctx context.Context, key string, opt *redis.ZRangeBy) *redis.StringSliceCmd {
	c.commands = append(c.commands, "zrangebylex")
	if c.indexed { return redis.NewStringSliceResult([]string{"entry"}, nil) }
	return redis.NewStringSliceResult(nil, nil)
}
func (c *reviewCleanupClient) Get(ctx context.Context, key string) *redis.StringCmd {
	c.commands = append(c.commands, "get")
	if !c.live { return redis.NewStringResult("", redis.Nil) }
	return redis.NewStringResult(c.value, nil)
}
func (c *reviewCleanupClient) ZRem(ctx context.Context, key string, members ...interface{}) *redis.IntCmd {
	c.commands = append(c.commands, "zrem")
	// Recreate after listing GET returned Nil, before removal: INSERT's
	// ZADD NX has already completed while the old member remained present.
	if c.mode != "orphan" { c.live = true }
	c.indexed = false
	if c.mode == "remove_reply_lost" { return redis.NewIntResult(0, errors.New("reply lost after application")) }
	return redis.NewIntResult(1, nil)
}
func (c *reviewCleanupClient) Exists(ctx context.Context, keys ...string) *redis.IntCmd {
	c.commands = append(c.commands, "exists")
	if c.mode == "exists_failure" { return redis.NewIntResult(0, errors.New("existence unavailable")) }
	if c.mode == "replica_lag" || !c.live { return redis.NewIntResult(0, nil) }
	return redis.NewIntResult(1, nil)
}
func (c *reviewCleanupClient) ZAddNX(ctx context.Context, key string, members ...redis.Z) *redis.IntCmd {
	c.commands = append(c.commands, "zadd")
	if c.mode == "restore_failure" { return redis.NewIntResult(0, errors.New("restoration unavailable")) }
	c.indexed = true
	return redis.NewIntResult(1, nil)
}

func TestReviewCleanupCommandOutcomes(t *testing.T) {
	for _, prefix := range []string{"", "sw:"} {
		for _, mode := range []string{"orphan", "restore_ok", "exists_failure", "replica_lag", "restore_failure", "remove_reply_lost"} {
			t.Run(fmt.Sprintf("%s/%s", prefix, mode), func(t *testing.T) {
				path := util.FullPath("/review/entry")
				value, err := (&filer.Entry{FullPath: path, Attr: filer.Attr{Mode: 0644}}).EncodeAttributesAndChunks()
				if err != nil { t.Fatal(err) }
				client := &reviewCleanupClient{mode: mode, indexed: true, value: string(value)}
				store := &UniversalRedis2Store{Client: client, keyPrefix: prefix}
				count := 0
				callback := func(entry *filer.Entry) (bool, error) { count++; return true, nil }
				if _, err := store.ListDirectoryEntries(context.Background(), "/review", "", true, 100, callback); err != nil { t.Fatal(err) }
				if count != 0 { t.Fatalf("initial stale read should be skipped, got %d", count) }
				if _, err := store.ListDirectoryEntries(context.Background(), "/review", "", true, 100, callback); err != nil { t.Fatal(err) }
				wantIndexed := mode == "restore_ok" || mode == "exists_failure"
				if client.indexed != wantIndexed { t.Fatalf("indexed=%v, expected %v", client.indexed, wantIndexed) }
				wantCount := 0
				if wantIndexed { wantCount = 1 }
				if count != wantCount { t.Fatalf("second list count=%d, expected %d", count, wantCount) }
				t.Logf("live=%v indexed=%v listed=%d commands=%v", client.live, client.indexed, count, client.commands)
			})
		}
	}
}
