// The helper applied directly to two types that each allow `undefined`.
import type { Overwrite } from '@trpc/server/core/internals/utils';

type Same<X, Y> = (<T>() => T extends X ? 1 : 2) extends <T>() => T extends Y
  ? 1
  : 2
  ? true
  : false;

export type Show_both_optional = Overwrite<{ a: 1 } | undefined, { b: 2 } | undefined>;
export type Show_both_optional_equals_merged_or_undefined = Same<
  Overwrite<{ a: 1 } | undefined, { b: 2 } | undefined>,
  { a: 1; b: 2 } | undefined
>;
export type Show_only_second_optional = Overwrite<{ a: 1 }, { b: 2 } | undefined>;
export type Show_only_first_optional = Overwrite<{ a: 1 } | undefined, { b: 2 }>;
export type Show_same_optional_twice = Overwrite<{ a: 1 } | undefined, { a: 1 } | undefined>;
export type Show_undefined_then_object = Overwrite<undefined, { b: 2 }>;
export type Show_object_then_undefined = Overwrite<{ a: 1 }, undefined>;
export type Show_both_nullable = Overwrite<{ a: 1 } | null, { b: 2 } | null>;
