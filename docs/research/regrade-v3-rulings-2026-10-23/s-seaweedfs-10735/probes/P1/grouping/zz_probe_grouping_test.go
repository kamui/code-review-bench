package redis2

// Grouping probe for seaweedfs/seaweedfs#10735, round of 2026-10-23.
// Taken from the probe of the round of 2026-10-17 (docs/research/regrade-v10-rulings-2026-10-17/
// s-seaweedfs-10735/probes/grouping/zz_probe_grouping_test.go). Changed in this round: the cases
// of that round's groups are renamed after the problems they became (P1 -> PE-s4, P2 -> PE-s5,
// C1n -> GT-s5 dead list, C2n -> PE-s3 update and PE-s2 update), their code unchanged; and two
// cases are added for this round's P1 (a directory past its creation time plus time-to-live while
// Redis still holds its record, reached by a listing of its parent). The text below is that
// round's header, unchanged.
//
// Round of 2026-10-17:
// Taken from the probe of the round of 2026-10-13 (docs/research/regrade-rulings-2026-10-13/
// s-seaweedfs-10735/probes/grouping/zz_probe_grouping_test.go), with the cases of that round's
// candidates renamed after the pre-existing problems they became (M1 -> PE-s1, M2a -> PE-s2,
// M2b -> PE-s3) and these cases added: P2 (DeleteEntry's DEL then ZREM against an insert, and
// against an update), C1n (a lost directory whose children are gone too), C2n (an update instead
// of a create in the two windows of the expiry branch), and, on a Redis cluster with replicas
// (PROBE_CLUSTER_ADDRS), P1 (the scaffold's readOnly key) and GT-s1 (a lagging replica).
// One short failing case for each. It is run against the head and against the head with one or
// more small patches (variants.py), to see which patch cures which case.
// It uses the saved helpers. The order of commands is forced with a go-redis hook, as in the
// other probes. Each case ends in one line "VERDICT <case>: FAILS|OK|NOT REACHED - <what was seen>".

