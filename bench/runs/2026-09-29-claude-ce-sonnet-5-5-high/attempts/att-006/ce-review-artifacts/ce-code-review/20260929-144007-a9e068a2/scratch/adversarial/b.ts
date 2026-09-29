import { inferRouterInputs, inferRouterOutputs, initTRPC } from '@trpc/server';
import { z } from 'zod';
import { Overwrite } from '@trpc/server/core/internals/utils';

type Eq<A,B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
const t = initTRPC.context<{ a: string } >().create();
const r = t.router({
  arr: t.procedure.input(z.array(z.string())).use((o) => o.next()).query(({ input }) => input),
  arrNoMw: t.procedure.input(z.array(z.string())).query(({ input }) => input),
  date: t.procedure.input(z.date()).use((o) => o.next()).query(({ input }) => input),
  tuple: t.procedure.input(z.tuple([z.string(), z.number()])).use((o) => o.next()).query(({ input }) => input),
  optstr: t.procedure.input(z.string().optional()).use((o) => o.next()).query(({ input }) => input),
  num: t.procedure.input(z.number()).use((o) => o.next()).query(({ input }) => input),
  bool: t.procedure.input(z.boolean().nullish()).use((o) => o.next()).query(({ input }) => input),
  rec: t.procedure.input(z.record(z.string())).use((o) => o.next()).query(({ input }) => input),
  obj: t.procedure.input(z.object({a: z.string()})).use((o) => o.next()).query(({ input }) => input),
  objopt: t.procedure.input(z.object({a: z.string()}).optional()).use((o) => o.next()).query(({ input }) => input),
  union: t.procedure.input(z.union([z.string(), z.object({a: z.string()})])).use((o) => o.next()).query(({ input }) => input),
  map: t.procedure.input(z.map(z.string(), z.string())).use((o) => o.next()).query(({ input }) => input),
});
type I = inferRouterInputs<typeof r>;
type O = inferRouterOutputs<typeof r>;
const i_arr: I['arr'] = null as unknown as never; const _i_arr: {__show: I['arr']} = 1 as any as string;
const i_arrNoMw: I['arrNoMw'] = null as unknown as never; const _i_arrNoMw: {__show: I['arrNoMw']} = 1 as any as string;
const i_date: I['date'] = null as unknown as never; const _i_date: {__show: I['date']} = 1 as any as string;
const i_tuple: I['tuple'] = null as unknown as never; const _i_tuple: {__show: I['tuple']} = 1 as any as string;
const i_optstr: I['optstr'] = null as unknown as never; const _i_optstr: {__show: I['optstr']} = 1 as any as string;
const i_num: I['num'] = null as unknown as never; const _i_num: {__show: I['num']} = 1 as any as string;
const i_bool: I['bool'] = null as unknown as never; const _i_bool: {__show: I['bool']} = 1 as any as string;
const i_rec: I['rec'] = null as unknown as never; const _i_rec: {__show: I['rec']} = 1 as any as string;
const i_obj: I['obj'] = null as unknown as never; const _i_obj: {__show: I['obj']} = 1 as any as string;
const i_objopt: I['objopt'] = null as unknown as never; const _i_objopt: {__show: I['objopt']} = 1 as any as string;
const i_union: I['union'] = null as unknown as never; const _i_union: {__show: I['union']} = 1 as any as string;
const i_map: I['map'] = null as unknown as never; const _i_map: {__show: I['map']} = 1 as any as string;
