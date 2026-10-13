package redis2

// Probe M2 for seaweedfs/seaweedfs#10735: the listing's logical-expiry branch (DEL of the value,
// then ZREM of the name) against an insert of the same path.
// Dropped into weed/filer/redis2/ beside the saved helpers, at the commit before the change, at
// the pull request's first commit and at its head. It drives the real redis2 store, and in cases
// E and F the real filer.Filer, against a real redis-server (PROBE_REDIS_ADDR).
// "Concurrent" requests are not raced. The order is forced: a go-redis hook on the listing's
// client runs the other request, complete, at a chosen point between two of the listing's
// commands. Every case prints the command order as Redis saw it. Nothing asserts.

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

func probeGotValue(cmd redis.Cmder) bool { return cmd.Err() == nil }

// seedLogicallyExpired leaves a file the way a time-to-live file is left when it was stored again
// before its time ran out: the creation time plus the time-to-live is in the past, and Redis still
// holds the value because the second write re-armed Redis's own timer.
func (env *probeEnv) seedLogicallyExpired(name string) {
	env.t.Helper()
	ctx := context.Background()
	store := env.quietStore()
	if err := store.InsertEntry(ctx, probeDirEntry(env.dir)); err != nil {
		env.t.Fatalf("seed directory: %v", err)
	}
	created := time.Now().Add(-10 * time.Second)
	entry := &filer.Entry{FullPath: env.dir.Child(name), Attr: filer.Attr{Crtime: created, Mtime: created, Mode: 0644, TtlSec: 5}}
	if err := store.InsertEntry(ctx, entry); err != nil {
		env.t.Fatalf("seed %s: %v", entry.FullPath, err)
	}
	env.t.Logf("seeded: %q is in the directory index; its value is in Redis (Redis timer: 5 s from now); its creation time + TtlSec was 5 s ago", name)
}

// insertAt places a complete InsertEntry of the same path, with no time-to-live, at one point of
// the listing.
func (env *probeEnv) insertAt(label, cmd, when string, match func(redis.Cmder) bool, name string) *probeRule {
	return &probeRule{label: label, cmd: cmd, when: when, match: match, act: func(ctx context.Context, _ redis.Cmder) error {
		inserter := env.store(env.client("inserter", env.addr, nil))
		err := inserter.InsertEntry(context.Background(), probeEntry(env.dir.Child(name)))
		env.trace.add("%-9s InsertEntry(%s) returned err=%v", "inserter:", name, err)
		return nil
	}}
}

func probeFiler(store *Redis2Store) *filer.Filer {
	f := filer.NewFiler(pb.ServerDiscovery{}, nil, "", "", "", "", "", 255, nil)
	f.DirBucketsPath = "/buckets"
	f.SetStore(store)
	return f
}

func (env *probeEnv) filerOn(client *redis.Client) *filer.Filer {
	store := &Redis2Store{}
	store.Client = client
	store.loadSuperLargeDirectories(nil)
	return probeFiler(store)
}

// createAt places a complete Filer.CreateEntry of the same path (what an upload of the file runs)
// at one point of the listing.
func (env *probeEnv) createAt(label, cmd, when string, match func(redis.Cmder) bool, name string) *probeRule {
	return &probeRule{label: label, cmd: cmd, when: when, match: match, act: func(ctx context.Context, _ redis.Cmder) error {
		writer := env.filerOn(env.client("writer", env.addr, nil))
		now := time.Now()
		entry := &filer.Entry{FullPath: env.dir.Child(name), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}, Content: []byte("new")}
		err := writer.CreateEntry(context.Background(), entry, nil, false, false, nil, false, 255)
		env.trace.add("%-9s Filer.CreateEntry(%s) returned err=%v", "writer:", name, err)
		return nil
	}}
}

func isValueOf(env *probeEnv, name string) func(redis.Cmder) bool {
	return func(cmd redis.Cmder) bool {
		args := cmd.Args()
		return len(args) > 1 && args[1] == string(env.dir.Child(name))
	}
}

func (env *probeEnv) filerListing(f *filer.Filer) ([]string, error) {
	entries, _, err := f.ListDirectoryEntries(context.Background(), env.dir, "", false, 100, "", "", "")
	names := []string{}
	for _, entry := range entries {
		names = append(names, entry.Name())
	}
	return names, err
}

