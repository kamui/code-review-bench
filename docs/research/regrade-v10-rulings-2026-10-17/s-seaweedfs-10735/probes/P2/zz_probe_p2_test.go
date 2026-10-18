package redis2

// Probe P2 for seaweedfs/seaweedfs#10735: DeleteEntry (weed/filer/redis2/universal_redis_store.go,
// unchanged by the pull request) deletes the file's record with DEL and then removes its name with
// a separate ZREM. A write of the same path that lands between the two commands.
// Dropped into weed/filer/redis2/ beside the saved helpers, at the commit before the change and at
// its head. It drives the real redis2 store, and in cases C and D a real filer.Filer on each side
// (Filer.DeleteEntryMetaAndData is what the HTTP DELETE handler calls; Filer.CreateEntry is what the
// HTTP upload handler calls), against a real redis-server (PROBE_REDIS_ADDR).
// "Concurrent" requests are not raced. The order is forced: a go-redis hook on the deleter's client
// runs the other request, complete, right after the deleter's DEL of the file's record. Every case
// prints the command order as Redis saw it. No error is injected. Nothing asserts.

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

func p2Filer(client *redis.Client) *filer.Filer {
	store := &Redis2Store{}
	store.Client = client
	store.loadSuperLargeDirectories(nil)
	f := filer.NewFiler(pb.ServerDiscovery{}, nil, "", "", "", "", "", 255, nil)
	f.DirBucketsPath = "/buckets"
	f.SetStore(store)
	return f
}

// p2IsRecordOf matches the DEL of the file's own record (DeleteEntry first deletes "<path>\x00",
// the file's own list of children, then "<path>").
func p2IsRecordOf(env *probeEnv, name string) func(redis.Cmder) bool {
	return func(cmd redis.Cmder) bool {
		args := cmd.Args()
		return len(args) > 1 && args[1] == string(env.dir.Child(name))
	}
}

func (env *probeEnv) p2Seed(name string) {
	env.t.Helper()
	ctx := context.Background()
	store := env.quietStore()
	if err := store.InsertEntry(ctx, probeDirEntry(env.dir)); err != nil {
		env.t.Fatalf("seed directory: %v", err)
	}
	if err := store.InsertEntry(ctx, probeEntry(env.dir.Child(name))); err != nil {
		env.t.Fatalf("seed %s: %v", name, err)
	}
	env.t.Logf("seeded: %q exists, with its record and its name in the directory's list", name)
}

func (env *probeEnv) p2FilerListing() ([]string, error) {
	entries, _, err := p2Filer(env.direct).ListDirectoryEntries(context.Background(), env.dir, "", false, 100, "", "", "")
	names := []string{}
	for _, entry := range entries {
		names = append(names, entry.Name())
	}
	return names, err
}

func (env *probeEnv) p2Order() {
	env.t.Helper()
	env.t.Logf("command order as seen by redis:")
	env.trace.dump(env.t)
	env.notReached()
}

