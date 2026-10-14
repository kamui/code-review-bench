// Way round in the resolver: an instanceof check narrows the input to the class.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

class Money {
  private cents: number;
  constructor(cents: number) {
    this.cents = cents;
  }
  format() {
    return (this.cents / 100).toFixed(2);
  }
}
function takesMoney(m: Money) {
  return m.format();
}
const t = initTRPC.create();
export const appRouter = t.router({
  p: t.procedure
    .input(z.instanceof(Money))
    .use((opts) => opts.next())
    .query(({ input }) => {
      if (!(input instanceof Money)) throw new Error('not a Money');
      return { isMoney: true, formatted: takesMoney(input) };
    }),
});
export async function use() {
  const caller = appRouter.createCaller({});
  return [await caller.p(new Money(1234))];
}
