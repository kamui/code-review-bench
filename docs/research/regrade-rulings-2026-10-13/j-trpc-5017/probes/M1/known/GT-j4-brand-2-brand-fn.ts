// The same, with a function that takes the branded type.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const userId = z.string().brand<'id'>();
function takesUserId(value: z.infer<typeof userId>) {
  return value;
}

export const router = t.router({
  foo: t.procedure
    .input(userId)
    .use((opts) => opts.next())
    .query(({ input }) => takesUserId(input)),
});