func TestProbeP2(t *testing.T) {
	ctx := context.Background()

	t.Run("A_store_InsertEntry_between_DeleteEntry_DEL_and_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "P2A", "DeleteEntry(f) runs; a complete InsertEntry(f) lands after its DEL of f's record and before its ZREM of the name")
		env.p2Seed("f")
		insert := &probeRule{label: "the insert after the DEL", cmd: "del", when: "after", match: p2IsRecordOf(env, "f"), act: func(context.Context, redis.Cmder) error {
			inserter := env.store(env.client("inserter", env.addr, nil))
			err := inserter.InsertEntry(context.Background(), probeEntry(env.dir.Child("f")))
			env.trace.add("%-9s InsertEntry(f) returned err=%v", "inserter:", err)
			return nil
		}}
		deleter := env.store(env.client("deleter", env.addr, nil, insert))
		err := deleter.DeleteEntry(ctx, env.dir.Child("f"))
		env.trace.add("%-9s DeleteEntry(f) returned err=%v", "deleter:", err)
		env.p2Order()
		env.state("after both requests", "f")
		env.aftermath("f")
	})

	t.Run("B_store_UpdateEntry_between_DeleteEntry_DEL_and_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "P2B", "DeleteEntry(f) runs; a complete UpdateEntry(f) (what the filer runs to overwrite a file it found) lands after its DEL and before its ZREM")
		env.p2Seed("f")
		update := &probeRule{label: "the update after the DEL", cmd: "del", when: "after", match: p2IsRecordOf(env, "f"), act: func(context.Context, redis.Cmder) error {
			updater := env.store(env.client("updater", env.addr, nil))
			err := updater.UpdateEntry(context.Background(), probeEntry(env.dir.Child("f")))
			env.trace.add("%-9s UpdateEntry(f) returned err=%v", "updater:", err)
			return nil
		}}
		deleter := env.store(env.client("deleter", env.addr, nil, update))
		err := deleter.DeleteEntry(ctx, env.dir.Child("f"))
		env.trace.add("%-9s DeleteEntry(f) returned err=%v", "deleter:", err)
		env.p2Order()
		env.state("after both requests", "f")
		env.laterListing("(a fresh listing)")
	})

	t.Run("C_filer_upload_between_the_HTTP_delete_DEL_and_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "P2C", "Filer.DeleteEntryMetaAndData(f) (the HTTP DELETE handler's call) runs; a complete Filer.CreateEntry(f) (the HTTP upload handler's call) lands after its DEL of f's record and before its ZREM")
		env.p2Seed("f")
		upload := &probeRule{label: "the upload after the DEL", cmd: "del", when: "after", match: p2IsRecordOf(env, "f"), act: func(context.Context, redis.Cmder) error {
			writer := p2Filer(env.client("writer", env.addr, nil))
			now := time.Now()
			entry := &filer.Entry{FullPath: env.dir.Child("f"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}, Content: []byte("new")}
			err := writer.CreateEntry(context.Background(), entry, nil, false, false, nil, false, 255)
			env.trace.add("%-9s Filer.CreateEntry(f) returned err=%v", "writer:", err)
			return nil
		}}
		deleter := p2Filer(env.client("deleter", env.addr, nil, upload))
		err := deleter.DeleteEntryMetaAndData(ctx, env.dir.Child("f"), false, false, false, false, nil, 0)
		env.trace.add("%-9s Filer.DeleteEntryMetaAndData(f) returned err=%v", "deleter:", err)
		env.p2Order()
		env.state("after both requests", "f")
		entry, findErr := p2Filer(env.direct).FindEntry(ctx, env.dir.Child("f"))
		content := ""
		if entry != nil {
			content = string(entry.Content)
		}
		t.Logf("Filer.FindEntry(f) by full path: found=%v content=%q err=%v", entry != nil && findErr == nil, content, findErr)
		names, listErr := env.p2FilerListing()
		t.Logf("Filer.ListDirectoryEntries(dir): names=%v err=%v", names, listErr)
		names, listErr = env.p2FilerListing()
		t.Logf("Filer.ListDirectoryEntries(dir) again: names=%v err=%v", names, listErr)
		again := &filer.Entry{FullPath: env.dir.Child("f"), Attr: filer.Attr{Crtime: time.Now(), Mtime: time.Now(), Mode: 0644}, Content: []byte("newer")}
		t.Logf("Filer.CreateEntry(f) again (an overwrite: the filer finds f and updates it): err=%v", p2Filer(env.direct).CreateEntry(ctx, again, nil, false, false, nil, false, 255))
		names, listErr = env.p2FilerListing()
		t.Logf("Filer.ListDirectoryEntries(dir) after the overwrite: names=%v err=%v", names, listErr)
		t.Logf("Filer.DeleteEntryMetaAndData(dir, recursive) err=%v", p2Filer(env.direct).DeleteEntryMetaAndData(ctx, env.dir, true, false, false, false, nil, 0))
		env.state("after the recursive delete of the directory", "f")
	})

	t.Run("D_control_the_same_two_requests_one_after_the_other", func(t *testing.T) {
		env := newProbeEnv(t, "P2D", "the same delete, then the same upload, not overlapping")
		env.p2Seed("f")
		deleter := p2Filer(env.client("deleter", env.addr, nil))
		err := deleter.DeleteEntryMetaAndData(ctx, env.dir.Child("f"), false, false, false, false, nil, 0)
		env.trace.add("%-9s Filer.DeleteEntryMetaAndData(f) returned err=%v", "deleter:", err)
		writer := p2Filer(env.client("writer", env.addr, nil))
		now := time.Now()
		err = writer.CreateEntry(ctx, &filer.Entry{FullPath: env.dir.Child("f"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}, Content: []byte("new")}, nil, false, false, nil, false, 255)
		env.trace.add("%-9s Filer.CreateEntry(f) returned err=%v", "writer:", err)
		env.p2Order()
		env.state("after both requests", "f")
		names, listErr := env.p2FilerListing()
		t.Logf("Filer.ListDirectoryEntries(dir): names=%v err=%v", names, listErr)
	})

	t.Run("E_filer_read_of_an_expired_file_deletes_it_while_an_upload_lands", func(t *testing.T) {
		env := newProbeEnv(t, "P2E", "f is past its creation time plus time-to-live and Redis still holds its record; Filer.FindEntry(f) (any read through the filer) deletes it through DeleteEntry; a complete Filer.CreateEntry(f) (an upload) lands after that DEL and before its ZREM")
		store := env.quietStore()
		if err := store.InsertEntry(ctx, probeDirEntry(env.dir)); err != nil {
			t.Fatalf("seed directory: %v", err)
		}
		created := time.Now().Add(-10 * time.Second)
		if err := store.InsertEntry(ctx, &filer.Entry{FullPath: env.dir.Child("f"), Attr: filer.Attr{Crtime: created, Mtime: created, Mode: 0644, TtlSec: 5}}); err != nil {
			t.Fatalf("seed f: %v", err)
		}
		t.Logf("seeded: f's creation time is 10 s ago, its time-to-live 5 s; Redis holds its record for 5 s more")
		upload := &probeRule{label: "the upload after the DEL", cmd: "del", when: "after", match: p2IsRecordOf(env, "f"), act: func(context.Context, redis.Cmder) error {
			writer := p2Filer(env.client("writer", env.addr, nil))
			now := time.Now()
			entry := &filer.Entry{FullPath: env.dir.Child("f"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}, Content: []byte("new")}
			err := writer.CreateEntry(context.Background(), entry, nil, false, false, nil, false, 255)
			env.trace.add("%-9s Filer.CreateEntry(f) returned err=%v", "writer:", err)
			return nil
		}}
		reader := p2Filer(env.client("reader", env.addr, nil, upload))
		entry, err := reader.FindEntry(ctx, env.dir.Child("f"))
		env.trace.add("%-9s Filer.FindEntry(f) returned found=%v err=%v", "reader:", entry != nil, err)
		env.p2Order()
		env.state("after both requests", "f")
		found, findErr := p2Filer(env.direct).FindEntry(ctx, env.dir.Child("f"))
		t.Logf("Filer.FindEntry(f) by full path afterwards: found=%v err=%v", found != nil && findErr == nil, findErr)
		names, listErr := env.p2FilerListing()
		t.Logf("Filer.ListDirectoryEntries(dir): names=%v err=%v", names, listErr)
	})
}

var _ = os.ModeDir
var _ = util.FullPath("")
