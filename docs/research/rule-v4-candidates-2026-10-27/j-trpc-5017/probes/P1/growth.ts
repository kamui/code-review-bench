import { initTRPC } from '../packages/server/src';
import { z } from 'zod';
const t = initTRPC.create();
const a = z.object({ type: z.literal('a'), a: z.string() });
const b = z.object({ type: z.literal('b'), b: z.string() });
const c = z.object({ type: z.literal('c'), c: z.string() });
export const two = t.procedure.input(z.discriminatedUnion('type', [a, b]))
  .use((opts) => opts.next())
  .use((opts) => opts.next())
  .use((opts) => opts.next())
  .use((opts) => opts.next())
  .query(({ input }) => input);
export const three = t.procedure.input(z.discriminatedUnion('type', [a, b, c]))
  .use((opts) => opts.next())
  .use((opts) => opts.next())
  .use((opts) => opts.next())
  .use((opts) => opts.next())
  .query(({ input }) => input);
