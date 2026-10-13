package redis2

// Grouping probe for seaweedfs/seaweedfs#10735, round of 2026-10-13.
// One short failing case for each known problem that can be run on a single Redis (GT-s2, GT-s3
// at both of its sites, GT-s4 in both of its forms, GT-s5) and for each candidate group of this
// round (M1 three ways, M2a, M2b). It is run against the head and against the head with one or
// more small patches (../grouping/variants.py), to see which patch cures which case.
// It uses the saved helpers. The order of commands is forced with a go-redis hook, as in the
// other probes. Each case ends in one line "VERDICT <case>: FAILS|OK|NOT REACHED - <what was seen>".
// GT-s1 needs a Redis cluster with replica reads and is not run.

import (
	"context"
	"fmt"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"

	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/pb"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

func verdict(t *testing.T, name string, fails bool, format string, args ...any) {
	t.Helper()
	word := "OK"
	if fails {
		word = "FAILS"
	}
	t.Logf("VERDICT %s: %s - %s", name, word, fmt.Sprintf(format, args...))
}

func groupingFiler(client *redis.Client) *filer.Filer {
	store := &Redis2Store{}
	store.Client = client
	store.loadSuperLargeDirectories(nil)
	f := filer.NewFiler(pb.ServerDiscovery{}, nil, "", "", "", "", "", 255, nil)
	f.DirBucketsPath = "/buckets"
	f.SetStore(store)
	return f
}

func (env *probeEnv) listNames(limit int64) []string {
	names := []string{}
	env.quietStore().ListDirectoryEntries(context.Background(), env.dir, "", true, limit, func(entry *filer.Entry) (bool, error) {
		names = append(names, entry.Name())
		return true, nil
	})
	return names
}

func (env *probeEnv) valueExists(name string) int64 {
	n, _ := env.direct.Exists(context.Background(), string(env.dir.Child(name))).Result()
	return n
}

// seedDeadThenLive puts `dead` names a-0.. whose values are gone in front of `live` names b-0..
func (env *probeEnv) seedDeadThenLive(dead, live int) {
	ctx := context.Background()
	store := env.quietStore()
	store.InsertEntry(ctx, probeDirEntry(env.dir))
	for i := 0; i < dead; i++ {
		path := env.dir.Child(fmt.Sprintf("a-%d", i))
		store.InsertEntry(ctx, probeEntry(path))
		env.direct.Del(ctx, string(path))
	}
	for i := 0; i < live; i++ {
		store.InsertEntry(ctx, probeEntry(env.dir.Child(fmt.Sprintf("b-%d", i))))
	}
}

func (env *probeEnv) seedExpiredWithValue(name string) {
	ctx := context.Background()
	store := env.quietStore()
	store.InsertEntry(ctx, probeDirEntry(env.dir))
	created := time.Now().Add(-10 * time.Second)
	store.InsertEntry(ctx, &filer.Entry{FullPath: env.dir.Child(name), Attr: filer.Attr{Crtime: created, Mtime: created, Mode: 0644, TtlSec: 5}})
}

func (env *probeEnv) insertRule(label, cmd, when string, match func(redis.Cmder) bool, name string, ran *bool) *probeRule {
	return &probeRule{label: label, cmd: cmd, when: when, match: match, act: func(context.Context, redis.Cmder) error {
		if *ran {
			return nil
		}
		*ran = true
		inserter := env.store(env.client("inserter", env.addr, nil))
		err := inserter.InsertEntry(context.Background(), probeEntry(env.dir.Child(name)))
		env.trace.add("%-9s InsertEntry(%s) returned err=%v", "inserter:", name, err)
		return nil
	}}
}

func TestProbeGrouping(t *testing.T) {
	ctx := context.Background()

	t.Run("GT-s2_update_after_the_cleanup", func(t *testing.T) {
		env := newProbeEnv(t, "KS2", "a writer has read f; the value is lost; a listing runs; the writer's UpdateEntry writes the value back")
		store := env.quietStore()
		store.InsertEntry(ctx, probeDirEntry(env.dir))
		store.InsertEntry(ctx, probeEntry(env.dir.Child("f")))
		pending, _ := store.FindEntry(ctx, env.dir.Child("f"))
		env.direct.Del(ctx, string(env.dir.Child("f")))
		env.listNames(100)
		err := store.UpdateEntry(ctx, pending)
		names := env.listNames(100)
		verdict(t, "GT-s2", len(names) == 0, "UpdateEntry err=%v; value exists=%d; a later listing shows %v", err, env.valueExists("f"), names)
	})

	t.Run("GT-s3_site1_put_back_fails", func(t *testing.T) {
		env := newProbeEnv(t, "KS3A", "re-create right after the listing's empty GET, then the listing's first ZADD NX fails (injected in the client)")
		env.seedOrphan("f")
		fail := &probeRule{label: "the failed put-back", cmd: "zadd", when: "before", act: func(context.Context, redis.Cmder) error { return errProbeInjected }}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), fail))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		names := env.listNames(100)
		verdict(t, "GT-s3 site 1", len(names) == 0, "value exists=%d; a later listing shows %v", env.valueExists("f"), names)
	})

	t.Run("GT-s3_site2_removal_ran_and_is_reported_failed", func(t *testing.T) {
		env := newProbeEnv(t, "KS3B", "re-create right after the listing's empty GET, then Redis runs the ZREM and the client is handed an error for it (injected in the client)")
		env.seedOrphan("f")
		fail := &probeRule{label: "the removal reported as failed", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error { return errProbeInjected }}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), fail))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		names := env.listNames(100)
		verdict(t, "GT-s3 site 2", len(names) == 0, "value exists=%d; a later listing shows %v", env.valueExists("f"), names)
	})

	t.Run("GT-s4_recursive_delete_in_the_gap", func(t *testing.T) {
		env := newProbeEnv(t, "KS4A", "re-create right after the listing's empty GET, then a recursive delete of the directory right after the listing's removal step")
		env.seedOrphan("f")
		ran := false
		del := func(cmd string) *probeRule {
			return &probeRule{label: "the recursive delete after " + cmd, cmd: cmd, when: "after", act: func(context.Context, redis.Cmder) error {
				if !ran {
					ran = true
					env.recursiveDelete()
				}
				return nil
			}}
		}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), del("zrem"), del("eval")))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		if !ran {
			t.Logf("VERDICT GT-s4 recursive delete: NOT REACHED - the listing issued neither a ZREM nor an EVAL")
			return
		}
		verdict(t, "GT-s4 recursive delete", env.valueExists("f") == 1, "after the delete reported success the value of f exists=%d", env.valueExists("f"))
	})

	t.Run("GT-s4_listing_in_the_gap", func(t *testing.T) {
		env := newProbeEnv(t, "KS4B", "re-create right after the listing's empty GET, then a second listing right after the first one's removal step")
		env.seedOrphan("f")
		ran := false
		var seen []string
		second := func(cmd string) *probeRule {
			return &probeRule{label: "the second listing after " + cmd, cmd: cmd, when: "after", act: func(context.Context, redis.Cmder) error {
				if !ran {
					ran = true
					seen = env.listNames(100)
					env.trace.add("%-9s a second listing saw names=%v", "other:", seen)
				}
				return nil
			}}
		}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), second("zrem"), second("eval")))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		if !ran {
			t.Logf("VERDICT GT-s4 listing in the gap: NOT REACHED - the listing issued neither a ZREM nor an EVAL")
			return
		}
		verdict(t, "GT-s4 listing in the gap", len(seen) == 0, "the file exists (value exists=%d) and the second listing saw %v", env.valueExists("f"), seen)
	})

	t.Run("GT-s5_directory_record_lost", func(t *testing.T) {
		env := newProbeEnv(t, "KS5", "a subdirectory's own record is lost, its list of children survives; the parent is listed, then DeleteFolderChildren(parent)")
		store := env.quietStore()
		sub := env.dir.Child("sub")
		store.InsertEntry(ctx, probeDirEntry(env.dir))
		store.InsertEntry(ctx, probeDirEntry(sub))
		store.InsertEntry(ctx, probeEntry(sub.Child("f")))
		env.direct.Del(ctx, string(sub))
		env.listNames(100)
		store.DeleteFolderChildren(ctx, env.dir)
		left, _ := env.direct.Exists(ctx, genDirectoryListKey(string(sub))).Result()
		verdict(t, "GT-s5", left == 1, "after the delete the lost subdirectory's list of children exists=%d", left)
		env.direct.Del(ctx, genDirectoryListKey(string(sub)), string(sub.Child("f")))
	})

	t.Run("M1_store_page_of_dead_names", func(t *testing.T) {
		env := newProbeEnv(t, "M1S", "3 dead names sort first, 2 live follow; the first store.ListDirectoryEntries with limit 3")
		env.seedDeadThenLive(3, 2)
		names := env.listNames(3)
		verdict(t, "M1 store page", len(names) < 2, "the first listing with limit 3 returned %v (2 live names exist)", names)
	})

	t.Run("M1_filer_listing_has_more", func(t *testing.T) {
		env := newProbeEnv(t, "M1F", "1 dead name sorts first, 5 live follow; the first Filer.ListDirectoryEntries with limit 2")
		env.seedDeadThenLive(1, 5)
		f := groupingFiler(env.direct)
		entries, hasMore, err := f.ListDirectoryEntries(ctx, env.dir, "", false, 2, "", "", "")
		verdict(t, "M1 filer hasMore", !hasMore, "the first listing with limit 2 returned %d entries, hasMore=%v, err=%v (5 live names exist)", len(entries), hasMore, err)
	})

	t.Run("M1_is_directory_empty", func(t *testing.T) {
		env := newProbeEnv(t, "M1E", "1 dead name sorts first, 1 live follows; the first Filer.IsDirectoryEmpty")
		env.seedDeadThenLive(1, 1)
		f := groupingFiler(env.direct)
		empty, err := f.IsDirectoryEmpty(ctx, env.dir)
		verdict(t, "M1 IsDirectoryEmpty", empty, "the first call returned isEmpty=%v err=%v (1 live name exists)", empty, err)
	})

	t.Run("M2a_insert_between_the_delete_and_the_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "M2A", "a listing finds f logically expired; a complete InsertEntry of f lands after the value's deletion and before the listing's ZREM")
		env.seedExpiredWithValue("f")
		ran := false
		lister := env.store(env.client("lister", env.addr, nil, env.insertRule("the insert before the ZREM", "zrem", "before", nil, "f", &ran)))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		if !ran {
			t.Logf("VERDICT M2a: NOT REACHED - the listing issued no ZREM")
			return
		}
		names := env.listNames(100)
		verdict(t, "M2a", len(names) == 0, "value exists=%d; a later listing shows %v", env.valueExists("f"), names)
	})

	t.Run("M2b_insert_between_the_GET_and_the_delete", func(t *testing.T) {
		env := newProbeEnv(t, "M2B", "a listing reads f, logically expired; a complete InsertEntry of f lands right after that GET, before the listing deletes the value")
		env.seedExpiredWithValue("f")
		ran := false
		lister := env.store(env.client("lister", env.addr, nil, env.insertRule("the insert after the GET", "get", "after", func(cmd redis.Cmder) bool { return cmd.Err() == nil }, "f", &ran)))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		names := env.listNames(100)
		verdict(t, "M2b", env.valueExists("f") == 0, "InsertEntry returned success; afterwards value exists=%d; a later listing shows %v", env.valueExists("f"), names)
	})
}

var _ = util.FullPath("")
