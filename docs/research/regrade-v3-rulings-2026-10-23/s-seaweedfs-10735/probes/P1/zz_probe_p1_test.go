package redis2

// Probe P1 for seaweedfs/seaweedfs#10735, round of 2026-10-23: a directory that is past its
// creation time plus its time-to-live while Redis still holds its record, reached by a listing of
// its parent. The listing's expiry branch (head lines 214-219, base 213-218, unchanged) deletes the
// directory's record and its name in the parent's list. What happens to the directory's own list
// of children ("<path>\x00") and to its child, at the commit before the change and at its head.
// Dropped into weed/filer/redis2/ beside the saved helpers (../zz_probe_helpers_test.go). A real
// filer.Filer over the real redis2 store against a real redis-server (PROBE_REDIS_ADDR). Nothing
// is forced: no hook, no injected error. Nothing asserts.

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

func p1Filer(client *redis.Client) *filer.Filer {
	store := &Redis2Store{}
	store.Client = client
	store.loadSuperLargeDirectories(nil)
	f := filer.NewFiler(pb.ServerDiscovery{}, nil, "", "", "", "", "", 255, nil)
	f.DirBucketsPath = "/buckets"
	f.SetStore(store)
	return f
}

func p1Dir(path util.FullPath, crtime time.Time, ttl int32) *filer.Entry {
	return &filer.Entry{FullPath: path, Attr: filer.Attr{Crtime: crtime, Mtime: crtime, Mode: os.ModeDir | 0755, TtlSec: ttl}}
}

func p1File(path util.FullPath) *filer.Entry {
	now := time.Now()
	return &filer.Entry{FullPath: path, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}}
}

// p1SeedThroughFiler: /d, /d/sub (a directory) and /d/sub/x (a file, no time-to-live) are created
// through Filer.CreateEntry. 3 s later Filer.UpdateEntry gives /d/sub a time-to-live of 2 s (what
// a gRPC UpdateEntry carrying TtlSec runs). Filer.UpdateEntry keeps the old creation time, so
// /d/sub is at once past its creation time plus its time-to-live, and Redis keeps its record 2 s.
func (env *probeEnv) p1SeedThroughFiler(f *filer.Filer) util.FullPath {
	env.t.Helper()
	ctx := context.Background()
	sub := env.dir.Child("sub")
	now := time.Now()
	for _, e := range []*filer.Entry{p1Dir(env.dir, now, 0), p1Dir(sub, now, 0), p1File(sub.Child("x"))} {
		if err := f.CreateEntry(ctx, e, nil, false, false, nil, false, 255); err != nil {
			env.t.Fatalf("create %s: %v", e.FullPath, err)
		}
	}
	time.Sleep(3 * time.Second)
	old, err := env.quietStore().FindEntry(ctx, sub)
	if err != nil {
		env.t.Fatalf("find %s: %v", sub, err)
	}
	updated := p1Dir(sub, time.Now(), 2)
	err = f.UpdateEntry(ctx, old, updated)
	ttl, _ := env.direct.TTL(ctx, string(sub)).Result()
	stored, _ := env.quietStore().FindEntry(ctx, sub)
	env.t.Logf("seeded through the filer: /d, /d/sub (directory), /d/sub/x (file, no time-to-live) by Filer.CreateEntry; 3 s later Filer.UpdateEntry(/d/sub, TtlSec 2) err=%v; stored TtlSec=%d, creation time %s old, Redis TTL of the record %v; past creation time plus time-to-live: %v",
		err, stored.TtlSec, time.Since(stored.Crtime).Round(100*time.Millisecond), ttl, stored.Crtime.Add(time.Duration(stored.TtlSec)*time.Second).Before(time.Now()))
	return sub
}

// p1SeedRearmed is the route of the candidate record: /d/sub stored through the store with
// TtlSec 3, /d/sub/x with none; 2 s later Filer.UpdateEntry stores /d/sub again with the same
// time-to-live, which re-arms Redis's expiry from that moment; the probe then waits until the
// creation time plus 3 s has passed (Redis still holds the record).
func (env *probeEnv) p1SeedRearmed(f *filer.Filer) util.FullPath {
	env.t.Helper()
	ctx := context.Background()
	store := env.quietStore()
	sub := env.dir.Child("sub")
	now := time.Now()
	store.InsertEntry(ctx, p1Dir(env.dir, now, 0))
	store.InsertEntry(ctx, p1Dir(sub, now, 3))
	store.InsertEntry(ctx, p1File(sub.Child("x")))
	time.Sleep(2 * time.Second)
	old, _ := store.FindEntry(ctx, sub)
	err := f.UpdateEntry(ctx, old, p1Dir(sub, time.Now(), 3))
	time.Sleep(1500 * time.Millisecond)
	ttl, _ := env.direct.TTL(ctx, string(sub)).Result()
	stored, _ := store.FindEntry(ctx, sub)
	env.t.Logf("seeded through the store: /d, /d/sub (directory, TtlSec 3), /d/sub/x (no time-to-live); 2 s later Filer.UpdateEntry(/d/sub, TtlSec 3) err=%v; 1.5 s after that: record present=%v, Redis TTL %v, past creation time plus time-to-live: %v",
		err, stored != nil, ttl, stored != nil && stored.Crtime.Add(time.Duration(stored.TtlSec)*time.Second).Before(time.Now()))
	return sub
}

