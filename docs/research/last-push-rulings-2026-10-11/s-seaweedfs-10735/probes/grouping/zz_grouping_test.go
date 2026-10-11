package redis2

// Three more scenarios for the grouping question, beside the saved S1 and S2 scenarios. They use
// the helpers of the saved S1 probe and assert nothing. Each prints the state after the listing
// and what two later listings on a healthy redis return.

import (
	"context"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"
)

func TestGrouping(t *testing.T) {
	ctx := context.Background()

	t.Run("X1_put_back_fails_on_every_attempt_injected", func(t *testing.T) {
		env := newProbeEnv(t, "GX1", "re-create in the window, then every ZADD NX the listing sends fails (error injected inside the client, as often as it is tried)")
		env.seedOrphan("f")
		rules := []*probeRule{env.recreateAfterMiss("f")}
		for i := 0; i < 64; i++ {
			rules = append(rules, &probeRule{label: "an injected failure of a restoring ZADD NX", cmd: "zadd", when: "before", act: func(context.Context, redis.Cmder) error { return errProbeInjected }})
		}
		lister := env.store(env.client("lister", env.addr, nil, rules...))
		names, err := probeListVia(lister, ctx, env.dir)
		env.t.Logf("command order as seen by redis:")
		env.trace.dump(env.t)
		env.t.Logf("THE LISTING RETURNED: names=%v err=%v", names, err)
		env.state("after the listing", "f")
		env.laterListing("(a fresh listing, healthy redis)")
		env.laterListing("(a second one)")
		env.state("after the later listings", "f")
	})

	t.Run("X2_redis_unreachable_for_5s_after_the_removal", func(t *testing.T) {
		env := newProbeEnv(t, "GX2", "re-create in the window, then redis is unreachable for the lister from just after its ZREM and comes back 5 seconds later (real dropped connections, go-redis default retries)")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		back := make(chan struct{})
		outage := &probeRule{label: "redis becoming unreachable for 5 seconds", cmd: "zrem", when: "after", act: func(context.Context, redis.Cmder) error {
			proxy.down()
			env.trace.add("%-9s redis is now unreachable for the lister, for 5 seconds", "fault:")
			time.AfterFunc(5*time.Second, func() {
				proxy.up()
				env.trace.add("%-9s redis is reachable again", "fault:")
				close(back)
			})
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, nil, env.recreateAfterMiss("f"), outage))
		start := time.Now()
		names, err := probeListVia(lister, ctx, env.dir)
		took := time.Since(start).Round(100 * time.Millisecond)
		if outage.fired {
			<-back
		}
		env.report(names, err)
		env.t.Logf("the listing took %v", took)
		env.state("after the listing, redis reachable again", "f")
		env.laterListing("(a fresh listing, healthy redis)")
		env.laterListing("(a second one)")
		env.state("after the later listings", "f")
	})

	t.Run("X3_removal_applied_reply_lost_and_redis_unreachable_from_then_on", func(t *testing.T) {
		env := newProbeEnv(t, "GX3", "re-create in the window, then redis runs the ZREM, its reply is lost, and redis is unreachable for the lister from that moment until the listing has returned (real, go-redis default retries)")
		env.seedOrphan("f")
		proxy := newProbeProxy(t, env.addr)
		lostAndGone := &probeRule{label: "the lost reply followed by redis becoming unreachable", cmd: "zrem", when: "before", act: func(context.Context, redis.Cmder) error {
			proxy.dropNextReply.Store(true)
			env.trace.add("%-9s the next reply to the lister will be lost, and redis unreachable from then on", "fault:")
			go func() {
				for proxy.dropNextReply.Load() {
					time.Sleep(50 * time.Microsecond)
				}
				proxy.down()
			}()
			return nil
		}}
		lister := env.store(env.client("lister", proxy.addr, nil, env.recreateAfterMiss("f"), lostAndGone))
		names, err := probeListVia(lister, ctx, env.dir)
		env.report(names, err)
		proxy.up()
		env.state("after the listing, redis reachable again", "f")
		env.laterListing("(a fresh listing, healthy redis)")
		env.laterListing("(a second one)")
		env.state("after the later listings", "f")
	})
}
