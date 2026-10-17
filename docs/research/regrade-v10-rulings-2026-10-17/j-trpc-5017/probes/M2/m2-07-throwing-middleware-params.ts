// What the procedure carries as its context type after a middleware that always throws, and after one that hands on never.
import { initTRPC, TRPCError } from '@trpc/server';
import type { ProcedureBuilder } from '@trpc/server/core/internals/procedureBuilder';

const t = initTRPC.context<{ user: string }>().create();
type CtxOut<T> = T extends ProcedureBuilder<infer P> ? P['_ctx_out'] : 'not a builder';
declare function neverCtx(): never;
const throwing = t.procedure.use(async () => {
  throw new TRPCError({ code: 'FORBIDDEN' });
});
const neverHandedOn = t.procedure.use((opts) => opts.next({ ctx: neverCtx() }));
type Show_ctx_out_after_throwing_middleware = CtxOut<typeof throwing>;
type Show_ctx_out_after_never_ctx = CtxOut<typeof neverHandedOn>;
type Show_ctx_out_plain = CtxOut<typeof t.procedure>;
