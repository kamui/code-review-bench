// The commoner shape of an array input, an array of records, followed by a middleware.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
type Row = { id: string; qty: number };
function saveMany(rows: Row[]) {
  return rows.length;
}
function saveOneOrMany(rows: Row | Row[]) {
  return Array.isArray(rows) ? rows.length : 1;
}
export const appRouter = t.router({
  rowsMw: t.procedure
    .input(z.array(z.object({ id: z.string(), qty: z.number() })))
    .use((opts) => opts.next())
    .mutation(({ input }) => {
      const ids: string[] = input.map((row) => row.id);
      const total: number = input.reduce((sum, row) => sum + row.qty, 0);
      const byId: Map<string, Row> = new Map(input.map((row) => [row.id, row]));
      const sorted: Row[] = [...input].sort((a, b) => a.qty - b.qty);
      const first: Row | undefined = input[0];
      return saveMany(input) + saveOneOrMany(input) + ids.length + total + byId.size + sorted.length + (first ? 1 : 0);
    }),
});
type Inputs = inferRouterInputs<typeof appRouter>;
export type Show_RowOfInput = Inputs['rowsMw'][number];
export async function caller() {
  const api = appRouter.createCaller({});
  const rows: Row[] = [{ id: 'a', qty: 1 }];
  const typed: Inputs['rowsMw'] = rows;
  return (await api.rowsMw(rows)) + typed.length;
}
