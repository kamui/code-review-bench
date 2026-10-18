package redis2

// Probe C2 for seaweedfs/seaweedfs#10735: the comment's own sequence in the listing's
// logical-expiry branch (unchanged lines): "FindEntry returns an entry that is logically expired.
// A concurrent InsertEntry or UpdateEntry then rewrites the same path with a fresh value. The
// listing runs Del(value) ... It then runs ZRem(member)".
// Dropped into weed/filer/redis2/ beside the saved helpers, at the commit before the change and
// at its head. Real redis2 store, and in cases E and F a real filer.Filer as the writer, against a
// real redis-server (PROBE_REDIS_ADDR). The order is forced with a go-redis hook on the listing's
// client; each case prints the order Redis saw. No error is injected. Nothing asserts.

import (
	"context"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"

	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/pb"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

func c2Filer(client *redis.Client) *filer.Filer {
	store := &Redis2Store{}
	store.Client = client
	store.loadSuperLargeDirectories(nil)
	f := filer.NewFiler(pb.ServerDiscovery{}, nil, "", "", "", "", "", 255, nil)
	f.DirBucketsPath = "/buckets"
	f.SetStore(store)
	return f
}

// c2Seed: f's creation time is `age` ago with TtlSec 5, stored through the store, so Redis holds
// its record for 5 s more whatever its creation time says.
func (env *probeEnv) c2Seed(age time.Duration) {
	env.t.Helper()
	ctx := context.Background()
	store := env.quietStore()
	store.InsertEntry(ctx, probeDirEntry(env.dir))
	created := time.Now().Add(-age)
	store.InsertEntry(ctx, &filer.Entry{FullPath: env.dir.Child("f"), Attr: filer.Attr{Crtime: created, Mtime: created, Mode: 0644, TtlSec: 5}})
	env.t.Logf("seeded: f's creation time is %v ago, its time-to-live 5 s; Redis holds its record for 5 s more", age)
}

func (env *probeEnv) c2Writer(label, cmd, when string, match func(redis.Cmder) bool, update bool) *probeRule {
	return &probeRule{label: label, cmd: cmd, when: when, match: match, act: func(context.Context, redis.Cmder) error {
		writer := env.store(env.client("writer", env.addr, nil))
		var err error
		if update {
			err = writer.UpdateEntry(context.Background(), probeEntry(env.dir.Child("f")))
			env.trace.add("%-9s UpdateEntry(f) returned err=%v", "writer:", err)
		} else {
			err = writer.InsertEntry(context.Background(), probeEntry(env.dir.Child("f")))
			env.trace.add("%-9s InsertEntry(f) returned err=%v", "writer:", err)
		}
		return nil
	}}
}

func (env *probeEnv) c2After() {
	env.t.Helper()
	env.t.Logf("command order as seen by redis:")
	env.trace.dump(env.t)
	env.notReached()
	env.state("after both requests", "f")
	entry, err := env.quietStore().FindEntry(context.Background(), env.dir.Child("f"))
	env.t.Logf("FindEntry(f) by full path: found=%v err=%v", entry != nil && err == nil, err)
	env.laterListing("(a fresh listing)")
}

func c2GotValue(cmd redis.Cmder) bool { return cmd.Err() == nil }

func TestProbeC2(t *testing.T) {
	ctx := context.Background()

	cases := []struct {
		id, title, cmd, when string
		match                func(redis.Cmder) bool
		update               bool
	}{
		{"C2A", "a complete InsertEntry(f) lands after the listing's GET of f and before its DEL", "get", "after", c2GotValue, false},
		{"C2B", "a complete UpdateEntry(f) lands after the listing's GET of f and before its DEL", "get", "after", c2GotValue, true},
		{"C2C", "a complete InsertEntry(f) lands after the listing's DEL of f and before its ZREM", "zrem", "before", nil, false},
		{"C2D", "a complete UpdateEntry(f) lands after the listing's DEL of f and before its ZREM", "zrem", "before", nil, true},
	}
	for _, c := range cases {
		c := c
		t.Run(c.id, func(t *testing.T) {
			env := newProbeEnv(t, c.id, "a listing finds f logically expired; "+c.title)
			env.c2Seed(10 * time.Second)
			lister := env.store(env.client("lister", env.addr, nil, env.c2Writer(c.title, c.cmd, c.when, c.match, c.update)))
			names, err := probeListVia(lister, ctx, env.dir)
			env.trace.add("%-9s the listing returned names=%v err=%v", "lister:", names, err)
			env.c2After()
		})
	}

	filerCases := []struct {
		id, title, cmd, when string
		match                func(redis.Cmder) bool
	}{
		{"C2E", "through the filer: a writer reads f before its creation time plus time-to-live passes, a listing runs after it has passed, and the writer's Filer.UpdateEntry(f) lands after the listing's GET and before its DEL", "get", "after", c2GotValue},
		{"C2F", "through the filer: the same, the writer's Filer.UpdateEntry(f) landing after the listing's DEL and before its ZREM", "zrem", "before", nil},
	}
	for _, c := range filerCases {
		c := c
		t.Run(c.id, func(t *testing.T) {
			env := newProbeEnv(t, c.id, c.title)
			env.c2Seed(4 * time.Second)
			writer := c2Filer(env.client("writer", env.addr, nil))
			old, err := writer.FindEntry(ctx, env.dir.Child("f"))
			t.Logf("writer: Filer.FindEntry(f) 1 s before its creation time plus time-to-live: found=%v err=%v", old != nil, err)
			time.Sleep(1500 * time.Millisecond)
			rule := &probeRule{label: c.title, cmd: c.cmd, when: c.when, match: c.match, act: func(context.Context, redis.Cmder) error {
				now := time.Now()
				newEntry := &filer.Entry{FullPath: env.dir.Child("f"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}, Content: []byte("new")}
				err := writer.UpdateEntry(context.Background(), old, newEntry)
				env.trace.add("%-9s Filer.UpdateEntry(f) returned err=%v", "writer:", err)
				return nil
			}}
			lister := env.store(env.client("lister", env.addr, nil, rule))
			names, err := probeListVia(lister, ctx, env.dir)
			env.trace.add("%-9s the listing returned names=%v err=%v", "lister:", names, err)
			env.c2After()
		})
	}
}

var _ = util.FullPath("")
