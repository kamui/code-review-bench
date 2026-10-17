// Ordinary uses of a Date input after a middleware. Every line is valid for a Date.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
function takesDate(d: Date) {
  return d.getTime();
}
function generic<T extends Date>(d: T): T {
  return d;
}
function takesDateOrString(d: Date | string) {
  return String(d);
}
export const appRouter = t.router({
  dateMw: t.procedure
    .input(z.date())
    .use((opts) => opts.next())
    .query(({ input }) => {
      const a: Date = input;
      const b = new Date(input);
      const c = takesDate(input) + takesDate(generic(input));
      const d = input.toISOString() + input.toLocaleDateString('en-US', { month: 'long' });
      const e = input < new Date() ? 1 : 0;
      const f = input.getTime() - Date.now();
      const g = +input;
      const h = JSON.stringify({ at: input });
      const i = [input, new Date()].sort((x, y) => x.getTime() - y.getTime());
      const j = new Intl.DateTimeFormat('en-US').format(input);
      const k = structuredClone(input).getTime();
      const l = `${input}`;
      const m = takesDateOrString(input);
      const n = input instanceof Date ? input.getFullYear() : 0;
      const o: number = input.valueOf();
      const p = Math.max(input.getTime(), b.getTime());
      const q = input.toJSON();
      const r: Date[] = [input];
      const s = new Map<Date, number>([[input, 1]]);
      return [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s];
    }),
});
