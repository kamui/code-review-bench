// A generic helper that builds procedures on an instance that is not generic.
import { initTRPC } from '@trpc/server';

const t = initTRPC.context<{ userId: string }>().create();

export function withFlag<T>(flag: T) {
  return t.procedure.use(t.middleware((opts) => opts.next({ ctx: { flag } })));
}

export const router = t.router({
  foo: withFlag('on' as const).query(({ ctx }) => `${ctx.userId} ${ctx.flag}`),
});
