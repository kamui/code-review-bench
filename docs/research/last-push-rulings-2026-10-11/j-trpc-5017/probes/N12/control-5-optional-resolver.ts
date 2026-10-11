// The resolver side of the optional field: the middleware-wrapped input passed
// to a function that takes the validator's own output type.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const shape = z.object({ a: z.string().optional() });
function takesShape(value: z.infer<typeof shape>) {
  return value.a ?? 'absent';
}

export const router = t.router({
  foo: t.procedure
    .input(shape)
    .use((opts) => opts.next())
    .query(({ input }) => takesShape(input)),
});
