// The optional field with a middleware, called with the field written out as
// undefined.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();

export const router = t.router({
  foo: t.procedure
    .input(z.object({ a: z.string().optional() }))
    .use((opts) => opts.next())
    .query(({ input }) => input.a ?? 'absent'),
});

export async function call() {
  const api = router.createCaller({});
  await api.foo({ a: undefined });
}
