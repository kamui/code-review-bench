package redis2

// Probe M1 for seaweedfs/seaweedfs#10735: a listing page filled by names whose values are gone.
// Dropped into weed/filer/redis2/ at the commit before the change and at its head, beside the
// saved helpers. It drives the real redis2 store and the real filer.Filer of each commit against a
// real redis-server (PROBE_REDIS_ADDR). Nothing is raced and nothing is injected: every dead name
// is made the way the pull request's own test makes one, an entry inserted with TtlSec 1 that
// Redis then expires. Nothing asserts; the printed lines are the result.

import (
	"context"
	"fmt"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"

	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/pb"
	"github.com/seaweedfs/seaweedfs/weed/s3api/s3_constants"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

type m1Env struct {
	t      *testing.T
	ctx    context.Context
	direct *redis.Client
	store  *Redis2Store
	filer  *filer.Filer
	root   util.FullPath
}

func newM1Env(t *testing.T) *m1Env {
	t.Helper()
	addr := probeAddr(t)
	direct := redis.NewClient(&redis.Options{Addr: addr})
	if err := direct.Ping(context.Background()).Err(); err != nil {
		t.Fatalf("connect to %s: %v", addr, err)
	}
	store := &Redis2Store{}
	store.Client = redis.NewClient(&redis.Options{Addr: addr})
	store.loadSuperLargeDirectories(nil)
	f := filer.NewFiler(pb.ServerDiscovery{}, nil, "", "", "", "", "", 255, nil)
	f.DirBucketsPath = "/buckets"
	f.SetStore(store)
	env := &m1Env{t: t, ctx: context.Background(), direct: direct, store: store, filer: f,
		root: util.FullPath(fmt.Sprintf("/buckets/m1probe%d", time.Now().UnixNano()))}
	t.Cleanup(func() {
		keys, _ := direct.Keys(context.Background(), string(env.root)+"*").Result()
		for len(keys) > 0 {
			n := len(keys)
			if n > 500 {
				n = 500
			}
			direct.Del(context.Background(), keys[:n]...)
			keys = keys[n:]
		}
		direct.Close()
		store.Client.Close()
	})
	return env
}

func (env *m1Env) mkdir(dir util.FullPath) {
	env.t.Helper()
	now := time.Now()
	entry := &filer.Entry{FullPath: dir, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: os.ModeDir | 0755}}
	if err := env.filer.CreateEntry(env.ctx, entry, nil, false, false, nil, false, 255); err != nil {
		env.t.Fatalf("create directory %s: %v", dir, err)
	}
}

// seed creates `dead` files named a-0000.. with TtlSec 1 and `live` files named b-0000.. with no
// TTL, through the store's InsertEntry. The a- names sort first.
func (env *m1Env) seed(dir util.FullPath, dead, live int) {
	env.t.Helper()
	env.mkdir(dir)
	now := time.Now()
	for i := 0; i < dead; i++ {
		entry := &filer.Entry{FullPath: dir.Child(fmt.Sprintf("a-%04d", i)), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644, TtlSec: 1}}
		if err := env.store.InsertEntry(env.ctx, entry); err != nil {
			env.t.Fatalf("seed: %v", err)
		}
	}
	for i := 0; i < live; i++ {
		entry := &filer.Entry{FullPath: dir.Child(fmt.Sprintf("b-%04d", i)), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}}
		if err := env.store.InsertEntry(env.ctx, entry); err != nil {
			env.t.Fatalf("seed: %v", err)
		}
	}
}

// state prints what Redis holds for the directory: how many names its list has, and how many of
// the a- and b- names still have a value.
func (env *m1Env) state(label string, dir util.FullPath) {
	env.t.Helper()
	indexKey := genDirectoryListKey(string(dir))
	members, _ := env.direct.ZRange(env.ctx, indexKey, 0, -1).Result()
	deadNames, liveNames, deadValues, liveValues := 0, 0, 0, 0
	for _, name := range members {
		exists, _ := env.direct.Exists(env.ctx, string(dir.Child(name))).Result()
		if strings.HasPrefix(name, "a-") {
			deadNames++
			deadValues += int(exists)
		} else {
			liveNames++
			liveValues += int(exists)
		}
	}
	dirExists, _ := env.direct.Exists(env.ctx, string(dir)).Result()
	indexExists, _ := env.direct.Exists(env.ctx, indexKey).Result()
	env.t.Logf("  REDIS %s: directory record exists=%d; list key exists=%d with %d names (a-: %d names, %d values; b-: %d names, %d values)",
		label, dirExists, indexExists, len(members), deadNames, deadValues, liveNames, liveValues)
}

func (env *m1Env) liveValues(dir util.FullPath, live int) int {
	found := 0
	for i := 0; i < live; i++ {
		exists, _ := env.direct.Exists(env.ctx, string(dir.Child(fmt.Sprintf("b-%04d", i)))).Result()
		found += int(exists)
	}
	return found
}

