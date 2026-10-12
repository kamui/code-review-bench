package redis2

// S1: after the cleanup's ZREM has removed the name, the ZADD NX that puts it back fails and its
// result is thrown away.

import (
	"context"
	"testing"

	"github.com/redis/go-redis/v9"

	"github.com/seaweedfs/seaweedfs/weed/filer"
)

func TestProbeS1(t *testing.T) {
	ctx := context.Background()

	t.Run("A_control_orphan_no_race_no_fault", func(t *testing.T) {
		env := newProbeEnv(t, "S1A", "an orphaned name, nothing else going on")
		env.seedOrphan("f")
		lister := env.store(env.client("lister", env.addr, nil))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
	})

	t.Run("B_recreate_in_window_no_fault", func(t *testing.T) {
		env := newProbeEnv(t, "S1B", "the path is re-created between the listing's GET and its next command; no fault")
		env.seedOrphan("f")
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f")))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("")
	})

	t.Run("C_recreate_then_restore_fails_injected", func(t *testing.T) {
		env := newProbeEnv(t, "S1C", "re-create in the window, then the listing's own ZADD NX fails (error injected inside the client, command never sent)")
		env.seedOrphan("f")
		failRestore := &probeRule{label: "the injected failure of the restoring ZADD NX", cmd: "zadd", when: "before", act: func(context.Context, redis.Cmder) error { return errProbeInjected }}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), failRestore))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.aftermath("f")
	})

	t.Run("D_recreate_then_redis_unreachable_after_zrem", func(t *testing.T) {
		env := newProbeEnv(t, "S1D", "re-create in the window, then redis becomes unreachable for the lister right after its ZREM (real dropped connections, go-redis default retries)")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		outage := &probeRule{label: "redis becoming unreachable", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
			proxy.down()
			env.trace.add("%-9s redis is now unreachable for the lister", "fault:")
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, nil, env.recreateAfterMiss("f"), outage))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		proxy.up()
		env.state("after the listing, redis reachable again", "f")
		env.laterListing("(redis reachable again)")
		env.laterListing("(a second one)")
	})

	t.Run("D2_recreate_then_redis_unreachable_before_zrem", func(t *testing.T) {
		env := newProbeEnv(t, "S1D2", "same outage, but starting one command earlier: right after the re-create, before the lister sends anything else")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		outage := &probeRule{label: "redis becoming unreachable", cmd: "get", when: "after", match: probeGotNil, act: func(context.Context, redis.Cmder) error {
			proxy.down()
			env.trace.add("%-9s redis is now unreachable for the lister", "fault:")
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, nil, env.recreateAfterMiss("f"), outage))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		proxy.up()
		env.state("after the listing, redis reachable again", "f")
		env.laterListing("(redis reachable again)")
	})

	t.Run("E_recreate_then_listing_context_cancelled_after_zrem_store_called_directly", func(t *testing.T) {
		env := newProbeEnv(t, "S1E", "re-create in the window, then the listing's context is cancelled right after its ZREM; the redis2 store is called directly with that context")
		env.seedOrphan("f")
		listCtx, cancel := context.WithCancel(ctx)
		defer cancel()
		cancelAfterZrem := &probeRule{label: "the context cancellation", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
			cancel()
			env.trace.add("%-9s the caller's context is cancelled", "fault:")
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), cancelAfterZrem))
		names, err := probeListVia(lister, listCtx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("")
	})

	t.Run("F_same_cancellation_through_the_filer_store_wrapper", func(t *testing.T) {
		env := newProbeEnv(t, "S1F", "the same cancellation, but the listing goes through filer.FilerStoreWrapper, which is how the filer reaches every store")
		env.seedOrphan("f")
		listCtx, cancel := context.WithCancel(ctx)
		defer cancel()
		cancelAfterZrem := &probeRule{label: "the context cancellation", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
			cancel()
			env.trace.add("%-9s the caller's context is cancelled", "fault:")
			return nil
		}}
		client := env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), cancelAfterZrem)
		wrapper := filer.NewFilerStoreWrapper(&Redis2Store{UniversalRedis2Store{Client: client}})
		names, err := probeListVia(wrapper, listCtx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("")
	})

	t.Run("F2_same_cancellation_through_the_wrapper_entry_point_the_filer_uses", func(t *testing.T) {
		env := newProbeEnv(t, "S1F2", "the same cancellation through FilerStoreWrapper.ListDirectoryPrefixedEntries, the call Filer.ListDirectoryEntries makes (weed/filer/filer.go)")
		env.seedOrphan("f")
		listCtx, cancel := context.WithCancel(ctx)
		defer cancel()
		cancelAfterZrem := &probeRule{label: "the context cancellation", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
			cancel()
			env.trace.add("%-9s the caller's context is cancelled", "fault:")
			return nil
		}}
		client := env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), cancelAfterZrem)
		wrapper := filer.NewFilerStoreWrapper(&Redis2Store{UniversalRedis2Store{Client: client}})
		names := []string{}
		_, err := wrapper.ListDirectoryPrefixedEntries(listCtx, env.dir, "", true, 100, "", func(entry *filer.Entry) (bool, error) {
			_, name := entry.FullPath.DirAndName()
			names = append(names, name)
			return true, nil
		})
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("")
	})

	t.Run("G_no_recreate_redis_unreachable_after_zrem", func(t *testing.T) {
		env := newProbeEnv(t, "S1G", "the fault alone: a genuine orphan (no re-create), redis unreachable right after the ZREM")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		outage := &probeRule{label: "redis becoming unreachable", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
			proxy.down()
			env.trace.add("%-9s redis is now unreachable for the lister", "fault:")
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, nil, outage))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		proxy.up()
		env.state("after the listing, redis reachable again", "f")
	})

	t.Run("H_recreate_then_redis_rejects_the_restore_out_of_memory", func(t *testing.T) {
		env := newProbeEnv(t, "S1H", "re-create in the window, then redis itself refuses the ZADD NX: maxmemory reached under the default noeviction policy (real server reply)")
		env.seedOrphan("f")
		memoryFull := &probeRule{label: "redis reaching its memory limit", cmd: "zadd", when: "before", act: func(context.Context, redis.Cmder) error {
			err := env.direct.ConfigSet(ctx, "maxmemory", "1").Err()
			env.trace.add("%-9s CONFIG SET maxmemory 1 (redis now over its memory limit) err=%v", "fault:", err)
			return nil
		}}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), memoryFull))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.direct.ConfigSet(ctx, "maxmemory", "0")
		env.state("after the listing, memory limit lifted", "f")
		env.laterListing("(memory limit lifted)")
	})
}
