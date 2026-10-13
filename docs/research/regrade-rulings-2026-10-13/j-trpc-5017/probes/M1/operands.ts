// The helper applied directly to the same union twice, and to two plain records.
import type { Overwrite } from '@trpc/server/core/internals/utils';

type A = { type: 'a'; x: string };
type B = { type: 'b'; y: number };

export type Show_same_union_twice = Overwrite<A | B, A | B>;
export type Show_one_record_twice = Overwrite<A, A>;
export type Show_a_then_b = Overwrite<A, B>;
export type Show_b_then_a = Overwrite<B, A>;
export type Show_two_unions_of_context = Overwrite<
  { user: string } | { guest: true },
  { locale: string }
>;
