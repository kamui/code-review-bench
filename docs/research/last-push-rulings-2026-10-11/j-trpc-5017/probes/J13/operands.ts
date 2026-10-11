// The helper applied directly to the pairs of types the probes hand it.
import type { Overwrite } from '@trpc/server/core/internals/utils';

type Ctx = { some: 'prop' };
export type Show_object_then_unknown = Overwrite<Ctx, unknown>;
export type Show_empty_object_then_unknown = Overwrite<{}, unknown>;
export type Show_object_then_undefined = Overwrite<Ctx, undefined>;
export type Show_object_then_null = Overwrite<Ctx, null>;
export type Show_object_then_object_or_undefined = Overwrite<Ctx, { tenant: string } | undefined>;
export type Show_object_then_string = Overwrite<Ctx, string>;
export type Show_object_then_object = Overwrite<Ctx, { tenant: string }>;
export type Show_string_then_string = Overwrite<string, string>;
export type Show_unknown_then_object = Overwrite<unknown, { object: { a: string } }>;
export type Show_unknown_extends_object = unknown extends object ? true : false;
export type Show_undefined_extends_object = undefined extends object ? true : false;
