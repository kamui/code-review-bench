package redis2

// S3: a delete that runs between the cleanup's separate commands.

import (
	"context"
	"testing"

	"github.com/redis/go-redis/v9"
)

func TestProbeS3(t *testing.T) {
	ctx := context.Background()

	recreateDirectoryAndList := func(env *probeEnv, name string) {
		store := env.quietStore()
		err := store.InsertEntry(ctx, probeDirEntry(env.dir))
		env.t.Logf("the directory is created again: InsertEntry(%s) err=%v", env.dir, err)
		env.laterListing("(of the re-created directory)")
		env.laterListing("(a second one)")
		env.state("after re-creating the directory and listing it twice", name)
	}

	t.Run("A_single_file_delete_between_recheck_and_restore", func(t *testing.T) {
		env := newProbeEnv(t, "S3A", "re-create in the window; then that file is deleted between the cleanup's EXISTS and its ZADD NX")
		env.seedOrphan("f")
		deleteFile := &probeRule{label: "the file delete", cmd: "zadd", when: "before", act: func(context.Context, redis.Cmder) error {
			deleter := env.store(env.client("deleter", env.addr, nil))
			err := deleter.DeleteEntry(ctx, env.dir.Child("f"))
			env.trace.add("%-9s DeleteEntry(f) returned err=%v", "deleter:", err)
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), deleteFile))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("(first later listing)")
		env.state("after the first later listing", "f")
	})

	t.Run("A0_same_actors_delete_after_the_listing", func(t *testing.T) {
		env := newProbeEnv(t, "S3A0", "comparison for A: the same re-create, and the same file delete run after the listing has finished")
		env.seedOrphan("f")
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f")))
		names, err := probeListVia(lister, ctx, env.dir)
		deleter := env.store(env.client("deleter", env.addr, nil))
		derr := deleter.DeleteEntry(ctx, env.dir.Child("f"))
		env.trace.add("%-9s DeleteEntry(f) returned err=%v", "deleter:", derr)
		env.report(names, err)
		env.state("after the listing and the delete", "f")
	})

	t.Run("A2_file_and_directory_deleted_between_recheck_and_restore", func(t *testing.T) {
		env := newProbeEnv(t, "S3A2", "re-create in the window; then the file and its (now empty) directory are both deleted between the cleanup's re-check and its ZADD NX")
		env.seedOrphan("f")
		deleteBoth := &probeRule{label: "the file and directory delete", cmd: "zadd", when: "before", act: func(context.Context, redis.Cmder) error {
			deleter := env.store(env.client("deleter", env.addr, nil))
			err := deleter.DeleteEntry(ctx, env.dir.Child("f"))
			env.trace.add("%-9s DeleteEntry(f) returned err=%v", "deleter:", err)
			err = deleter.DeleteEntry(ctx, env.dir)
			env.trace.add("%-9s DeleteEntry(directory) returned err=%v", "deleter:", err)
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), deleteBoth))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing and the deletes", "f")
		recreateDirectoryAndList(env, "f")
	})

	t.Run("B_recursive_directory_delete_between_zrem_and_restore", func(t *testing.T) {
		env := newProbeEnv(t, "S3B", "re-create in the window; then the whole directory is deleted recursively between the cleanup's ZREM and its EXISTS")
		env.seedOrphan("f")
		deleteDirectory := &probeRule{label: "the recursive directory delete", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
			env.recursiveDelete()
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), deleteDirectory))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing and the recursive delete", "f")
		entry, ferr := env.quietStore().FindEntry(ctx, env.dir.Child("f"))
		env.t.Logf("FindEntry(%s) after the directory was deleted: found=%v err=%v", env.dir.Child("f"), entry != nil && ferr == nil, ferr)
		recreateDirectoryAndList(env, "f")
	})

	t.Run("B0_same_actors_recursive_delete_just_before_the_zrem", func(t *testing.T) {
		env := newProbeEnv(t, "S3B0", "comparison for B: the same re-create and the same recursive delete, one step earlier (right after the re-create, before the lister's next command)")
		env.seedOrphan("f")
		deleteDirectory := &probeRule{label: "the recursive directory delete", cmd: "get", when: "after", match: probeGotNil, act: func(context.Context, redis.Cmder) error {
			env.recursiveDelete()
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), deleteDirectory))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing and the recursive delete", "f")
		recreateDirectoryAndList(env, "f")
	})

	t.Run("C_no_listing_at_all_insert_of_a_new_file_races_a_recursive_delete", func(t *testing.T) {
		env := newProbeEnv(t, "S3C", "no listing and no cleanup involved: a brand-new file is inserted while the directory is deleted recursively (delete lands between InsertEntry's SET and its ZADD NX)")
		if err := env.quietStore().InsertEntry(ctx, probeDirEntry(env.dir)); err != nil {
			t.Fatalf("seed directory: %v", err)
		}
		deleteDirectory := &probeRule{label: "the recursive directory delete", cmd: "set", when: "after", act: func(context.Context, redis.Cmder) error {
			env.recursiveDelete()
			return nil
		}}
		inserter := env.store(env.client("inserter", env.addr, nil, deleteDirectory))
		err := inserter.InsertEntry(ctx, probeEntry(env.dir.Child("g")))
		env.trace.add("%-9s InsertEntry(g) returned err=%v", "inserter:", err)
		env.t.Logf("command order as seen by redis:")
		env.trace.dump(t)
		env.notReached()
		env.state("after the insert and the recursive delete", "g")
		recreateDirectoryAndList(env, "g")
	})

	t.Run("D_second_listing_inside_the_window", func(t *testing.T) {
		env := newProbeEnv(t, "S3D", "re-create in the window; a second listing runs between the cleanup's ZREM and its EXISTS")
		env.seedOrphan("f")
		secondListing := &probeRule{label: "the second listing", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
			other := env.store(env.client("lister2", env.addr, nil))
			names, err := probeListVia(other, ctx, env.dir)
			env.trace.add("%-9s a second listing inside the window returned names=%v err=%v", "lister2:", names, err)
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), secondListing))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("")
	})
}
