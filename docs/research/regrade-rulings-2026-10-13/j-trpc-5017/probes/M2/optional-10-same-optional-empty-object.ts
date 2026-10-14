// The same procedure as optional-9, called with an empty object, which the parser refuses.
// A call the parsers refuse at run time is marked as an expected error, so this file
// compiles only when the type refuses it too.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const router = t.router({
  proc: t.procedure
    .input(z.object({ a: z.string() }).optional())
    .use((opts) => opts.next())
    .query(({ input }) => (input ? input.a.toUpperCase() : 'none')),
});
const caller = router.createCaller({});
// @ts-expect-error the parser refuses an empty object
export const empty = caller.proc({});