func (env *probeEnv) p1State(label string, sub util.FullPath) {
	env.t.Helper()
	ctx := context.Background()
	parent, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(env.dir)), 0, -1).Result()
	subRecord, _ := env.direct.Exists(ctx, string(sub)).Result()
	subList, _ := env.direct.Exists(ctx, genDirectoryListKey(string(sub))).Result()
	subNames, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(sub)), 0, -1).Result()
	xRecord, _ := env.direct.Exists(ctx, string(sub.Child("x"))).Result()
	env.t.Logf("STATE %s: the parent's list holds %v; record of sub exists=%d; sub's list of children exists=%d holding %v; record of sub/x exists=%d",
		label, parent, subRecord, subList, subNames, xRecord)
}

func p1Names(f *filer.Filer, ctx context.Context, dir util.FullPath) ([]string, error) {
	entries, _, err := f.ListDirectoryEntries(ctx, dir, "", false, 100, "", "", "")
	names := []string{}
	for _, e := range entries {
		names = append(names, e.Name())
	}
	return names, err
}

func (env *probeEnv) p1Cleanup(sub util.FullPath) {
	ctx := context.Background()
	env.direct.Del(ctx, string(sub), genDirectoryListKey(string(sub)), string(sub.Child("x")), genDirectoryListKey(string(sub.Child("x"))))
}

