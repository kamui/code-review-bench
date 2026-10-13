// What the resolver's input is after narrowing to variant 'b', with and without a middleware.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const schema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('a'), x: z.string() }),
  z.object({ type: z.literal('b'), y: z.number() }),
]);
const plain = t.procedure.input(schema);
const afterUse = t.procedure.input(schema).use((opts) => opts.next());
type ResolverPlain = Parameters<Parameters<(typeof plain)['query']>[0]>[0]['input'];
type ResolverAfterUse = Parameters<Parameters<(typeof afterUse)['query']>[0]>[0]['input'];
export type Show_narrowed_to_b_plain = Extract<ResolverPlain, { type: 'b' }>;
export type Show_narrowed_to_b_with_middleware = Extract<ResolverAfterUse, { type: 'b' }>;
