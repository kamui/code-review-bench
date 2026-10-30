import { initTRPC } from '../packages/server/src';
import { z } from 'zod';
const t = initTRPC.create();
const optional = t.procedure.input(z.object({ a: z.string().optional() }));
const router = t.router({
  plain: optional.query(({ input }) => input),
  withMiddleware: optional.use((opts) => opts.next()).query(({ input }) => input),
  concat: optional.unstable_concat(optional).query(({ input }) => input),
});
router.createCaller({}).plain({});
router.createCaller({}).withMiddleware({});
router.createCaller({}).concat({});
router.createCaller({}).withMiddleware({ a: undefined });
class Money {
  private cents = 100;
  format() { return String(this.cents); }
}
function takesMoney(m: Money) { return m.format(); }
t.procedure.input(z.instanceof(Money)).use((opts) => opts.next())
  .query(({ input }) => takesMoney(input));
t.procedure.input(z.instanceof(Money)).query(({ input }) => takesMoney(input));
const fn = z.function().args(z.number()).returns(z.number());
t.procedure.input(fn).use((opts) => opts.next()).query(({ input }) => input(2));
t.procedure.input(fn).query(({ input }) => input(2));
const fnRouter = t.router({ fn: t.procedure.input(fn).use((opts) => opts.next()).query(() => 1) });
// @ts-expect-error A number is not a function.
fnRouter.createCaller({}).fn(42);
