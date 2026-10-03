package redis2

import (
	"context"
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"

	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

func newTestStore(t *testing.T, keyPrefix string) (*UniversalRedis2Store, util.FullPath) {
	t.Helper()

	if os.Getenv("RUN_REDIS_TESTS") != "1" {
		t.Skip("redis2 tests are disabled. Start a redis-server and set RUN_REDIS_TESTS=1 to enable, REDIS_ADDR defaults to 127.0.0.1:6379.")
	}

	addr := os.Getenv("REDIS_ADDR")
	if addr == "" {
		addr = "127.0.0.1:6379"
	}

	ctx := context.Background()
	client := redis.NewClient(&redis.Options{Addr: addr})
	if err := client.Ping(ctx).Err(); err != nil {
		t.Fatalf("connect to redis at %s: %v", addr, err)
	}

	store := &UniversalRedis2Store{Client: client, keyPrefix: keyPrefix}
	store.loadSuperLargeDirectories(nil)

	dir := util.FullPath(fmt.Sprintf("/redis2_test_%d", time.Now().UnixNano()))
	t.Cleanup(func() {
		store.DeleteFolderChildren(ctx, dir)
		store.DeleteEntry(ctx, dir)
		client.Close()
	})

	return store, dir
}

func insertTestEntry(t *testing.T, store *UniversalRedis2Store, path util.FullPath, ttlSec int32) {
	t.Helper()

	now := time.Now()
	if err := store.InsertEntry(context.Background(), &filer.Entry{
		FullPath: path,
		Attr:     filer.Attr{Crtime: now, Mtime: now, Mode: 0644, TtlSec: ttlSec},
	}); err != nil {
		t.Fatalf("InsertEntry %s: %v", path, err)
	}
}

func listNames(t *testing.T, store *UniversalRedis2Store, dir util.FullPath) []string {
	t.Helper()

	names := []string{}
	if _, err := store.ListDirectoryEntries(context.Background(), dir, "", true, 100, func(entry *filer.Entry) (bool, error) {
		_, name := entry.FullPath.DirAndName()
		names = append(names, name)
		return true, nil
	}); err != nil {
		t.Fatalf("ListDirectoryEntries %s: %v", dir, err)
	}
	return names
}

func indexMembers(t *testing.T, store *UniversalRedis2Store, dir util.FullPath) []string {
	t.Helper()

	members, err := store.Client.ZRangeByLex(context.Background(), store.getKey(genDirectoryListKey(string(dir))), &redis.ZRangeBy{Min: "-", Max: "+"}).Result()
	if err != nil {
		t.Fatalf("read directory index of %s: %v", dir, err)
	}
	return members
}

func TestListDirectoryEntriesRemovesOrphanedIndexMembers(t *testing.T) {
	for _, keyPrefix := range []string{"", "sw:"} {
		t.Run("keyPrefix="+keyPrefix, func(t *testing.T) {
			store, dir := newTestStore(t, keyPrefix)

			insertTestEntry(t, store, dir.Child("alive"), 0)
			insertTestEntry(t, store, dir.Child("orphan"), 0)

			if err := store.Client.Del(context.Background(), store.getKey(string(dir.Child("orphan")))).Err(); err != nil {
				t.Fatalf("drop value key: %v", err)
			}

			if names := listNames(t, store, dir); len(names) != 1 || names[0] != "alive" {
				t.Fatalf("listed %v, want [alive]", names)
			}

			if members := indexMembers(t, store, dir); len(members) != 1 || members[0] != "alive" {
				t.Fatalf("directory index holds %v, want [alive]", members)
			}
		})
	}
}

func TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry(t *testing.T) {
	store, dir := newTestStore(t, "")

	path := dir.Child("recreated")
	insertTestEntry(t, store, path, 0)

	store.removeOrphanedDirectoryListMember(context.Background(), store.getKey(genDirectoryListKey(string(dir))), path, "recreated")

	if members := indexMembers(t, store, dir); len(members) != 1 || members[0] != "recreated" {
		t.Fatalf("directory index holds %v, want [recreated]", members)
	}

	if names := listNames(t, store, dir); len(names) != 1 || names[0] != "recreated" {
		t.Fatalf("listed %v, want [recreated]", names)
	}
}

func TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis(t *testing.T) {
	store, dir := newTestStore(t, "")

	insertTestEntry(t, store, dir.Child("ttl"), 1)

	time.Sleep(1500 * time.Millisecond)

	if exists, err := store.Client.Exists(context.Background(), store.getKey(string(dir.Child("ttl")))).Result(); err != nil {
		t.Fatalf("check value key: %v", err)
	} else if exists != 0 {
		t.Fatal("redis did not expire the value key, the logical expiry path is not being bypassed")
	}

	if names := listNames(t, store, dir); len(names) != 0 {
		t.Fatalf("listed %v, want none", names)
	}

	if members := indexMembers(t, store, dir); len(members) != 0 {
		t.Fatalf("directory index holds %v, want none", members)
	}
}


// Scripted single-path client. No sockets or Redis fixture are used.
// The first GET models the not-found observation; a completed InsertEntry
// can then leave a live value while its ZAddNX was a no-op on the old member.
type reviewScriptClient struct {
    redis.UniversalClient
    member bool
    value string
    encoded string
    getCalls int
    recreate bool
    staleExists bool
    zremErr error
    restoreErr error
    cancelAfterRemove context.CancelFunc
    events []string
}

func (c *reviewScriptClient) ZRangeByLex(ctx context.Context, key string, opt *redis.ZRangeBy) *redis.StringSliceCmd {
    if c.member { return redis.NewStringSliceResult([]string{"live"}, nil) }
    return redis.NewStringSliceResult([]string{}, nil)
}
func (c *reviewScriptClient) Get(ctx context.Context, key string) *redis.StringCmd {
    c.getCalls++
    if c.getCalls == 1 {
        c.events = append(c.events, "GET missing")
        if c.recreate {
            c.value = c.encoded
            c.events = append(c.events, "insert SET live; ZADD NX no-op on existing member")
        }
        return redis.NewStringResult("", redis.Nil)
    }
    if c.value == "" { return redis.NewStringResult("", redis.Nil) }
    return redis.NewStringResult(c.value, nil)
}
func (c *reviewScriptClient) ZRem(ctx context.Context, key string, members ...interface{}) *redis.IntCmd {
    c.member = false
    c.events = append(c.events, "ZREM applied")
    if c.cancelAfterRemove != nil { c.cancelAfterRemove() }
    return redis.NewIntResult(1, c.zremErr)
}
func (c *reviewScriptClient) Exists(ctx context.Context, keys ...string) *redis.IntCmd {
    if err := ctx.Err(); err != nil {
        c.events = append(c.events, "EXISTS canceled")
        return redis.NewIntResult(0, err)
    }
    if c.staleExists {
        c.events = append(c.events, "EXISTS stale replica: 0")
        return redis.NewIntResult(0, nil)
    }
    if c.value != "" {
        c.events = append(c.events, "EXISTS live: 1")
        return redis.NewIntResult(1, nil)
    }
    c.events = append(c.events, "EXISTS absent: 0")
    return redis.NewIntResult(0, nil)
}
func (c *reviewScriptClient) ZAddNX(ctx context.Context, key string, members ...redis.Z) *redis.IntCmd {
    err := ctx.Err()
    if err == nil { err = c.restoreErr }
    if err != nil {
        c.events = append(c.events, "ZADD NX failed: " + err.Error())
        return redis.NewIntResult(0, err)
    }
    c.member = true
    c.events = append(c.events, "ZADD NX restored")
    return redis.NewIntResult(1, nil)
}

func TestReviewCleanupStateTransitions(t *testing.T) {
    cases := []struct {
        name string
        recreate, staleExists, cancel bool
        zremErr, restoreErr error
        wantMember bool
    }{
        {name: "genuine_orphan"},
        {name: "successful_recreation", recreate: true, wantMember: true},
        {name: "restoration_error", recreate: true, restoreErr: fmt.Errorf("injected transport failure")},
        {name: "removal_reply_lost", recreate: true, zremErr: fmt.Errorf("injected read timeout after apply")},
        {name: "cancellation_after_removal", recreate: true, cancel: true},
        {name: "stale_replica_absence", recreate: true, staleExists: true},
    }
    for _, tc := range cases {
        t.Run(tc.name, func(t *testing.T) {
            ctx, cancel := context.WithCancel(context.Background())
            defer cancel()
            path := util.FullPath("/review/live")
            data, err := (&filer.Entry{FullPath: path, Attr: filer.Attr{Mode: 0644}}).EncodeAttributesAndChunks()
            if err != nil { t.Fatal(err) }
            client := &reviewScriptClient{member: true, encoded: string(data), recreate: tc.recreate,
                staleExists: tc.staleExists, zremErr: tc.zremErr, restoreErr: tc.restoreErr}
            if tc.cancel { client.cancelAfterRemove = cancel }
            store := &UniversalRedis2Store{Client: client}
            count := 0
            last, err := store.ListDirectoryEntries(ctx, "/review", "", true, 100, func(*filer.Entry) (bool, error) {
                count++
                return true, nil
            })
            if err != nil || last != "live" || count != 0 {
                t.Fatalf("unexpected first listing: last=%q err=%v count=%d", last, err, count)
            }
            if client.member != tc.wantMember { t.Fatalf("member=%v want=%v", client.member, tc.wantMember) }
            if tc.recreate {
                if _, err := store.FindEntry(context.Background(), path); err != nil {
                    t.Fatalf("live value must remain directly readable: %v", err)
                }
            }
            count = 0
            _, err = store.ListDirectoryEntries(context.Background(), "/review", "", true, 100, func(*filer.Entry) (bool, error) {
                count++
                return true, nil
            })
            wantCount := 0
            if tc.wantMember { wantCount = 1 }
            if err != nil || count != wantCount { t.Fatalf("subsequent listing count=%d want=%d err=%v", count, wantCount, err) }
            t.Logf("events=%v; live=%v indexed=%v next_listing_count=%d first_listing_error=nil", client.events, client.value != "", client.member, count)
        })
    }
}
