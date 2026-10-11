// The typed HTTP client a front end uses, against a router with an array input and a middleware.
import { createTRPCProxyClient, httpBatchLink } from '@trpc/client';
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const appRouter = t.router({
  arrMw: t.procedure
    .input(z.array(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => input),
  arrMwMutation: t.procedure
    .input(z.array(z.string()))
    .use((opts) => opts.next())
    .mutation(({ input }) => input.length),
});
type AppRouter = typeof appRouter;

export async function frontEnd() {
  const client = createTRPCProxyClient<AppRouter>({
    links: [httpBatchLink({ url: 'http://localhost:1' })],
  });
  const ids: string[] = ['a', 'b'];
  const echoed: string[] = await client.arrMw.query(ids);
  // ^? the call a front end writes @query(
  const count: number = await client.arrMwMutation.mutate(ids);
  const literal: string[] = await client.arrMw.query(['a', 'b']);
  return { echoed, count, literal };
}
