// A middleware that always throws (for example a procedure switched off), then a resolver that reads ctx.
import { initTRPC, TRPCError } from '@trpc/server';

const t = initTRPC.context<{ user: string }>().create();
export const appRouter = t.router({
  inline: t.procedure
    .use(async () => {
      throw new TRPCError({ code: 'FORBIDDEN' });
    })
    .query(({ ctx }) => {
      type Show_ctx_inline = typeof ctx;
      return ctx.user;
    }),
  viaMiddleware: t.procedure
    .use(
      t.middleware(async () => {
        throw new TRPCError({ code: 'FORBIDDEN' });
      }),
    )
    .query(({ ctx }) => {
      type Show_ctx_via_t_middleware = typeof ctx;
      return ctx.user;
    }),
});
