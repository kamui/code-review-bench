// The same with a hand-written validator function instead of Zod.
import { initTRPC } from '@trpc/server';

const t = initTRPC.create();
function parse(value: unknown): { a?: string } {
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    const a = (value as { a?: unknown }).a;
    if (a === undefined) return {};
    if (typeof a === 'string') return { a };
  }
  throw new Error('expected an object with an optional string a');
}

export const router = t.router({
  foo: t.procedure
    .input(parse)
    .use((opts) => opts.next())
    .query(({ input }) => input.a ?? 'absent'),
});

export async function call() {
  const api = router.createCaller({});
  await api.foo({});
}
