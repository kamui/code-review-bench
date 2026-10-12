// Date, Set and Map inputs followed by a middleware (inputs a data transformer such as superjson carries).
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
function takesDate(d: Date) {
  return d.getTime();
}
function takesSet(s: Set<string>) {
  return s.size;
}
function takesMap(m: Map<string, number>) {
  return m.size;
}
export const appRouter = t.router({
  dateMw: t.procedure
    .input(z.date())
    .use((opts) => opts.next())
    .query(({ input }) => {
      const later = new Date(input);
      const plain: Date = input;
      return takesDate(input) + later.getTime() + plain.getTime() + (input instanceof Date ? 1 : 0);
    }),
  setMw: t.procedure
    .input(z.set(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      const plain: Set<string> = input;
      return takesSet(input) + [...input].length + plain.size;
    }),
  mapMw: t.procedure
    .input(z.map(z.string(), z.number()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      const plain: Map<string, number> = input;
      return takesMap(input) + [...input.keys()].length + plain.size;
    }),
});
type Inputs = inferRouterInputs<typeof appRouter>;
export async function caller() {
  const api = appRouter.createCaller({});
  await api.dateMw(new Date());
  await api.setMw(new Set(['a']));
  await api.mapMw(new Map([['a', 1]]));
  const d: Inputs['dateMw'] = new Date();
  return d;
}
