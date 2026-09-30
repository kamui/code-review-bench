package redis2

import (
	"context"
	"fmt"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"
	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

type claimExpiryValue struct {
	data     []byte
	deadline time.Time
}

type claimExpiryRedis struct {
	redis.UniversalClient
	values  map[string]claimExpiryValue
	indexes map[string]map[string]bool
}

func (r *claimExpiryRedis) value(key string) ([]byte, bool) {
	v, ok := r.values[key]
	if ok && !v.deadline.IsZero() && !time.Now().Before(v.deadline) {
		delete(r.values, key)
		return nil, false
	}
	return v.data, ok
}

func (r *claimExpiryRedis) Set(ctx context.Context, key string, value interface{}, expiration time.Duration) *redis.StatusCmd {
	v := claimExpiryValue{data: append([]byte(nil), value.([]byte)...)}
	if expiration > 0 {
		v.deadline = time.Now().Add(expiration)
	}
	r.values[key] = v
	return redis.NewStatusResult("OK", nil)
}

func (r *claimExpiryRedis) Get(ctx context.Context, key string) *redis.StringCmd {
	data, ok := r.value(key)
	if !ok {
		return redis.NewStringResult("", redis.Nil)
	}
	return redis.NewStringResult(string(data), nil)
}

func (r *claimExpiryRedis) Exists(ctx context.Context, keys ...string) *redis.IntCmd {
	var count int64
	for _, key := range keys {
		if _, ok := r.value(key); ok {
			count++
		}
	}
	return redis.NewIntResult(count, nil)
}

func (r *claimExpiryRedis) ZAddNX(ctx context.Context, key string, members ...redis.Z) *redis.IntCmd {
	if r.indexes[key] == nil {
		r.indexes[key] = make(map[string]bool)
	}
	var count int64
	for _, member := range members {
		name := fmt.Sprint(member.Member)
		if !r.indexes[key][name] {
			r.indexes[key][name] = true
			count++
		}
	}
	return redis.NewIntResult(count, nil)
}

func (r *claimExpiryRedis) ZRem(ctx context.Context, key string, members ...interface{}) *redis.IntCmd {
	var count int64
	for _, member := range members {
		name := fmt.Sprint(member)
		if r.indexes[key][name] {
			delete(r.indexes[key], name)
			count++
		}
	}
	return redis.NewIntResult(count, nil)
}

func (r *claimExpiryRedis) ZRangeByLex(ctx context.Context, key string, bounds *redis.ZRangeBy) *redis.StringSliceCmd {
	var names []string
	for name := range r.indexes[key] {
		names = append(names, name)
	}
	return redis.NewStringSliceResult(names, nil)
}

func TestClaimUpdateAfterExpiryKeepsMember(t *testing.T) {
	ctx := context.Background()
	client := &claimExpiryRedis{values: make(map[string]claimExpiryValue), indexes: make(map[string]map[string]bool)}
	store := &Redis2Store{UniversalRedis2Store: UniversalRedis2Store{Client: client}}
	f := &filer.Filer{Store: filer.NewFilerStoreWrapper(store)}
	path := util.FullPath("/claim/item")
	initial := &filer.Entry{FullPath: path, Attr: filer.Attr{Crtime: time.Now(), Mode: 0644, Inode: 123, TtlSec: 1}}
	if err := store.InsertEntry(ctx, initial); err != nil {
		t.Fatal(err)
	}
	old, err := f.FindEntry(ctx, path)
	if err != nil || old == nil {
		t.Fatalf("supported pre-expiry read: entry=%v error=%v", old, err)
	}
	time.Sleep(1100 * time.Millisecond)
	if _, present := client.value(string(path)); present {
		t.Fatal("value did not expire")
	}
	var before []string
	if _, err := store.ListDirectoryEntries(ctx, "/claim", "", true, 100, func(e *filer.Entry) (bool, error) {
		before = append(before, string(e.FullPath))
		return true, nil
	}); err != nil {
		t.Fatal(err)
	}
	if len(before) != 0 {
		t.Fatalf("expired entry listed: %v", before)
	}
	updated := &filer.Entry{FullPath: path, Attr: filer.Attr{Crtime: time.Now(), Mode: 0644, TtlSec: 0}}
	if err := f.UpdateEntry(ctx, old, updated); err != nil {
		t.Fatalf("supported TTL-removing update: %v", err)
	}
	if found, err := f.FindEntry(ctx, path); err != nil || found == nil || found.TtlSec != 0 {
		t.Fatalf("updated entry not live: entry=%v error=%v", found, err)
	}
	var after []string
	if _, err := store.ListDirectoryEntries(ctx, "/claim", "", true, 100, func(e *filer.Entry) (bool, error) {
		after = append(after, string(e.FullPath))
		return true, nil
	}); err != nil {
		t.Fatal(err)
	}
	t.Logf("live value after update; directory listing=%v; index=%v", after, client.indexes[genDirectoryListKey("/claim")])
	if len(after) != 1 || after[0] != string(path) {
		t.Fatalf("live updated entry missing from listing: %v", after)
	}
}
