// The same joined procedure, called with the first procedure's field alone.
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
// @ts-expect-error the second parser requires `b` whenever an object is passed
export const onlyFirst = caller.joined({ a: 'x' });
