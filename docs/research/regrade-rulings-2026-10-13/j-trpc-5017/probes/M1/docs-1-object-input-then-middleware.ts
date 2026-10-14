// The documented order with an object input: an input, then a middleware that reads it,
// then a second input (www/docs/server/validators.md, "Input Merging").
import { inferRouterInputs, initTRPC, TRPCError } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.context<{ userId?: string }>().create();

const organizationProcedure = t.procedure
  .input(z.object({ organizationId: z.string() }))
  .use((opts) => {
    if (!opts.ctx.userId) {
      throw new TRPCError({ code: 'UNAUTHORIZED' });
    }
    return opts.next({ ctx: { organizationId: opts.input.organizationId } });
  });

export const router = t.router({
  members: organizationProcedure
    .input(z.object({ limit: z.number() }))
    .query(({ input, ctx }) => `${ctx.organizationId} ${input.organizationId} ${input.limit}`),
});
export type Show_caller = inferRouterInputs<typeof router>['members'];
