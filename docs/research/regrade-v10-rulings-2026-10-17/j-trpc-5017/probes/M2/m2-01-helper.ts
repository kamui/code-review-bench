// The helper itself: what Overwrite gives when the second type (or the first) is never.
import type { Overwrite } from '@trpc/server/core/internals/utils';

type Show_object_then_never = Overwrite<{ a: string }, never>;
type Show_string_then_never = Overwrite<string, never>;
type Show_undefined_then_never = Overwrite<undefined, never>;
type Show_never_then_object = Overwrite<never, { a: string }>;
type Show_object_then_object = Overwrite<{ a: string }, { b: number }>;
type IsNever<T> = [T] extends [never] ? true : false;
export const objectThenNeverIsNever: IsNever<Overwrite<{ a: string }, never>> = true;
export const stringThenNeverIsNever: IsNever<Overwrite<string, never>> = true;
