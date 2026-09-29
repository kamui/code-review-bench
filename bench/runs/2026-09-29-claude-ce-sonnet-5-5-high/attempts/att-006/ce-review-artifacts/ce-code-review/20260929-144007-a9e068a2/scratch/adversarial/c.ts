import type { ProcedureBuilder } from '@trpc/server/core/internals/procedureBuilder';
import { initTRPC, TRPCError } from '@trpc/server';
import { z } from 'zod';
const t = initTRPC.context<{ a: string }>().create();
const tv = initTRPC.context<void>().create();
const td = initTRPC.create();
const tu = initTRPC.context<{ a: string } | undefined>().create();
const p = {
  throwOnly: t.procedure.use(() => { throw new TRPCError({ code: 'FORBIDDEN' }); }),
  throwAsync: t.procedure.use(async () => { throw new TRPCError({ code: 'FORBIDDEN' }); }),
  condThrow: t.procedure.use(async (o) => { if (Math.random() > 0.5) throw new TRPCError({ code: 'FORBIDDEN' }); return o.next(); }),
  ctxObj: t.procedure.use((o) => o.next({ ctx: { b: 1 } })),
  ctxObjChain: t.procedure.use((o) => o.next({ ctx: { b: 1 } })).use((o) => o.next({ ctx: { b: 'x' } })),
  ctxUnknown: t.procedure.use((o) => o.next({ ctx: 1 as unknown })),
  ctxUndef: t.procedure.use((o) => o.next({ ctx: undefined })),
  ctxNull: t.procedure.use((o) => o.next({ ctx: null })),
  ctxOptional: t.procedure.use((o) => o.next({ ctx: (Math.random() > 0.5 ? { b: 1 } : undefined) })),
  ctxUnion: t.procedure.use((o) => o.next({ ctx: (Math.random() > 0.5 ? { b: 1 } : { c: 1 }) })),
  voidBase: tv.procedure.use((o) => o.next({ ctx: { b: 1 } })),
  voidBaseNoNext: tv.procedure.use((o) => o.next()),
  defBase: td.procedure.use((o) => o.next({ ctx: { b: 1 } })),
  defBaseNo: td.procedure.use((o) => o.next()),
  undefBase: tu.procedure.use((o) => o.next({ ctx: { b: 1 } })),
  undefBaseNo: tu.procedure.use((o) => o.next()),
  arrIn: t.procedure.input(z.array(z.string())).use((o) => o.next()),
  dateIn: t.procedure.input(z.date()).use((o) => o.next()),
  tupleIn: t.procedure.input(z.tuple([z.string()])).use((o) => o.next()),
  setIn: t.procedure.input(z.set(z.string())).use((o) => o.next()),
  optObj: t.procedure.input(z.object({a: z.string()}).optional()).use((o) => o.next()),
  strIn: t.procedure.input(z.string()).use((o) => o.next()),
  strMwFirst: t.procedure.use((o) => o.next()).input(z.string()),
  objObj: t.procedure.input(z.object({a: z.string()})).use((o) => o.next()).input(z.object({b: z.string()})),
};
type P<B> = B extends ProcedureBuilder<infer X> ? X : never;
type Ctx = { [K in keyof typeof p]: { old: P<(typeof p)[K]>['_ctx_out']; new: P<(typeof p)[K]>['_input_in'] } };
