// The documented way to write a middleware that is not tied to one instance: experimental_standaloneMiddleware
// with its context requirement declared, reading ctx, then used on an instance with that context.
import { experimental_standaloneMiddleware, initTRPC } from '@trpc/server';

const logSession = experimental_standaloneMiddleware<{ ctx: { session: string } }>().create(async (opts) => {
  const session = opts.ctx.session;
  return opts.next({ ctx: { session } });
});
const t = initTRPC.context<{ session: string }>().create();
export const appRouter = t.router({
  p: t.procedure.use(logSession).query(({ ctx }) => ctx.session),
});
