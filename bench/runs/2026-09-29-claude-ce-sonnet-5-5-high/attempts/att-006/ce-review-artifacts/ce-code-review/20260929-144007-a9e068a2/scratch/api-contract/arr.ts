import { initTRPC, inferRouterInputs } from '@trpc/server';
import { z } from 'zod';
const t = initTRPC.create();
const r = t.router({
  arr: t.procedure.input(z.array(z.string())).use((o) => o.next()).query(({ input }) => input),
  date: t.procedure.input(z.date()).use((o) => o.next()).query(({ input }) => input),
});
type I = inferRouterInputs<typeof r>;
declare const a: I['arr'];
const s: string[] = a;
const s2: ReadonlyArray<string> = a;
const m = a.map((x) => x);
const n: string[] = a.filter(Boolean);
declare const d: I['date'];
const dd: Date = d;
const t2 = d.getTime();