import (
	"context"
	"fmt"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"
	"github.com/spf13/viper"

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

func (env *probeEnv) isRecordOf(name string) func(redis.Cmder) bool {
	return func(cmd redis.Cmder) bool {
		args := cmd.Args()
		return len(args) > 1 && args[1] == string(env.dir.Child(name))
	}
}

func (env *probeEnv) updateRule(label, cmd, when string, match func(redis.Cmder) bool, name string, ran *bool) *probeRule {
	return &probeRule{label: label, cmd: cmd, when: when, match: match, act: func(context.Context, redis.Cmder) error {
		if *ran {
			return nil
		}
		*ran = true
		updater := env.store(env.client("updater", env.addr, nil))
		err := updater.UpdateEntry(context.Background(), probeEntry(env.dir.Child(name)))
		env.trace.add("%-9s UpdateEntry(%s) returned err=%v", "updater:", name, err)
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

	t.Run("PE-s1_store_page_of_dead_names", func(t *testing.T) {
		env := newProbeEnv(t, "PE1S", "3 dead names sort first, 2 live follow; the first store.ListDirectoryEntries with limit 3")
		env.seedDeadThenLive(3, 2)
		names := env.listNames(3)
		verdict(t, "PE-s1 store page", len(names) < 2, "the first listing with limit 3 returned %v (2 live names exist)", names)
	})

	t.Run("PE-s1_filer_listing_has_more", func(t *testing.T) {
		env := newProbeEnv(t, "PE1F", "1 dead name sorts first, 5 live follow; the first Filer.ListDirectoryEntries with limit 2")
		env.seedDeadThenLive(1, 5)
		f := groupingFiler(env.direct)
		entries, hasMore, err := f.ListDirectoryEntries(ctx, env.dir, "", false, 2, "", "", "")
		verdict(t, "PE-s1 filer hasMore", !hasMore, "the first listing with limit 2 returned %d entries, hasMore=%v, err=%v (5 live names exist)", len(entries), hasMore, err)
	})

	t.Run("PE-s1_is_directory_empty", func(t *testing.T) {
		env := newProbeEnv(t, "PE1E", "1 dead name sorts first, 1 live follows; the first Filer.IsDirectoryEmpty")
		env.seedDeadThenLive(1, 1)
		f := groupingFiler(env.direct)
		empty, err := f.IsDirectoryEmpty(ctx, env.dir)
		verdict(t, "PE-s1 IsDirectoryEmpty", empty, "the first call returned isEmpty=%v err=%v (1 live name exists)", empty, err)
	})

	t.Run("PE-s2_insert_between_the_delete_and_the_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "PE2", "a listing finds f logically expired; a complete InsertEntry of f lands after the value's deletion and before the listing's ZREM")
		env.seedExpiredWithValue("f")
		ran := false
		lister := env.store(env.client("lister", env.addr, nil, env.insertRule("the insert before the ZREM", "zrem", "before", nil, "f", &ran)))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		if !ran {
			t.Logf("VERDICT PE-s2: NOT REACHED - the listing issued no ZREM")
			return
		}
		names := env.listNames(100)
		verdict(t, "PE-s2", len(names) == 0, "value exists=%d; a later listing shows %v", env.valueExists("f"), names)
	})

	t.Run("PE-s3_insert_between_the_GET_and_the_delete", func(t *testing.T) {
		env := newProbeEnv(t, "PE3", "a listing reads f, logically expired; a complete InsertEntry of f lands right after that GET, before the listing deletes the value")
		env.seedExpiredWithValue("f")
		ran := false
		lister := env.store(env.client("lister", env.addr, nil, env.insertRule("the insert after the GET", "get", "after", func(cmd redis.Cmder) bool { return cmd.Err() == nil }, "f", &ran)))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		names := env.listNames(100)
		verdict(t, "PE-s3", env.valueExists("f") == 0, "InsertEntry returned success; afterwards value exists=%d; a later listing shows %v", env.valueExists("f"), names)
	})

	t.Run("P2_insert_between_DeleteEntry_DEL_and_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "P2", "DeleteEntry(f) runs; a complete InsertEntry(f) lands after its DEL of f's record and before its ZREM of the name")
		store := env.quietStore()
		store.InsertEntry(ctx, probeDirEntry(env.dir))
		store.InsertEntry(ctx, probeEntry(env.dir.Child("f")))
		ran := false
		deleter := env.store(env.client("deleter", env.addr, nil, env.insertRule("the insert after the DEL", "del", "after", env.isRecordOf("f"), "f", &ran)))
		err := deleter.DeleteEntry(ctx, env.dir.Child("f"))
		env.trace.dump(t)
		if !ran {
			t.Logf("VERDICT PE-s5: NOT REACHED - DeleteEntry issued no DEL of f's record")
			return
		}
		names := env.listNames(100)
		verdict(t, "PE-s5", env.valueExists("f") == 1 && len(names) == 0, "DeleteEntry err=%v; the insert returned success; value exists=%d; a later listing shows %v", err, env.valueExists("f"), names)
	})

	t.Run("P2_update_between_DeleteEntry_DEL_and_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "P2U", "DeleteEntry(f) runs; a complete UpdateEntry(f) lands after its DEL of f's record and before its ZREM of the name")
		store := env.quietStore()
		store.InsertEntry(ctx, probeDirEntry(env.dir))
		store.InsertEntry(ctx, probeEntry(env.dir.Child("f")))
		ran := false
		deleter := env.store(env.client("deleter", env.addr, nil, env.updateRule("the update after the DEL", "del", "after", env.isRecordOf("f"), "f", &ran)))
		err := deleter.DeleteEntry(ctx, env.dir.Child("f"))
		env.trace.dump(t)
		if !ran {
			t.Logf("VERDICT PE-s5 update: NOT REACHED - DeleteEntry issued no DEL of f's record")
			return
		}
		names := env.listNames(100)
		verdict(t, "PE-s5 update", env.valueExists("f") == 1 && len(names) == 0, "DeleteEntry err=%v; the update returned success; value exists=%d; a later listing shows %v", err, env.valueExists("f"), names)
	})

	t.Run("P2_neighbour_update_read_before_the_delete_written_after_it", func(t *testing.T) {
		env := newProbeEnv(t, "P2N", "a writer has read f; DeleteEntry(f) runs to the end; the writer's UpdateEntry(f) then writes f (the order the unforced HTTP race showed at the head)")
		store := env.quietStore()
		store.InsertEntry(ctx, probeDirEntry(env.dir))
		store.InsertEntry(ctx, probeEntry(env.dir.Child("f")))
		pending, _ := store.FindEntry(ctx, env.dir.Child("f"))
		deleteErr := env.store(env.client("deleter", env.addr, nil)).DeleteEntry(ctx, env.dir.Child("f"))
		updateErr := env.store(env.client("updater", env.addr, nil)).UpdateEntry(ctx, pending)
		env.trace.dump(t)
		names := env.listNames(100)
		verdict(t, "PE-s5 neighbour", env.valueExists("f") == 1 && len(names) == 0, "DeleteEntry err=%v; UpdateEntry err=%v; value exists=%d; a later listing shows %v", deleteErr, updateErr, env.valueExists("f"), names)
	})

	t.Run("C1n_lost_directory_whose_children_are_gone_too", func(t *testing.T) {
		env := newProbeEnv(t, "C1N", "subdirectory sub with children a and b; the records of sub, a and b are gone, sub's list of children survives; the parent is listed, then the filer's recursive delete of the parent")
		store := env.quietStore()
		sub := env.dir.Child("sub")
		store.InsertEntry(ctx, probeDirEntry(env.dir))
		store.InsertEntry(ctx, probeDirEntry(sub))
		store.InsertEntry(ctx, probeEntry(sub.Child("a")))
		store.InsertEntry(ctx, probeEntry(sub.Child("b")))
		env.direct.Del(ctx, string(sub), string(sub.Child("a")), string(sub.Child("b")))
		env.listNames(100)
		parentNames, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(env.dir)), 0, -1).Result()
		subNames, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(sub)), 0, -1).Result()
		env.recursiveDelete()
		left, _ := env.direct.Exists(ctx, genDirectoryListKey(string(sub))).Result()
		leftNames, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(sub)), 0, -1).Result()
		verdict(t, "GT-s5 dead list", left == 1, "after the listing the parent's list holds %v and sub's list of children holds %v; after the recursive delete of the parent reported success, sub's list of children exists=%d holding %v", parentNames, subNames, left, leftNames)
		env.direct.Del(ctx, genDirectoryListKey(string(sub)))
	})

	t.Run("C2n_update_between_the_GET_and_the_delete", func(t *testing.T) {
		env := newProbeEnv(t, "C2NA", "a listing reads f, logically expired; a complete UpdateEntry of f (no time-to-live) lands right after that GET, before the listing deletes the value")
		env.seedExpiredWithValue("f")
		ran := false
		lister := env.store(env.client("lister", env.addr, nil, env.updateRule("the update after the GET", "get", "after", func(cmd redis.Cmder) bool { return cmd.Err() == nil }, "f", &ran)))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		names := env.listNames(100)
		verdict(t, "PE-s3 update", env.valueExists("f") == 0, "UpdateEntry returned success; afterwards value exists=%d; a later listing shows %v", env.valueExists("f"), names)
	})

	t.Run("C2n_update_between_the_delete_and_the_ZREM", func(t *testing.T) {
		env := newProbeEnv(t, "C2NB", "a listing finds f logically expired; a complete UpdateEntry of f (no time-to-live) lands after the value's deletion and before the listing's ZREM")
		env.seedExpiredWithValue("f")
		ran := false
		lister := env.store(env.client("lister", env.addr, nil, env.updateRule("the update before the ZREM", "zrem", "before", nil, "f", &ran)))
		probeListVia(lister, ctx, env.dir)
		env.trace.dump(t)
		if !ran {
			t.Logf("VERDICT PE-s2 update: NOT REACHED - the listing issued no ZREM")
			return
		}
		names := env.listNames(100)
		verdict(t, "PE-s2 update", env.valueExists("f") == 1 && len(names) == 0, "UpdateEntry returned success; value exists=%d; a later listing shows %v", env.valueExists("f"), names)
	})
	t.Run("P1_logically_expired_directory_then_recursive_delete", func(t *testing.T) {
		env := newProbeEnv(t, "P1X", "subdirectory sub created 10 s ago with TtlSec 5 (Redis still holds its record), child x with no time-to-live; the parent is listed, then the filer's recursive delete of the parent")
		store := env.quietStore()
		sub := env.dir.Child("sub")
		store.InsertEntry(ctx, probeDirEntry(env.dir))
		created := time.Now().Add(-10 * time.Second)
		store.InsertEntry(ctx, &filer.Entry{FullPath: sub, Attr: filer.Attr{Crtime: created, Mtime: created, Mode: os.ModeDir | 0755, TtlSec: 5}})
		store.InsertEntry(ctx, probeEntry(sub.Child("x")))
		listed := env.listNames(100)
		parentNames, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(env.dir)), 0, -1).Result()
		subRecord, _ := env.direct.Exists(ctx, string(sub)).Result()
		subNames, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(sub)), 0, -1).Result()
		env.recursiveDelete()
		left, _ := env.direct.Exists(ctx, genDirectoryListKey(string(sub))).Result()
		leftNames, _ := env.direct.ZRange(ctx, genDirectoryListKey(string(sub)), 0, -1).Result()
		verdict(t, "P1", left == 1, "the listing returned %v; after it the parent's list holds %v, sub's record exists=%d and sub's list of children holds %v; after the recursive delete of the parent reported success, sub's list of children exists=%d holding %v; x's record exists=%d", listed, parentNames, subRecord, subNames, left, leftNames, env.valueExists("sub/x"))
		env.direct.Del(ctx, genDirectoryListKey(string(sub)), string(sub.Child("x")))
	})

	t.Run("P1_logically_expired_directory_recreated_lists_old_file", func(t *testing.T) {
		env := newProbeEnv(t, "P1R", "as the case above; then sub is created again as a directory and listed")
		store := env.quietStore()
		sub := env.dir.Child("sub")
		store.InsertEntry(ctx, probeDirEntry(env.dir))
		created := time.Now().Add(-10 * time.Second)
		store.InsertEntry(ctx, &filer.Entry{FullPath: sub, Attr: filer.Attr{Crtime: created, Mtime: created, Mode: os.ModeDir | 0755, TtlSec: 5}})
		store.InsertEntry(ctx, probeEntry(sub.Child("x")))
		env.listNames(100)
		env.recursiveDelete()
		store.InsertEntry(ctx, probeDirEntry(env.dir))
		store.InsertEntry(ctx, probeDirEntry(sub))
		names, err := probeListVia(store, ctx, sub)
		verdict(t, "P1 re-create", len(names) > 0, "after the listing, the recursive delete of the parent and the re-create of the parent and sub, a listing of the new sub returned %v err=%v", names, err)
		env.direct.Del(ctx, genDirectoryListKey(string(sub)), string(sub.Child("x")), string(sub))
	})
}

