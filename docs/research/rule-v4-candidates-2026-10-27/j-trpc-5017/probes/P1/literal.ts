import { experimental_standaloneMiddleware, initTRPC } from '../packages/server/src';
import { z } from 'zod';
import type { Overwrite } from '../packages/server/src/core/internals/utils';

const t = initTRPC.create();
const schema = z.object({ a: z.literal('x') });
const mw = experimental_standaloneMiddleware<{ input: { a: string } }>()
  .create((opts) => opts.next());
export const procedure = t.procedure.input(schema).use(mw).query(({ input }) => {
  const inspectLiteral = input;
  const literal: 'x' = input.a;
  return literal;
});
const router = t.router({ procedure });
const caller = router.createCaller({});
caller.procedure({ a: 'x' });
// @ts-expect-error The validator accepts only x.
caller.procedure({ a: 'y' });
export type InspectHelper = Overwrite<{ a: 'x' }, { a: string }>;

const exact = experimental_standaloneMiddleware<{ input: { a: 'x' } }>()
  .create((opts) => opts.next());
t.procedure.input(schema).use(exact).query(({ input }) => {
  const literal: 'x' = input.a;
  return literal;
});
t.procedure.input(schema).query(({ input }) => {
  const literal: 'x' = input.a;
  return literal;
});
t.procedure.input(schema).use((opts) => opts.next()).query(({ input }) => {
  const literal: 'x' = input.a;
  return literal;
});
t.procedure.input(schema).use(mw).input(schema).query(({ input }) => {
  const literal: 'x' = input.a;
  return literal;
});
const publicExample = experimental_standaloneMiddleware<{ input: { projectId: string } }>()
  .create((opts) => opts.next());
t.procedure.input(z.object({ projectId: z.string() })).use(publicExample)
  .query(({ input }) => input.projectId.toUpperCase());

t.procedure.input(schema).use(mw).query(({ input }) => {
  if (input.a !== 'x') throw new Error('unexpected input');
  const literal: 'x' = input.a;
  return literal;
});
