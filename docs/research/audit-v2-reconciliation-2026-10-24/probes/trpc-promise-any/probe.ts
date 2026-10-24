import { initTRPC, TRPCError } from '@trpc/server';

const t = initTRPC.context<{ user: string }>().create();

// The review's own reproduction: a throw-only middleware.
export const throwOnly = t.procedure
  .use(async () => {
    throw new TRPCError({ code: 'UNAUTHORIZED' });
  })
  .query(({ ctx }) => {
    const shown: number = ctx; // shows ctx's type in the error message
    return ctx.user;
  });

// The first grader's open fact: a middleware typed to return Promise<any>.
export const promiseAny = t.procedure
  .use(async (opts): Promise<any> => opts.next())
  .query(({ ctx }) => {
    const shown: number = ctx;
    return ctx.user;
  });

// Control: an ordinary pass-through middleware.
export const passThrough = t.procedure
  .use((opts) => opts.next())
  .query(({ ctx }) => {
    const shown: number = ctx;
    return ctx.user;
  });