// clusterStore initializes RedisCluster2Store the way weed/filer/configuration.go does, from a
// configuration whose [redis_cluster2] section has the cluster's addresses and `key = true`.
func clusterStore(t *testing.T, addrs []string, key string) *RedisCluster2Store {
	t.Helper()
	v := viper.New()
	v.Set("redis_cluster2.enabled", true)
	v.Set("redis_cluster2.addresses", addrs)
	if key != "" {
		v.Set("redis_cluster2."+key, true)
	}
	store := &RedisCluster2Store{}
	if err := store.Initialize(&util.ViperProxy{Viper: v}, "redis_cluster2."); err != nil {
		t.Fatalf("Initialize: %v", err)
	}
	t.Cleanup(store.Shutdown)
	return store
}

// gtS1Case: a file is created while the replica of its record's slot lags (held back with
// CLIENT PAUSE ... WRITE); a listing of its directory then runs; the replica catches up.
func gtS1Case(t *testing.T, addrs []string, key, verdictName string) {
	ctx := context.Background()
	store := clusterStore(t, addrs, key)
	plain := redis.NewClusterClient(&redis.ClusterOptions{Addrs: addrs})
	finder := redis.NewClusterClient(&redis.ClusterOptions{Addrs: addrs, ReadOnly: true})
	defer plain.Close()
	defer finder.Close()
	var dir util.FullPath
	var path util.FullPath
	for i := 0; ; i++ {
		dir = util.FullPath(fmt.Sprintf("/probe_GS1_%d_%d", time.Now().UnixNano(), i))
		path = dir.Child("f")
		valueMaster, _ := plain.MasterForKey(ctx, string(path))
		indexMaster, _ := plain.MasterForKey(ctx, genDirectoryListKey(string(dir)))
		if valueMaster.Options().Addr != indexMaster.Options().Addr {
			break
		}
	}
	store.InsertEntry(ctx, probeDirEntry(dir))
	replica, err := finder.SlaveForKey(ctx, string(path))
	if err != nil {
		t.Fatalf("SlaveForKey: %v", err)
	}
	opts := store.Client.(*redis.ClusterClient).Options()
	if err := replica.Do(ctx, "CLIENT", "PAUSE", "3000", "WRITE").Err(); err != nil {
		t.Fatalf("pause: %v", err)
	}
	insertErr := store.InsertEntry(ctx, probeEntry(path))
	time.Sleep(200 * time.Millisecond)
	names := []string{}
	_, listErr := store.ListDirectoryEntries(ctx, dir, "", true, 100, func(entry *filer.Entry) (bool, error) {
		names = append(names, entry.Name())
		return true, nil
	})
	replica.Do(ctx, "CLIENT", "UNPAUSE")
	time.Sleep(500 * time.Millisecond)
	valueExists, _ := plain.Exists(ctx, string(path)).Result()
	members, _ := plain.ZRange(ctx, genDirectoryListKey(string(dir)), 0, -1).Result()
	verdict(t, verdictName, valueExists == 1 && len(members) == 0, "config key %q (ClusterOptions.ReadOnly=%v); replica %s of the record's slot held back; InsertEntry err=%v; the listing returned %v err=%v; after the replica caught up: value exists=%d, the directory's list holds %v", key, opts.ReadOnly, replica.Options().Addr, insertErr, names, listErr, valueExists, members)
	plain.Del(ctx, string(path))
	plain.Del(ctx, string(dir))
	plain.Del(ctx, genDirectoryListKey(string(dir)))
	parent, name := dir.DirAndName()
	plain.ZRem(ctx, genDirectoryListKey(parent), name)
}

func TestProbeGroupingCluster(t *testing.T) {
	raw := os.Getenv("PROBE_CLUSTER_ADDRS")
	if raw == "" {
		t.Skip("set PROBE_CLUSTER_ADDRS to a scratch Redis cluster with replicas")
	}
	addrs := strings.Split(raw, ",")

	t.Run("P1_the_scaffold_key_readOnly", func(t *testing.T) {
		store := clusterStore(t, addrs, "readOnly")
		opts := store.Client.(*redis.ClusterClient).Options()
		verdict(t, "PE-s4", !opts.ReadOnly, "with readOnly = true in [redis_cluster2], ClusterOptions.ReadOnly=%v RouteByLatency=%v", opts.ReadOnly, opts.RouteByLatency)
	})

	t.Run("GT-s1_lagging_replica_via_useReadOnly", func(t *testing.T) {
		gtS1Case(t, addrs, "useReadOnly", "GT-s1 via useReadOnly")
	})

	t.Run("GT-s1_lagging_replica_via_readOnly", func(t *testing.T) {
		gtS1Case(t, addrs, "readOnly", "GT-s1 via readOnly")
	})
}

var _ = util.FullPath("")