func TestProbeP1(t *testing.T) {
	ctx := context.Background()

	t.Run("A_listing_of_the_parent", func(t *testing.T) {
		env := newProbeEnv(t, "P1A", "a directory given a time-to-live by Filer.UpdateEntry, past its creation time plus time-to-live, Redis still holding its record; Filer.ListDirectoryEntries(/d)")
		f := p1Filer(env.direct)
		sub := env.p1SeedThroughFiler(f)
		env.p1State("before the listing", sub)
		names, err := p1Names(f, ctx, env.dir)
		t.Logf("Filer.ListDirectoryEntries(/d) returned names=%v err=%v", names, err)
		env.p1State("after the listing of /d", sub)
		env.p1Cleanup(sub)
	})

	t.Run("B_then_the_parent_is_deleted_recursively_and_sub_created_again", func(t *testing.T) {
		env := newProbeEnv(t, "P1B", "as A; then Filer.DeleteEntryMetaAndData(/d, recursive); then Filer.CreateEntry(/d/sub) as a directory and a listing of it")
		f := p1Filer(env.direct)
		sub := env.p1SeedThroughFiler(f)
		names, err := p1Names(f, ctx, env.dir)
		t.Logf("Filer.ListDirectoryEntries(/d) returned names=%v err=%v", names, err)
		env.p1State("after the listing of /d", sub)
		err = f.DeleteEntryMetaAndData(ctx, env.dir, true, false, false, false, nil, 0)
		t.Logf("Filer.DeleteEntryMetaAndData(/d, isRecursive=true) err=%v", err)
		env.p1State("after the recursive delete of /d", sub)
		now := time.Now()
		err = f.CreateEntry(ctx, p1Dir(sub, now, 0), nil, false, false, nil, false, 255)
		t.Logf("Filer.CreateEntry(/d/sub) as a directory err=%v", err)
		names, err = p1Names(f, ctx, sub)
		t.Logf("Filer.ListDirectoryEntries(/d/sub) of the new directory returned names=%v err=%v", names, err)
		entry, err := f.FindEntry(ctx, sub.Child("x"))
		t.Logf("Filer.FindEntry(/d/sub/x) found=%v err=%v", entry != nil && err == nil, err)
		env.p1State("after the re-create and the listing", sub)
		f.DeleteEntryMetaAndData(ctx, env.dir, true, true, false, false, nil, 0)
		env.p1Cleanup(sub)
	})

	t.Run("C_the_filers_own_expiry_by_path_for_comparison", func(t *testing.T) {
		env := newProbeEnv(t, "P1C", "as A, but the first access is Filer.FindEntry(/d/sub) (the filer's own expiry check, weed/filer/filer.go:434-436) instead of a listing of /d; then the same recursive delete, re-create and listing")
		f := p1Filer(env.direct)
		sub := env.p1SeedThroughFiler(f)
		entry, err := f.FindEntry(ctx, sub)
		t.Logf("Filer.FindEntry(/d/sub) found=%v err=%v", entry != nil && err == nil, err)
		env.p1State("after Filer.FindEntry(/d/sub)", sub)
		names, err := p1Names(f, ctx, env.dir)
		t.Logf("Filer.ListDirectoryEntries(/d) returned names=%v err=%v", names, err)
		err = f.DeleteEntryMetaAndData(ctx, env.dir, true, false, false, false, nil, 0)
		t.Logf("Filer.DeleteEntryMetaAndData(/d, isRecursive=true) err=%v", err)
		env.p1State("after the recursive delete of /d", sub)
		err = f.CreateEntry(ctx, p1Dir(sub, time.Now(), 0), nil, false, false, nil, false, 255)
		t.Logf("Filer.CreateEntry(/d/sub) as a directory err=%v", err)
		names, err = p1Names(f, ctx, sub)
		t.Logf("Filer.ListDirectoryEntries(/d/sub) of the new directory returned names=%v err=%v", names, err)
		env.p1State("after the re-create and the listing", sub)
		f.DeleteEntryMetaAndData(ctx, env.dir, true, true, false, false, nil, 0)
		env.p1Cleanup(sub)
	})

	t.Run("D_the_candidate_records_route_rearmed_by_an_update", func(t *testing.T) {
		env := newProbeEnv(t, "P1D", "the candidate record's route: /d/sub stored with TtlSec 3, re-armed by Filer.UpdateEntry 2 s later, listed 1.5 s after that; then the recursive delete of /d, re-create of /d/sub, listing")
		f := p1Filer(env.direct)
		sub := env.p1SeedRearmed(f)
		names, err := p1Names(f, ctx, env.dir)
		t.Logf("Filer.ListDirectoryEntries(/d) returned names=%v err=%v", names, err)
		env.p1State("after the listing of /d", sub)
		err = f.DeleteEntryMetaAndData(ctx, env.dir, true, false, false, false, nil, 0)
		t.Logf("Filer.DeleteEntryMetaAndData(/d, isRecursive=true) err=%v", err)
		env.p1State("after the recursive delete of /d", sub)
		err = f.CreateEntry(ctx, p1Dir(sub, time.Now(), 0), nil, false, false, nil, false, 255)
		t.Logf("Filer.CreateEntry(/d/sub) as a directory err=%v", err)
		names, err = p1Names(f, ctx, sub)
		t.Logf("Filer.ListDirectoryEntries(/d/sub) of the new directory returned names=%v err=%v", names, err)
		env.p1State("after the re-create and the listing", sub)
		f.DeleteEntryMetaAndData(ctx, env.dir, true, true, false, false, nil, 0)
		env.p1Cleanup(sub)
	})

	t.Run("E_Redis_expires_the_record_first_for_comparison", func(t *testing.T) {
		env := newProbeEnv(t, "P1E", "the same directory, but the listing comes after Redis has dropped its record (the not-found branch, where the change adds its cleanup); then the same recursive delete, re-create and listing")
		f := p1Filer(env.direct)
		sub := env.p1SeedThroughFiler(f)
		time.Sleep(2500 * time.Millisecond)
		env.p1State("after Redis dropped the record of sub", sub)
		names, err := p1Names(f, ctx, env.dir)
		t.Logf("Filer.ListDirectoryEntries(/d) returned names=%v err=%v", names, err)
		env.p1State("after the listing of /d", sub)
		err = f.DeleteEntryMetaAndData(ctx, env.dir, true, false, false, false, nil, 0)
		t.Logf("Filer.DeleteEntryMetaAndData(/d, isRecursive=true) err=%v", err)
		env.p1State("after the recursive delete of /d", sub)
		err = f.CreateEntry(ctx, p1Dir(sub, time.Now(), 0), nil, false, false, nil, false, 255)
		t.Logf("Filer.CreateEntry(/d/sub) as a directory err=%v", err)
		names, err = p1Names(f, ctx, sub)
		t.Logf("Filer.ListDirectoryEntries(/d/sub) of the new directory returned names=%v err=%v", names, err)
		env.p1State("after the re-create and the listing", sub)
		f.DeleteEntryMetaAndData(ctx, env.dir, true, true, false, false, nil, 0)
		env.p1Cleanup(sub)
	})

	t.Run("G_then_the_lost_directorys_own_path_is_listed_and_read", func(t *testing.T) {
		env := newProbeEnv(t, "P1G", "as A; then Filer.FindEntry(/d/sub), Filer.ListDirectoryEntries(/d/sub) with no record for /d/sub, and Filer.FindEntry(/d/sub/x)")
		f := p1Filer(env.direct)
		sub := env.p1SeedThroughFiler(f)
		names, err := p1Names(f, ctx, env.dir)
		t.Logf("Filer.ListDirectoryEntries(/d) returned names=%v err=%v", names, err)
		env.p1State("after the listing of /d", sub)
		entry, err := f.FindEntry(ctx, sub)
		t.Logf("Filer.FindEntry(/d/sub) found=%v err=%v", entry != nil && err == nil, err)
		names, err = p1Names(f, ctx, sub)
		t.Logf("Filer.ListDirectoryEntries(/d/sub) returned names=%v err=%v", names, err)
		entry, err = f.FindEntry(ctx, sub.Child("x"))
		t.Logf("Filer.FindEntry(/d/sub/x) found=%v err=%v", entry != nil && err == nil, err)
		env.p1State("after those reads", sub)
		env.p1Cleanup(sub)
	})

	t.Run("H_the_documented_way_a_time_to_live_on_the_file_not_the_directory", func(t *testing.T) {
		env := newProbeEnv(t, "P1H", "the documented use of a time-to-live: /d/sub is an ordinary directory (no time-to-live) and its file /d/sub/x has TtlSec 2, created 3 s ago (Redis still holds its record); Filer.ListDirectoryEntries(/d) and (/d/sub); the recursive delete of /d; re-create and listing of /d/sub")
		f := p1Filer(env.direct)
		ctx := context.Background()
		sub := env.dir.Child("sub")
		created := time.Now().Add(-3 * time.Second)
		f.CreateEntry(ctx, p1Dir(env.dir, time.Now(), 0), nil, false, false, nil, false, 255)
		f.CreateEntry(ctx, p1Dir(sub, time.Now(), 0), nil, false, false, nil, false, 255)
		err := f.CreateEntry(ctx, &filer.Entry{FullPath: sub.Child("x"), Attr: filer.Attr{Crtime: created, Mtime: created, Mode: 0644, TtlSec: 2}}, nil, false, false, nil, false, 255)
		ttl, _ := env.direct.TTL(ctx, string(sub.Child("x"))).Result()
		t.Logf("Filer.CreateEntry(/d/sub/x, TtlSec 2, created 3 s ago) err=%v; Redis TTL of its record %v", err, ttl)
		env.p1State("before the listings", sub)
		names, err := p1Names(f, ctx, env.dir)
		t.Logf("Filer.ListDirectoryEntries(/d) returned names=%v err=%v", names, err)
		names, err = p1Names(f, ctx, sub)
		t.Logf("Filer.ListDirectoryEntries(/d/sub) returned names=%v err=%v", names, err)
		env.p1State("after the listings", sub)
		err = f.DeleteEntryMetaAndData(ctx, env.dir, true, false, false, false, nil, 0)
		t.Logf("Filer.DeleteEntryMetaAndData(/d, isRecursive=true) err=%v", err)
		env.p1State("after the recursive delete of /d", sub)
		f.CreateEntry(ctx, p1Dir(sub, time.Now(), 0), nil, false, false, nil, false, 255)
		names, err = p1Names(f, ctx, sub)
		t.Logf("Filer.ListDirectoryEntries(/d/sub) of the new directory returned names=%v err=%v", names, err)
		f.DeleteEntryMetaAndData(ctx, env.dir, true, true, false, false, nil, 0)
		env.p1Cleanup(sub)
	})

	t.Run("F_can_a_directory_get_a_time_to_live_through_the_filer", func(t *testing.T) {
		env := newProbeEnv(t, "P1F", "Filer.CreateEntry of a directory with TtlSec 30; Filer.UpdateEntry of an existing directory with TtlSec 30")
		f := p1Filer(env.direct)
		now := time.Now()
		f.CreateEntry(ctx, p1Dir(env.dir, now, 0), nil, false, false, nil, false, 255)
		made := env.dir.Child("made")
		err := f.CreateEntry(ctx, p1Dir(made, now, 30), nil, false, false, nil, false, 255)
		ttl, _ := env.direct.TTL(ctx, string(made)).Result()
		t.Logf("Filer.CreateEntry(directory, TtlSec 30) err=%v; Redis TTL of its record: %v (-1ns: none)", err, ttl)
		updated := env.dir.Child("updated")
		f.CreateEntry(ctx, p1Dir(updated, now, 0), nil, false, false, nil, false, 255)
		old, _ := f.FindEntry(ctx, updated)
		err = f.UpdateEntry(ctx, old, p1Dir(updated, now, 30))
		ttl, _ = env.direct.TTL(ctx, string(updated)).Result()
		t.Logf("Filer.UpdateEntry(directory, TtlSec 30) err=%v; Redis TTL of its record: %v", err, ttl)
		env.direct.Del(ctx, string(made), string(updated), genDirectoryListKey(string(env.dir)), string(env.dir))
	})
}
