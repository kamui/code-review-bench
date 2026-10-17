// The helper itself with two array types, as the comment states it. The `true` line compiles only if the
// result is identical to number[] (TypeScript's strict identity check).
import type { Overwrite } from '@trpc/server';

type Identical<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
type Show_overwrite_string_array_number_array = Overwrite<string[], number[]>;
export const resultIsNumberArray: Identical<Overwrite<string[], number[]>, number[]> = true;