func TestProbeM2(t *testing.T) {
	ctx := context.Background()

	t.Run("A_insert_between_the_DEL_and_the_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "M2A", "a listing finds f logically expired; a complete InsertEntry of f lands after the listing's DEL and before its ZREM")
		env.seedLogicallyExpired("f")
		lister := env.store(env.client("lister", env.addr, nil, env.insertAt("the insert after the DEL", "del", "after", nil, "f")))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.aftermath("f")
	})

	t.Run("B_insert_between_the_GET_and_the_DEL", func(t *testing.T) {
		env := newProbeEnv(t, "M2B", "a listing reads f, logically expired; a complete InsertEntry of f lands after the listing's GET and before its DEL")
		env.seedLogicallyExpired("f")
		lister := env.store(env.client("lister", env.addr, nil, env.insertAt("the insert after the GET", "get", "after", probeGotValue, "f")))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		entry, findErr := env.quietStore().FindEntry(ctx, env.dir.Child("f"))
		t.Logf("FindEntry(f): found=%v err=%v", entry != nil && findErr == nil, findErr)
		env.laterListing("(a fresh listing)")
	})

	t.Run("C_no_other_request", func(t *testing.T) {
		env := newProbeEnv(t, "M2C", "a listing finds f logically expired and nothing else happens")
		env.seedLogicallyExpired("f")
		lister := env.store(env.client("lister", env.addr, nil))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
	})

	t.Run("D_neighbouring_branch_insert_after_the_GET_found_nothing", func(t *testing.T) {
		env := newProbeEnv(t, "M2D", "the not-found branch: the value of f is gone; a complete InsertEntry of f lands right after the listing's GET came back empty")
		env.seedOrphan("f")
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f")))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("(a fresh listing)")
	})

	t.Run("E_through_the_filer_upload_between_the_DEL_and_the_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "M2E", "Filer.ListDirectoryEntries finds f logically expired; a complete Filer.CreateEntry of f (an upload) lands after the listing's DEL of f and before its ZREM")
		env.seedLogicallyExpired("f")
		lister := env.filerOn(env.client("lister", env.addr, nil, env.createAt("the upload after the DEL", "del", "after", isValueOf(env, "f"), "f")))
		names, err := env.filerListing(lister)
		env.report(names, err)
		env.state("after the listing", "f")
		entry, findErr := env.filerOn(env.direct).FindEntry(ctx, env.dir.Child("f"))
		t.Logf("Filer.FindEntry(f) by full path: found=%v err=%v", entry != nil && findErr == nil, findErr)
		env.laterListing("(a fresh listing)")
		env.laterListing("(a second fresh listing)")
	})

	t.Run("F_through_the_filer_upload_between_the_GET_and_the_DEL", func(t *testing.T) {
		env := newProbeEnv(t, "M2F", "Filer.ListDirectoryEntries reads f, logically expired; a complete Filer.CreateEntry of f (an upload) lands after the listing's GET of f and before its DEL")
		env.seedLogicallyExpired("f")
		lister := env.filerOn(env.client("lister", env.addr, nil, env.createAt("the upload after the GET", "get", "after", func(cmd redis.Cmder) bool { return probeGotValue(cmd) && isValueOf(env, "f")(cmd) }, "f")))
		names, err := env.filerListing(lister)
		env.report(names, err)
		env.state("after the listing", "f")
		entry, findErr := env.filerOn(env.direct).FindEntry(ctx, env.dir.Child("f"))
		t.Logf("Filer.FindEntry(f) by full path: found=%v err=%v", entry != nil && findErr == nil, findErr)
		env.laterListing("(a fresh listing)")
	})

	t.Run("G_through_the_filer_an_upload_stored_again_reaches_the_branch", func(t *testing.T) {
		env := newProbeEnv(t, "M2G", "how a listing reaches the branch with no forcing: Filer.CreateEntry of f with TtlSec 3, the same again 2 s later, a listing 1.5 s after that")
		writer := env.filerOn(env.client("writer", env.addr, nil))
		mk := func() *filer.Entry {
			now := time.Now()
			return &filer.Entry{FullPath: env.dir.Child("f"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644, TtlSec: 3}, Content: []byte("x")}
		}
		dirEntry := &filer.Entry{FullPath: env.dir, Attr: filer.Attr{Crtime: time.Now(), Mtime: time.Now(), Mode: os.ModeDir | 0755}}
		t.Logf("create directory: err=%v", writer.CreateEntry(ctx, dirEntry, nil, false, false, nil, false, 255))
		t.Logf("first store: err=%v", writer.CreateEntry(ctx, mk(), nil, false, false, nil, false, 255))
		time.Sleep(2 * time.Second)
		t.Logf("second store, 2 s later: err=%v", writer.CreateEntry(ctx, mk(), nil, false, false, nil, false, 255))
		time.Sleep(1500 * time.Millisecond)
		ttl, _ := env.direct.TTL(ctx, string(env.dir.Child("f"))).Result()
		t.Logf("3.5 s after the first store: Redis still holds the value, its timer has %v left", ttl.Round(100*time.Millisecond))
		lister := env.filerOn(env.client("lister", env.addr, nil))
		names, err := env.filerListing(lister)
		env.report(names, err)
		env.state("after the listing", "f")
	})
}

var _ = util.FullPath("")
