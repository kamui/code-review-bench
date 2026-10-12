// A "one or many" input, `string | string[]`, followed by a middleware.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const appRouter = t.router({
  oneOrManyMw: t.procedure
    .input(z.union([z.string(), z.array(z.string())]))
    .use((opts) => opts.next())
    .query(({ input }) => {
      const many: string[] = Array.isArray(input) ? input : [input];
      const viaTypeof: string[] = typeof input === 'string' ? [input] : input;
      return many.length + viaTypeof.length;
    }),
});
export async function caller() {
  const api = appRouter.createCaller({});
  await api.oneOrManyMw('a');
  await api.oneOrManyMw(['a', 'b']);
}
