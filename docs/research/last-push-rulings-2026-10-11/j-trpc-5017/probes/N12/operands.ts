// The helper applied directly to the pairs of types the probes hand it.
import type { Overwrite } from '@trpc/server/core/internals/utils';
import { z } from 'zod';

const userId = z.string().brand<'id'>();
type UserId = z.infer<typeof userId>;
type Optional = { a?: string };

export type Show_brand_extends_object = UserId extends object ? true : false;
export type Show_brand_extends_string = UserId extends string ? true : false;
export type Show_optional_extends_object = Optional extends object ? true : false;
export type Show_brand_then_brand_is_a_string = Overwrite<UserId, UserId> extends string ? true : false;
export type Show_optional_then_optional = Overwrite<Optional, Optional>;
export type Show_optional_then_optional_accepts_empty = {} extends Overwrite<Optional, Optional> ? true : false;
export type Show_readonly_then_readonly = Overwrite<{ readonly a: string }, { readonly a: string }>;
export type Show_brand_then_brand = Overwrite<UserId, UserId>;
