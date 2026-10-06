import { initTRPC, inferRouterInputs, inferRouterOutputs, Overwrite } from './clone/packages/server/src';
import { z } from 'zod';

const t = initTRPC.create();
const branded = z.string().brand<'id'>();
type Branded = z.infer<typeof branded>;
function takesString(value: string) { return value.toUpperCase(); }
function takesBrand(value: Branded) { return value; }
export const router = t.router({
  plain: t.procedure.input(z.string()).use(o => o.next()).query(({ input }) => takesString(input)),
  withoutMiddleware: t.procedure.input(branded).query(({ input }) => takesString(input)),
  withMiddleware: t.procedure.input(branded).use(o => o.next()).query(({ input }) => takesString(input)),
  preserveBrand: t.procedure.input(branded).use(o => o.next()).query(({ input }) => takesBrand(input)),
});
export type BrandIsObject = Branded extends object ? true : false;
export type MergedBrand = Overwrite<Branded, Branded>;
export type BrandedCallerInput = inferRouterInputs<typeof router>['withMiddleware'];
export type BrandedResolverInput = typeof router['_def']['procedures']['withMiddleware']['_def']['_input_out'];
export type BrandedNoMiddlewareInput = typeof router['_def']['procedures']['withoutMiddleware']['_def']['_input_out'];
export type BrandedOutput = inferRouterOutputs<typeof router>['preserveBrand'];
export async function runtime() {
  const caller = router.createCaller({});
  console.log('withoutMiddleware', await caller.withoutMiddleware('hello'));
  console.log('withMiddleware', await caller.withMiddleware('hello'));
  console.log('preserveBrand', await caller.preserveBrand('hello'));
}
