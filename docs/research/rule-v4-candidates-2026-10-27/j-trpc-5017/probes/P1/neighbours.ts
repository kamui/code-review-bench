import { experimental_standaloneMiddleware, initTRPC } from '../packages/server/src';
import { z } from 'zod';
const t = initTRPC.create();
const branded = z.string().brand<'id'>();
t.procedure.input(branded).use((opts) => opts.next()).query(({ input }) => {
  const value: string = input;
  return value;
});
function pair(a: string, b: number) { return a + b; }
t.procedure.input(z.tuple([z.string(), z.number()])).use((opts) => opts.next())
  .query(({ input }) => pair(...input));
t.procedure.input(z.array(z.string())).use((opts) => opts.next()).query(async ({ input }) => {
  const values: string[] = await Promise.all(input);
  const objectValues: string[] = Object.values(input);
  const entries: [string, string][] = Object.entries(input);
  return { values, objectValues, entries };
});
t.procedure.input(z.array(z.string())).unstable_concat(t.procedure.input(z.array(z.string())))
  .query(async ({ input }) => {
    const values: string[] = await Promise.all(input);
    return values;
  });
const union = z.discriminatedUnion('type', [
  z.object({ type: z.literal('a'), x: z.string() }),
  z.object({ type: z.literal('b'), y: z.number() }),
]);
const u = t.procedure.input(union).use((opts) => opts.next());
const ur = t.router({ u: u.query(({ input }) => {
  if ('x' in input) {
    const a: 'a' = input.type;
    return a;
  }
  return input.type;
}) });
// @ts-expect-error The declared union has no variant with all three fields.
ur.createCaller({}).u({ type: 'b', x: 'x', y: 1 });
const first = z.object({ a: z.string() }).optional();
const second = z.object({ b: z.number() }).optional();
const sm = experimental_standaloneMiddleware<{ input: z.infer<typeof second> }>()
  .create((opts) => opts.next());
const op = t.procedure.input(first);
const or = t.router({
  concat: op.unstable_concat(t.procedure.input(second)).query(({ input }) => input?.a),
  standalone: t.procedure.input(z.object({ a: z.string(), b: z.number() }).optional())
    .use(sm).query(({ input }) => input?.a),
});
// @ts-expect-error Missing first object's required field when an object is supplied.
or.createCaller({}).concat({ b: 1 });
// @ts-expect-error Missing first object's required field when an object is supplied.
or.createCaller({}).standalone({ b: 1 });
