// A required object input, then a standalone middleware whose declared input
// also allows null.
import { experimental_standaloneMiddleware, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const acceptsObjectOrNull = experimental_standaloneMiddleware<{
  input: { value: string } | null;
}>().create((opts) => opts.next());

export const router = t.router({
  foo: t.procedure
    .input(z.object({ value: z.string() }))
    .use(acceptsObjectOrNull)
    .query(({ input }) => input.value.toUpperCase()),
});
