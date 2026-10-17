package redis2

// Probe C1 for seaweedfs/seaweedfs#10735: the comment's own scenario. A directory /d/sub stored
// with a time-to-live, children stored with a time-to-live; Redis expires all of their records;
// /d is listed. What happens to /d/sub's list of children ("<path>\x00"), at the commit before
// the change and at its head, and what later reaches it.
// Dropped into weed/filer/redis2/ beside the saved helpers. Real redis2 store and, where named, a
// real filer.Filer against a real redis-server (PROBE_REDIS_ADDR). No order is forced; Redis's own
// expiry removes the records. Nothing asserts.

import (
	"context"
	"os"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"

	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/pb"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

func c1Filer(client *redis.Client) *filer.Filer {
	store := &Redis2Store{}
	store.Client = client
	store.loadSuperLargeDirectories(nil)
	f := filer.NewFiler(pb.ServerDiscovery{}, nil, "", "", "", "", "", 255, nil)
	f.DirBucketsPath = "/buckets"
	f.SetStore(store)
	return f
}

// c1Seed stores /d (no time-to-live), /d/sub as a directory with TtlSec 1, and /d/sub/x and
// /d/sub/y with TtlSec 1, through the store, then waits until Redis has expired the three records.
func (env *probeEnv) c1Seed() util.FullPath {
	env.t.Helper()
	ctx := context.Background()
	store := env.quietStore()
	sub := env.dir.Child("sub")
	now := time.Now()
	store.InsertEntry(ctx, probeDirEntry(env.dir))
	store.InsertEntry(ctx, &filer.Entry{FullPath: sub, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: os.ModeDir | 0755, TtlSec: 1}})
	store.InsertEntry(ctx, &filer.Entry{FullPath: sub.Child("x"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644, TtlSec: 1}})
	store.InsertEntry(ctx, &filer.Entry{FullPath: sub.Child("y"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644, TtlSec: 1}})
	time.Sleep(2500 * time.Millisecond)
	n, _ := env.direct.Exists(ctx, string(sub), string(sub.Child("x")), string(sub.Child("y"))).Result()
	ttl, _ := env.direct.TTL(ctx, genDirectoryListKey(string(sub))).Result()
	env.t.Logf("seeded through the store: /d/sub (directory, TtlSec 1), /d/sub/x and /d/sub/y (TtlSec 1); 2.5 s later Redis holds %d of those 3 records; sub's list of children has Redis TTL %v (-1ns: none)", n, ttl)
	return sub
}

func (env *probeEnv) c1Lists(label string, sub util.FullPath) {
	env.t.Helper()
	ctx := context.Background()
	parent, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(env.dir)), 0, -1).Result()
	subExists, _ := env.direct.Exists(ctx, genDirectoryListKey(string(sub))).Result()
	subNames, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(sub)), 0, -1).Result()
	env.t.Logf("STATE %s: the parent's list holds %v; sub's list of children exists=%d holding %v", label, parent, subExists, subNames)
}

func TestProbeC1(t *testing.T) {
	ctx := context.Background()

	t.Run("A_the_comments_scenario_then_nothing", func(t *testing.T) {
		env := newProbeEnv(t, "C1A", "the comment's scenario: list /d after Redis expired /d/sub and its children; then nothing else")
		sub := env.c1Seed()
		names, err := probeListVia(env.quietStore(), ctx, env.dir)
		t.Logf("listing of /d returned names=%v err=%v", names, err)
		env.c1Lists("after the listing of /d", sub)
		names, err = probeListVia(env.quietStore(), ctx, env.dir)
		t.Logf("a second listing of /d returned names=%v err=%v", names, err)
		env.c1Lists("after a second listing of /d", sub)
	})

	t.Run("B_then_the_parent_is_deleted_recursively", func(t *testing.T) {
		env := newProbeEnv(t, "C1B", "the comment's scenario, then the filer's recursive delete of /d (its listing, DeleteFolderChildren, DeleteEntry)")
		sub := env.c1Seed()
		probeListVia(env.quietStore(), ctx, env.dir)
		env.c1Lists("after the listing of /d", sub)
		env.recursiveDelete()
		env.trace.dump(t)
		env.c1Lists("after the recursive delete of /d", sub)
	})

	t.Run("C_then_the_lost_directory_path_is_listed", func(t *testing.T) {
		env := newProbeEnv(t, "C1C", "the comment's scenario, then Filer.ListDirectoryEntries(/d/sub) (what a gRPC ListEntries of that path runs), with no record for /d/sub")
		sub := env.c1Seed()
		probeListVia(env.quietStore(), ctx, env.dir)
		env.c1Lists("after the listing of /d", sub)
		entries, _, err := c1Filer(env.direct).ListDirectoryEntries(ctx, sub, "", false, 100, "", "", "")
		t.Logf("Filer.ListDirectoryEntries(/d/sub) returned %d entries err=%v", len(entries), err)
		env.c1Lists("after the listing of /d/sub", sub)
	})

	t.Run("D_then_the_directory_is_created_again_and_listed", func(t *testing.T) {
		env := newProbeEnv(t, "C1D", "the comment's scenario, then Filer.CreateEntry(/d/sub) as a directory, then a listing of /d/sub")
		sub := env.c1Seed()
		probeListVia(env.quietStore(), ctx, env.dir)
		f := c1Filer(env.direct)
		now := time.Now()
		err := f.CreateEntry(ctx, &filer.Entry{FullPath: sub, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: os.ModeDir | 0755}}, nil, false, false, nil, false, 255)
		t.Logf("Filer.CreateEntry(/d/sub) err=%v", err)
		env.c1Lists("after the re-create", sub)
		entries, _, err := f.ListDirectoryEntries(ctx, sub, "", false, 100, "", "", "")
		t.Logf("Filer.ListDirectoryEntries(/d/sub) returned %d entries err=%v", len(entries), err)
		env.c1Lists("after the listing of the re-created /d/sub", sub)
	})

	t.Run("E_can_a_directory_get_a_time_to_live_through_the_filer", func(t *testing.T) {
		env := newProbeEnv(t, "C1E", "Filer.CreateEntry of a directory with TtlSec 30; Filer.UpdateEntry of a directory with TtlSec 30")
		f := c1Filer(env.direct)
		now := time.Now()
		created := env.dir.Child("made")
		err := f.CreateEntry(ctx, &filer.Entry{FullPath: created, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: os.ModeDir | 0755, TtlSec: 30}}, nil, false, false, nil, false, 255)
		ttl, _ := env.direct.TTL(ctx, string(created)).Result()
		t.Logf("Filer.CreateEntry(directory, TtlSec 30) err=%v; Redis TTL of its record: %v (-1ns: none)", err, ttl)
		updated := env.dir.Child("updated")
		f.CreateEntry(ctx, &filer.Entry{FullPath: updated, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: os.ModeDir | 0755}}, nil, false, false, nil, false, 255)
		old, _ := f.FindEntry(ctx, updated)
		err = f.UpdateEntry(ctx, old, &filer.Entry{FullPath: updated, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: os.ModeDir | 0755, TtlSec: 30}})
		ttl, _ = env.direct.TTL(ctx, string(updated)).Result()
		t.Logf("Filer.UpdateEntry(directory, TtlSec 30) err=%v; Redis TTL of its record: %v", err, ttl)
	})
}
