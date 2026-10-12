package redis2

// The description as it read at the last push says of the cleanup: "a failed repair is retried on
// the next listing and must not fail the listing itself." This probe runs the cases that sentence
// can be read against. It uses the helpers of the saved S1 probe and asserts nothing.

import (
	"context"
	"testing"

	"github.com/redis/go-redis/v9"
)

func TestLastPushS1(t *testing.T) {
	ctx := context.Background()

	t.Run("P1_orphan_removal_fails_before_it_is_sent", func(t *testing.T) {
		env := newProbeEnv(t, "LP1", "an orphaned name, no re-create; redis becomes unreachable before the listing sends its next command")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		outage := &probeRule{label: "redis becoming unreachable", cmd: "get", when: "after", match: probeGotNil, act: func(context.Context, redis.Cmder) error {
			proxy.down()
			env.trace.add("%-9s redis is now unreachable for the lister", "fault:")
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, nil, outage))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		proxy.up()
		env.state("after the listing, redis reachable again", "f")
		env.laterListing("(the next listing, redis reachable again)")
		env.state("after the next listing", "f")
	})

	t.Run("P2_recreate_then_put_back_fails", func(t *testing.T) {
		env := newProbeEnv(t, "LP2", "re-create in the window, then the listing's own ZADD NX fails (error injected inside the client)")
		env.seedOrphan("f")
		failRestore := &probeRule{label: "the injected failure of the restoring ZADD NX", cmd: "zadd", when: "before", act: func(context.Context, redis.Cmder) error { return errProbeInjected }}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), failRestore))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("(the next listing)")
		env.state("after the next listing", "f")
	})

	t.Run("P3_recreate_in_window_no_fault", func(t *testing.T) {
		env := newProbeEnv(t, "LP3", "re-create in the window, no fault at all")
		env.seedOrphan("f")
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f")))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("(the next listing)")
	})
}
