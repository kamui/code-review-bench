import { initTRPC, inferRouterOutputs } from '@trpc/server';
const t = initTRPC.create();
const ta = initTRPC.context<any>().create();
const r = t.router({
  ctxUndef: t.procedure.use((o) => o.next({ ctx: undefined })).query(({ ctx }) => ctx),
  ctxNullU: t.procedure.use((o) => o.next({ ctx: Math.random() > 0.5 ? { user: 'x' } : null })).query(({ ctx }) => ctx),
  ctxUnknown: t.procedure.use((o) => o.next({ ctx: {} as unknown })).query(({ ctx }) => ctx),
  ctxAnyIn: t.procedure.use((o) => o.next({ ctx: {} as any })).query(({ ctx }) => ctx),
  ctxAdd: t.procedure.use((o) => o.next({ ctx: { user: 'x' } })).query(({ ctx }) => ctx),
  ctxPlain: t.procedure.query(({ ctx }) => ctx),
  ctxTwo: t.procedure.use((o) => o.next({ ctx: { a: 1 } })).use((o) => o.next({ ctx: { b: 2 } })).query(({ ctx }) => ctx),
  ctxOverride: t.procedure.use((o) => o.next({ ctx: { a: 1 } })).use((o) => o.next({ ctx: { a: 'x' } })).query(({ ctx }) => ctx),
  anyCtx: ta.procedure.use((o) => o.next({ ctx: { a: 1 } })).query(({ ctx }) => ctx),
  anyCtxPlain: ta.procedure.query(({ ctx }) => ctx),
  anyCtxMw: ta.procedure.use((o) => o.next()).query(({ ctx }) => ctx),
});
type O = inferRouterOutputs<typeof r>;
const k: { [K in keyof O]: O[K] } = 0;
