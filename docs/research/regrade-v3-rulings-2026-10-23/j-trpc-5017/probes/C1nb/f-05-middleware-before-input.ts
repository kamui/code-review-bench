// The order the documentation shows for a reusable middleware: a base procedure that
// carries the middleware, with the input added after it.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const fn = z.function().args(z.number()).returns(z.number());
const t = initTRPC.create();
const base = t.procedure.use((opts) => opts.next());
export const router = t.router({
  middlewareFirst: base.input(fn).query(({ input }) => input(2)),
});
export async function call() {
  const api = router.createCaller({});
  await api.middlewareFirst((x) => x * 2);
  // @ts-expect-error a number is not a function
  await api.middlewareFirst(42);
}
