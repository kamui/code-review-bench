import { initTRPC, experimental_standaloneMiddleware, inferRouterInputs } from './clone/packages/server/src';
import { z } from 'zod';
const t = initTRPC.context<{ user: string }>().create();
export const contextRoute = t.procedure.use(o => o.next({ ctx: undefined })).query(({ ctx }) => ctx.user);
const loose = experimental_standaloneMiddleware<{ input: { value: string } | undefined }>().create(o => o.next());
export const inputRoute = t.procedure.input(z.object({ value: z.string() })).use(loose).query(({ input }) => input.value);
export const router = t.router({ contextRoute, inputRoute });
export type CallerInput = inferRouterInputs<typeof router>['inputRoute'];
export async function runtime() {
  const caller = router.createCaller({ user: 'still present' });
  console.log('contextRoute', await caller.contextRoute());
  try { console.log('missingInput', await caller.inputRoute()); }
  catch (error) { console.log('missingInput', error instanceof Error ? error.message : error); }
}
