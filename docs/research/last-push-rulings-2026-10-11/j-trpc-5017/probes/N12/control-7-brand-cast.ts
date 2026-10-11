// The branded string with a middleware, with the input cast before use.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
function takesString(value: string) {
  return value.toUpperCase();
}

export const router = t.router({
  foo: t.procedure
    .input(z.string().brand<'id'>())
    .use((opts) => opts.next())
    .query(({ input }) => takesString(input as unknown as string)),
});
