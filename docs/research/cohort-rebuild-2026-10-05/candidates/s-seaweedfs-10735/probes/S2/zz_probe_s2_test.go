package redis2

// S2: redis applies the cleanup's ZREM, but the client is handed an error for it, so the helper
// returns before it re-checks the value or puts the name back.

import (
	"context"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"
)

func TestProbeS2(t *testing.T) {
	ctx := context.Background()

	t.Run("A_recreate_then_zrem_applied_but_reported_failed_injected", func(t *testing.T) {
		env := newProbeEnv(t, "S2A", "re-create in the window, then the ZREM runs on redis but the client is handed an error for it (error injected inside the client after the command ran)")
		env.seedOrphan("f")
		lostReply := &probeRule{label: "the injected error for an applied ZREM", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error { return errProbeInjected }}
		lister := env.store(env.client("lister", env.addr, nil, env.recreateAfterMiss("f"), lostReply))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.aftermath("f")
	})

	t.Run("B_one_real_lost_reply_default_retries", func(t *testing.T) {
		env := newProbeEnv(t, "S2B", "re-create in the window, then the ZREM's reply is really lost once: the connection is cut after redis ran it; redis stays reachable; go-redis default settings (3 retries)")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		cutReply := &probeRule{label: "the lost reply", cmd: "zrem", when: "before", act: func(context.Context, redis.Cmder) error {
			proxy.dropNextReply.Store(true)
			env.trace.add("%-9s the next reply to the lister will be lost and its connection cut", "fault:")
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, nil, env.recreateAfterMiss("f"), cutReply))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.t.Logf("note: the single ZREM line above is the result after go-redis retried it on a new connection; a reply of 0 means the first attempt had already removed the name")
		env.state("after the listing", "f")
		env.laterListing("")
	})

	t.Run("B2_one_real_lost_reply_retries_disabled", func(t *testing.T) {
		env := newProbeEnv(t, "S2B2", "the same single lost reply, with retries switched off (the store's max_retries = -1 setting)")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		cutReply := &probeRule{label: "the lost reply", cmd: "zrem", when: "before", act: func(context.Context, redis.Cmder) error {
			proxy.dropNextReply.Store(true)
			env.trace.add("%-9s the next reply to the lister will be lost and its connection cut", "fault:")
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, &redis.Options{MaxRetries: -1}, env.recreateAfterMiss("f"), cutReply))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("")
		env.laterListing("(a second one)")
	})

	t.Run("C_redis_stops_answering_from_the_zrem_on", func(t *testing.T) {
		env := newProbeEnv(t, "S2C", "re-create in the window, then redis keeps executing but stops answering the lister from the ZREM on (real read timeouts, read timeout 150ms, default 3 retries)")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		stall := &probeRule{label: "redis no longer answering", cmd: "zrem", when: "before", act: func(context.Context, redis.Cmder) error {
			proxy.withhold.Store(true)
			env.trace.add("%-9s replies to the lister are withheld from now on", "fault:")
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, &redis.Options{ReadTimeout: 150 * time.Millisecond}, env.recreateAfterMiss("f"), stall))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		proxy.withhold.Store(false)
		env.state("after the listing, redis answering again", "f")
		env.laterListing("(redis answering again)")
		env.laterListing("(a second one)")
	})

	t.Run("D_listing_deadline_passes_while_the_zrem_is_in_flight", func(t *testing.T) {
		env := newProbeEnv(t, "S2D", "re-create in the window, then the listing context's deadline (400ms) passes while the ZREM's reply is still on its way (reply delayed 800ms); store called directly")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		slow := &probeRule{label: "the delayed reply", cmd: "zrem", when: "before", act: func(context.Context, redis.Cmder) error {
			proxy.delayReplies.Store(int64(800 * time.Millisecond))
			env.trace.add("%-9s replies to the lister are delayed 800ms from now on", "fault:")
			return nil
		}}
		fast := &probeRule{label: "the end of the delay", cmd: "zrem", when: "after", act: func(deadlineCtx context.Context, _ redis.Cmder) error {
			proxy.delayReplies.Store(0)
			env.trace.add("%-9s the ZREM call returned; listing context error at this point: %v", "note:", deadlineCtx.Err())
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, nil, env.recreateAfterMiss("f"), slow, fast))
		deadlineCtx, cancel := context.WithTimeout(ctx, 400*time.Millisecond)
		defer cancel()
		names, err := probeListVia(lister, deadlineCtx, env.dir)
		env.report(names, err)
		env.state("after the listing", "f")
		env.laterListing("")
	})
}
