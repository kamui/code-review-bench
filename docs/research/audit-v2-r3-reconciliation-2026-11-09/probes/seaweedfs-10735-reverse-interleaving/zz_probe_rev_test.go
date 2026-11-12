package redis2

// REV: can "a live value be left without its member if the ZREM lands after the EXISTS check"?
// Every command succeeds in every scenario. Each scenario places a concurrent actor at one
// point of the listing's cleanup (ZREM, then EXISTS, then ZADD NX) and prints the end state.
// Uses zz_probe_helpers_test.go from the S3 probe of the 2026-10-05 rebuild, unchanged.

import (
	"context"
	"testing"

	"github.com/redis/go-redis/v9"
)

func probeArgIs(i int, want string) func(redis.Cmder) bool {
	return func(cmd redis.Cmder) bool {
		args := cmd.Args()
		return len(args) > i && args[i] == want
	}
}

func TestProbeREV(t *testing.T) {
	ctx := context.Background()

	insertNow := func(env *probeEnv, name string) {
		inserter := env.store(env.client("inserter", env.addr, nil))
		err := inserter.InsertEntry(ctx, probeEntry(env.dir.Child(name)))
		env.trace.add("%-9s InsertEntry(%s) returned err=%v", "inserter:", name, err)
	}

	finish := func(env *probeEnv, names []string, err error) {
		env.report(names, err)
		env.state("at the end", "f")
		env.laterListing("(a fresh listing afterwards)")
	}

	t.Run("A1_insert_before_the_cleanup_zrem", func(t *testing.T) {
		env := newProbeEnv(t, "REVA1", "the PR's own case: a complete InsertEntry lands after the listing's GET returned nil and before the cleanup's ZREM")
		env.seedOrphan("f")
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f")))
		names, err := probeListVia(lister, ctx, env.dir)
		finish(env, names, err)
	})

	t.Run("A2_insert_between_the_cleanup_zrem_and_exists", func(t *testing.T) {
		env := newProbeEnv(t, "REVA2", "a complete InsertEntry lands after the cleanup's ZREM and before its EXISTS")
		env.seedOrphan("f")
		rule := &probeRule{label: "the insert", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
			insertNow(env, "f")
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, rule))
		names, err := probeListVia(lister, ctx, env.dir)
		finish(env, names, err)
	})

	t.Run("A3_insert_after_the_cleanup_exists_saw_no_value", func(t *testing.T) {
		env := newProbeEnv(t, "REVA3", "a complete InsertEntry lands after the cleanup's EXISTS returned 0 (the cleanup then returns without ZADD NX)")
		env.seedOrphan("f")
		rule := &probeRule{label: "the insert", cmd: "exists", when: "after", act: func(context.Context, redis.Cmder) error {
			insertNow(env, "f")
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, rule))
		names, err := probeListVia(lister, ctx, env.dir)
		finish(env, names, err)
	})

	// B: a second listing's cleanup of the same name. Both listings got nil from GET before the
	// re-create; the second one's ZREM is placed after the first one's EXISTS check.
	secondCleanup := func(t *testing.T, id, title, releaseAt string, interleaveZadd bool) {
		env := newProbeEnv(t, id, title)
		env.seedOrphan("f")
		got2 := make(chan struct{})
		go2 := make(chan struct{})
		zrem2 := make(chan struct{})
		go2b := make(chan struct{})
		done2 := make(chan struct{})
		rules2 := []*probeRule{{label: "lister2 pauses after its GET returned nil", cmd: "get", when: "after", match: probeGotNil, act: func(context.Context, redis.Cmder) error {
			close(got2)
			<-go2
			return nil
		}}}
		if interleaveZadd {
			rules2 = append(rules2, &probeRule{label: "lister2 pauses after its ZREM", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
				close(zrem2)
				<-go2b
				return nil
			}})
		}
		lister2 := env.store(env.client("lister2", env.addr, nil, rules2...))
		start2 := &probeRule{label: "lister2 starts and reaches its nil GET; then the re-create", cmd: "get", when: "after", match: probeGotNil, act: func(context.Context, redis.Cmder) error {
			go func() {
				names, err := probeListVia(lister2, ctx, env.dir)
				env.trace.add("%-9s the second listing returned names=%v err=%v", "lister2:", names, err)
				close(done2)
			}()
			<-got2
			insertNow(env, "f")
			return nil
		}}
		release := &probeRule{label: "lister2 released", cmd: releaseAt, when: "after", act: func(context.Context, redis.Cmder) error {
			close(go2)
			if interleaveZadd {
				<-zrem2
			} else {
				<-done2
			}
			return nil
		}}
		rules1 := []*probeRule{start2, release}
		if interleaveZadd {
			rules1 = append(rules1, &probeRule{label: "lister2 resumes after the first cleanup's ZADD NX", cmd: "zadd", when: "after", act: func(context.Context, redis.Cmder) error {
				close(go2b)
				<-done2
				return nil
			}})
		}
		lister := env.store(env.client("lister", env.addr, nil, rules1...))
		names, err := probeListVia(lister, ctx, env.dir)
		finish(env, names, err)
	}

	t.Run("B1_second_cleanup_runs_whole_between_first_exists_and_first_zadd", func(t *testing.T) {
		secondCleanup(t, "REVB1", "two listings got nil, then a re-create; the second listing's cleanup (ZREM, EXISTS, ZADD NX) runs after the first cleanup's EXISTS and before its ZADD NX", "exists", false)
	})

	t.Run("B2_second_cleanup_zrem_after_first_exists_first_zadd_before_second_exists", func(t *testing.T) {
		secondCleanup(t, "REVB2", "as B1, but the first cleanup's ZADD NX lands between the second cleanup's ZREM and its EXISTS", "exists", true)
	})

	t.Run("B3_second_cleanup_runs_whole_after_first_zadd", func(t *testing.T) {
		secondCleanup(t, "REVB3", "as B1, but the second cleanup runs after the first cleanup's ZADD NX", "zadd", false)
	})

	// C: a DeleteEntry of the same path, whose DEL lands before a re-create and whose ZREM is
	// placed after the cleanup's EXISTS check.
	deleteRace := func(t *testing.T, id, title, releaseAt string) {
		env := newProbeEnv(t, id, title)
		env.seedOrphan("f")
		path := string(env.dir.Child("f"))
		del := make(chan struct{})
		goDel := make(chan struct{})
		doneDel := make(chan struct{})
		deleter := env.store(env.client("deleter", env.addr, nil, &probeRule{label: "the deleter pauses after DEL of the value", cmd: "del", when: "after", match: probeArgIs(1, path), act: func(context.Context, redis.Cmder) error {
			close(del)
			<-goDel
			return nil
		}}))
		startDelete := &probeRule{label: "DeleteEntry(f) starts; after its DEL, the re-create", cmd: "get", when: "after", match: probeGotNil, act: func(context.Context, redis.Cmder) error {
			go func() {
				err := deleter.DeleteEntry(ctx, env.dir.Child("f"))
				env.trace.add("%-9s DeleteEntry(f) returned err=%v", "deleter:", err)
				close(doneDel)
			}()
			<-del
			insertNow(env, "f")
			return nil
		}}
		release := &probeRule{label: "the deleter's ZREM released", cmd: releaseAt, when: "after", act: func(context.Context, redis.Cmder) error {
			close(goDel)
			<-doneDel
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, startDelete, release))
		names, err := probeListVia(lister, ctx, env.dir)
		finish(env, names, err)
	}

	t.Run("C1_delete_zrem_after_cleanup_exists_and_zadd", func(t *testing.T) {
		deleteRace(t, "REVC1", "listing got nil; DeleteEntry(f) runs DEL; a re-create (SET, no-op ZADD NX); the cleanup's ZREM, EXISTS, ZADD NX; then the DeleteEntry's ZREM", "zadd")
	})

	t.Run("C2_delete_zrem_between_cleanup_exists_and_zadd", func(t *testing.T) {
		deleteRace(t, "REVC2", "as C1, but the DeleteEntry's ZREM lands after the cleanup's EXISTS and before its ZADD NX", "exists")
	})

	t.Run("C0_no_listing_delete_and_recreate", func(t *testing.T) {
		env := newProbeEnv(t, "REVC0", "no listing: f is live and indexed; DeleteEntry(f) runs DEL; a re-create (SET, no-op ZADD NX); then the DeleteEntry's ZREM")
		if err := env.quietStore().InsertEntry(ctx, probeDirEntry(env.dir)); err != nil {
			t.Fatalf("seed directory: %v", err)
		}
		if err := env.quietStore().InsertEntry(ctx, probeEntry(env.dir.Child("f"))); err != nil {
			t.Fatalf("seed f: %v", err)
		}
		path := string(env.dir.Child("f"))
		deleter := env.store(env.client("deleter", env.addr, nil, &probeRule{label: "the re-create after the deleter's DEL", cmd: "del", when: "after", match: probeArgIs(1, path), act: func(context.Context, redis.Cmder) error {
			insertNow(env, "f")
			return nil
		}}))
		err := deleter.DeleteEntry(ctx, env.dir.Child("f"))
		env.trace.add("%-9s DeleteEntry(f) returned err=%v", "deleter:", err)
		env.t.Logf("command order as seen by redis:")
		env.trace.dump(t)
		env.notReached()
		env.state("at the end", "f")
		env.laterListing("(a fresh listing afterwards)")
	})
}
