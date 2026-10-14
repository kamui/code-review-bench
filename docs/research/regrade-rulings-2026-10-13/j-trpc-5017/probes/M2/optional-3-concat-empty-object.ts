// The same joined procedure, called with an empty object.
// A call the parsers refuse at run time is marked as an expected error, so this file
// compiles only when the type refuses it too.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const first = t.procedure.input(z.object({ a: z.string() }).optional());
const second = t.procedure.input(z.object({ b: z.number() }).optional());
export const router = t.router({
  joined: first.unstable_concat(second).query(({ input }) => input),
});
const caller = router.createCaller({});
// @ts-expect-error both parsers refuse an empty object
export const empty = caller.joined({});
