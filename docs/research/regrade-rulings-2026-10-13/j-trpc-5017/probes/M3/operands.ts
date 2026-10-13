// The helper applied directly with `any` as its first type.
import type { Overwrite } from '@trpc/server/core/internals/utils';

// what a t.middleware() function's ctx is built from: the root context and {}
export type Show_any_then_empty_object = Overwrite<any, {}>;
// what the known problem's resolver is built from: the root context and the added keys
export type Show_any_then_added_keys = Overwrite<any, { user: number }>;
// what a function passed straight to .use() is built from
export type Show_any_then_any = Overwrite<any, any>;
