// The comment's statements about the helper: which types pass `extends object`,
// and what `Overwrite` builds for each of the kinds the comment lists.
import type { Overwrite } from '@trpc/server/core/internals/utils';

class Money {
  private cents: number;
  constructor(cents: number) {
    this.cents = cents;
  }
  format() {
    return (this.cents / 100).toFixed(2);
  }
}
class Point {
  constructor(public x: number, public y: number) {}
  norm() {
    return Math.hypot(this.x, this.y);
  }
}
type Fn = (x: number) => number;
type Identical<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
type IsObject<T> = T extends object ? true : false;

type Show_object_check_array = IsObject<string[]>;
type Show_object_check_date = IsObject<Date>;
type Show_object_check_map = IsObject<Map<string, number>>;
type Show_object_check_function = IsObject<Fn>;
type Show_object_check_class_private = IsObject<Money>;
type Show_object_check_class_public = IsObject<Point>;

type Show_array_with_b = Overwrite<string[], { b: 2 }>;
type Show_array_with_array = Overwrite<string[], string[]>;
type Show_date = Overwrite<Date, Date>;
type Show_map = Overwrite<Map<string, number>, Map<string, number>>;
type Show_function = Overwrite<Fn, Fn>;
type Show_class_private = Overwrite<Money, Money>;
type Show_class_public = Overwrite<Point, Point>;
type Show_optional = Overwrite<{ a?: string }, { a?: string }>;

export const arrayWithBHasConcat: 'concat' extends keyof Overwrite<string[], { b: 2 }> ? true : false = true;
export const arrayWithBHasIterator: typeof Symbol.iterator extends keyof Overwrite<string[], { b: 2 }> ? true : false = true;
export const arrayWithBHasB: Overwrite<string[], { b: 2 }>['b'] = 2;
export const arrayIdentical: Identical<Overwrite<string[], string[]>, string[]> = true;
export const dateIdentical: Identical<Overwrite<Date, Date>, Date> = true;
export const mapIdentical: Identical<Overwrite<Map<string, number>, Map<string, number>>, Map<string, number>> = true;
export const functionIdentical: Identical<Overwrite<Fn, Fn>, Fn> = true;
export const classPrivateIdentical: Identical<Overwrite<Money, Money>, Money> = true;
export const classPublicIdentical: Identical<Overwrite<Point, Point>, Point> = true;
export const optionalIdentical: Identical<Overwrite<{ a?: string }, { a?: string }>, { a?: string }> = true;
