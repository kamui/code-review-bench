// A required object input, then unstable_concat() with a procedure whose input
// is the optional form of the same object.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const requiredObject = z.object({ value: z.string() });

export const router = t.router({
  foo: t.procedure
    .input(requiredObject)
    .unstable_concat(t.procedure.input(requiredObject.optional()))
    .query(({ input }) => input.value.toUpperCase()),
});