func summarize(names []string) string {
	if len(names) == 0 {
		return "0 entries"
	}
	return fmt.Sprintf("%d entries (%s .. %s)", len(names), names[0], names[len(names)-1])
}

func (env *m1Env) storeList(dir util.FullPath, start string, limit int64) ([]string, string) {
	names := []string{}
	last, err := env.store.ListDirectoryEntries(env.ctx, dir, start, false, limit, func(entry *filer.Entry) (bool, error) {
		names = append(names, entry.Name())
		return true, nil
	})
	if err != nil {
		env.t.Logf("  store listing error: %v", err)
	}
	return names, last
}

func (env *m1Env) filerList(dir util.FullPath, start string, limit int64) ([]string, bool) {
	entries, hasMore, err := env.filer.ListDirectoryEntries(env.ctx, dir, start, false, limit, "", "", "")
	if err != nil {
		env.t.Logf("  filer listing error: %v", err)
	}
	names := []string{}
	for _, entry := range entries {
		names = append(names, entry.Name())
	}
	return names, hasMore
}

func TestProbeM1(t *testing.T) {
	env := newM1Env(t)
	root := env.root
	env.mkdir(root)

	dirs := map[string]util.FullPath{}
	for _, name := range []string{"store-page", "store-backlog", "store-follow", "filer-page", "filer-one-dead", "filer-count", "is-empty", "delete-empty-parents", "listing-store-expiry", "listing-s3-expiry-1", "listing-s3-expiry-2", "rmdir"} {
		dirs[name] = root.Child(name)
	}

	env.seed(dirs["store-page"], 100, 50)
	env.seed(dirs["store-follow"], 100, 50)
	env.seed(dirs["store-backlog"], 250, 5)
	env.seed(dirs["filer-page"], 101, 50)
	env.seed(dirs["filer-one-dead"], 1, 300)
	env.seed(dirs["filer-count"], 1001, 5)
	env.seed(dirs["is-empty"], 1, 1)
	env.seed(dirs["delete-empty-parents"], 1, 2)
	env.seed(dirs["rmdir"], 1025, 3)

	t.Logf("waiting 2.5 s for Redis to expire every value written with TtlSec 1")
	time.Sleep(2500 * time.Millisecond)

	t.Logf("=== M1-A store.ListDirectoryEntries(limit 100): 100 dead names sort first, 50 live names follow")
	env.state("before", dirs["store-page"])
	for call := 1; call <= 3; call++ {
		names, last := env.storeList(dirs["store-page"], "", 100)
		t.Logf("  call %d from the start: %s, lastFileName=%q", call, summarize(names), last)
	}
	env.state("after", dirs["store-page"])

	t.Logf("=== M1-K store.ListDirectoryEntries(limit 100), each call from the start: 250 dead names sort first, 5 live names follow")
	for call := 1; call <= 4; call++ {
		names, last := env.storeList(dirs["store-backlog"], "", 100)
		t.Logf("  call %d from the start: %s, lastFileName=%q", call, summarize(names), last)
	}
	env.state("after", dirs["store-backlog"])

	t.Logf("=== M1-B the same directory shape, a caller that asks again from lastFileName")
	names, last := env.storeList(dirs["store-follow"], "", 100)
	t.Logf("  page 1: %s, lastFileName=%q", summarize(names), last)
	names, last = env.storeList(dirs["store-follow"], last, 100)
	t.Logf("  page 2 from lastFileName: %s, lastFileName=%q", summarize(names), last)

	t.Logf("=== M1-C Filer.ListDirectoryEntries(limit 100), which asks the store for 101: 101 dead names first, 50 live follow")
	env.state("before", dirs["filer-page"])
	for call := 1; call <= 3; call++ {
		names, hasMore := env.filerList(dirs["filer-page"], "", 100)
		t.Logf("  call %d from the start: %s, hasMore=%v", call, summarize(names), hasMore)
	}
	env.state("after", dirs["filer-page"])

	t.Logf("=== M1-D Filer.ListDirectoryEntries(limit 100): ONE dead name first, 300 live follow")
	env.state("before", dirs["filer-one-dead"])
	for call := 1; call <= 2; call++ {
		names, hasMore := env.filerList(dirs["filer-one-dead"], "", 100)
		t.Logf("  call %d from the start: %s, hasMore=%v (300 live names exist)", call, summarize(names), hasMore)
	}

	t.Logf("=== M1-E Filer.CountDirectoryEntries(limit 1000), the count the empty-folder cleaner uses: 1001 dead first, 5 live follow")
	for call := 1; call <= 2; call++ {
		count, err := env.filer.CountDirectoryEntries(env.ctx, dirs["filer-count"], 1000)
		t.Logf("  call %d: count=%d err=%v", call, count, err)
	}

	t.Logf("=== M1-F Filer.IsDirectoryEmpty (store listing with limit 1): one dead name first, one live name follows")
	env.state("before", dirs["is-empty"])
	for call := 1; call <= 2; call++ {
		empty, err := env.filer.IsDirectoryEmpty(env.ctx, dirs["is-empty"])
		t.Logf("  call %d: isEmpty=%v err=%v", call, empty, err)
	}
	env.state("after", dirs["is-empty"])

	t.Logf("=== M1-G Filer.DeleteEmptyParentDirectories called directly on a directory with one dead name first and two live names")
	env.state("before", dirs["delete-empty-parents"])
	env.filer.DeleteEmptyParentDirectories(env.ctx, dirs["delete-empty-parents"], util.FullPath("/buckets"))
	env.state("after", dirs["delete-empty-parents"])
	t.Logf("  live values still in Redis: %d of 2", env.liveValues(dirs["delete-empty-parents"], 2))
	_, findErr := env.filer.FindEntry(env.ctx, dirs["delete-empty-parents"])
	t.Logf("  FindEntry(directory): err=%v", findErr)
	_, findErr = env.filer.FindEntry(env.ctx, dirs["delete-empty-parents"].Child("b-0000"))
	t.Logf("  FindEntry(b-0000 by full path): err=%v", findErr)
	names, hasMore := env.filerList(dirs["delete-empty-parents"], "", 100)
	t.Logf("  listing of the directory afterwards: %s, hasMore=%v", summarize(names), hasMore)

	// The only caller of DeleteEmptyParentDirectories is filer.go doListDirectoryEntries, and only
	// when the filer's own callback met an expired entry and no valid one. The next three cases ask
	// whether a listing reaches it with this store.
	now := time.Now()
	t.Logf("=== M1-H can a listing reach that caller? a file past Crtime+TtlSec whose value Redis still holds, then a dead name, then a live one")
	dir := dirs["listing-store-expiry"]
	env.mkdir(dir)
	insert := func(entry *filer.Entry) {
		if err := env.store.InsertEntry(env.ctx, entry); err != nil {
			t.Fatalf("seed: %v", err)
		}
	}
	insert(&filer.Entry{FullPath: dir.Child("a-0000"), Attr: filer.Attr{Crtime: now.Add(-10 * time.Second), Mtime: now.Add(-10 * time.Second), Mode: 0644, TtlSec: 5}})
	insert(&filer.Entry{FullPath: dir.Child("a-0001"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}})
	env.direct.Del(env.ctx, string(dir.Child("a-0001")))
	insert(&filer.Entry{FullPath: dir.Child("b-0000"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}})
	env.state("before", dir)
	names, hasMore = env.filerList(dir, "", 1)
	t.Logf("  Filer.ListDirectoryEntries(limit 1): %s, hasMore=%v", summarize(names), hasMore)
	env.state("after", dir)

	s3Expired := func(path util.FullPath) *filer.Entry {
		return &filer.Entry{FullPath: path, Attr: filer.Attr{Crtime: now, Mtime: now.Add(-10 * time.Second), Mode: 0644, TtlSec: 5},
			Extended: map[string][]byte{s3_constants.SeaweedFSExpiresS3: []byte("true")}}
	}
	for _, variant := range []struct {
		key  string
		dead int
	}{{"listing-s3-expiry-1", 1}, {"listing-s3-expiry-2", 2}} {
		t.Logf("=== M1-I (%d dead) a file the filer's own check calls expired and the store's does not (S3 expiry flag, Mtime 10 s before Crtime, TtlSec 5), then %d dead name(s), then a live one", variant.dead, variant.dead)
		dir = dirs[variant.key]
		env.mkdir(dir)
		insert(s3Expired(dir.Child("a-0000")))
		for i := 1; i <= variant.dead; i++ {
			path := dir.Child(fmt.Sprintf("a-%04d", i))
			insert(&filer.Entry{FullPath: path, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}})
			env.direct.Del(env.ctx, string(path))
		}
		insert(&filer.Entry{FullPath: dir.Child("b-0000"), Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}})
		env.state("before", dir)
		names, hasMore = env.filerList(dir, "", 1)
		t.Logf("  Filer.ListDirectoryEntries(limit 1): %s, hasMore=%v", summarize(names), hasMore)
		env.state("after", dir)
		t.Logf("  live value b-0000 still in Redis: %d of 1", env.liveValues(dir, 1))
		_, findErr = env.filer.FindEntry(env.ctx, dir)
		t.Logf("  FindEntry(directory): err=%v", findErr)
		names, hasMore = env.filerList(dir, "", 100)
		t.Logf("  a later listing: %s, hasMore=%v", summarize(names), hasMore)
	}

	t.Logf("=== M1-J a delete of the directory WITHOUT the recursive flag (PaginationSize 1024, so the store is asked for 1025): 1025 dead names first, 3 live follow")
	dir = dirs["rmdir"]
	env.state("before", dir)
	err := env.filer.DeleteEntryMetaAndData(env.ctx, dir, false, false, false, false, nil, 0)
	t.Logf("  DeleteEntryMetaAndData(isRecursive=false): err=%v", err)
	env.state("after", dir)
	t.Logf("  live values still in Redis: %d of 3", env.liveValues(dir, 3))
}
