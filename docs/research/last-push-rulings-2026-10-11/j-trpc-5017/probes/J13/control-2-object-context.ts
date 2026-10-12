// An ordinary middleware that adds an object to the context.
import { initTRPC } from '@trpc/server';

const t = initTRPC.context<{ userId: string }>().create();

export const router = t.router({
  foo: t.procedure
    .use((opts) => opts.next({ ctx: { tenant: 'a' } }))
    .use((opts) => opts.next())
    .query(({ ctx }) => `${ctx.userId} ${ctx.tenant}`),
});
