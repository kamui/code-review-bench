// The comment's own example, and the same for `readonly`.
import type { Overwrite } from '@trpc/server/core/internals/utils';

export type Show_optional_key_twice = Overwrite<{ a?: 1 }, { a?: 2 }>;
export type Show_readonly_key_twice = Overwrite<{ readonly a: 1 }, { readonly a: 2 }>;
export type Show_optional_key_kept_from_first = Overwrite<{ a?: 1; b: 1 }, { b: 2 }>;
