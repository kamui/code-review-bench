# M3b: an array input reaching `Overwrite` "from chained `.input()` calls" (split from M3)

Pull request: trpc/trpc #5017. Base `2abb2d5c`, head `7dc04a7e`. Cut-off: the last push, 2023-11-09T22:08:09Z. Candidate covered: `NC-b3ad806d202a` (the part about the route). Probes: [`../probes/M3b/`](../probes/M3b/). This is the route that the comment behind C2 names, so it is also C2's own candidate (the brief's `C2n`).

## Problem

The comment says: "The `_input_in` and `_input_out` slots in `packages/server/src/core/internals/procedureBuilder.ts` (lines 38-45) feed `Overwrite` from chained `.input()` calls, so a procedure whose earlier or later input is an array type reaches this path."

What the runs show (*run*, identical at base and head):

- `.input(z.array(z.string())).input(z.array(z.number()))` is refused on the second `.input()`: TS2345 "Argument of type 'ZodArray<ZodNumber, "many">' is not assignable to parameter of type '"All input parsers did not resolve to an object"'."
- `.input(z.object({ a: z.string() })).input(z.array(z.number()))` is refused with the same message.
- `.input(z.array(z.string())).input(z.object({ a: z.string() }))` is refused: "Argument of type 'ZodObject<{ a: ZodString; }, "strip", ZodTypeAny, { a: string; }, { a: string; }>' is not assignable to parameter of type '"All input parsers did not resolve to an object"'."

So no procedure with an array input compiles with chained `.input()` calls, at either commit.

## What changed

Nothing on this route. `ProcedureBuilder.input()` (`procedureBuilder.ts` lines 92 to 119, untouched) merges chained inputs with `OverwriteIfDefined`, which is `Simplify<TType & TWith>`, not `Overwrite`, and refuses a second input when either type is not an object (*read*). The slots the comment names, lines 38 to 45, are in `CreateProcedureReturnInput`. That builds the type after `.use()` (line 145) and `.unstable_concat()` (line 153), not after `.input()` (*read*). The pull request changed only `Overwrite` and added a test (*read*).

## Promised?

Not asked: the needed fact is false. The route the claim rests on does not reach the helper, and the builder refuses it at both commits (run).

## Intended or announced

The builder's refusal of a second input that is not an object was designed in #2498 (2022, read). The change does not mention chained inputs (read). Released in 10.43.3 (read, later).

## What the affected person sees

A developer who chains `.input()` calls with an array gets the builder's error "All input parsers did not resolve to an object" on the second call, before and after the change. This error is deliberate. In pull request #2498 (2022-08-22) a maintainer wrote: "I think we should only allow multiple inputs for object-like input parsers" (`../upstream/pr-2498-comments.json`, *read*, before the cut-off).

## What the maintainers did

They designed the refusal in 2022 (read). Nothing later touches it (run: the same refusal at 10.43.4, 10.43.6 and 10.45.2, later).

## How each fact is known

The refusals: *run* at base, head and three later releases. The route through `OverwriteIfDefined` and the place of lines 38 to 45: *read* at the head. The comment's statement: *reported*, and refuted by the run.

## Relation to existing reference families and ruled claims

Not decided by any saved ruling on this route. GT-j7's record already notes: "Two array inputs in a row do not reach this path, because the builder refuses a second input that is not an object (run)." The nearest ruling of the same shape is regrade ruling 26 (`regrade-26`). There a comment placed a known problem's result on two routes where it does not happen, and the owner chose "No credit; own routes refuted". The candidate record reads these facts correctly.

**Grouping tests.** Not applicable: this route has no failing case for a fix to cure. GT-j7's saved fix leaves the refusal as it is (run, `../probes/M3a/results/result-gt-j7-array-check.txt`).

**Clean control.** The control state is `known-problems`.

## Both sides

*For refuted.* The claim's route is chained `.input()` calls. The builder refuses every such chain with an array, at both commits, and chained inputs do not use `Overwrite` (run, read).

*Against.* The comment also says "a procedure whose earlier or later input is an array type reaches this path". With `.use()` or `.unstable_concat()` after an array input that is true, and that is GT-j7 (M3a). Read loosely, the wrong route is a slip in an explanation.

## Recommendation

> **What happens:** nothing; a chained `.input()` with an array is refused by the builder at both commits, and chained inputs never reach `Overwrite`. **Promised:** not asked. **Delivered:** not asked. **So: refuted.** **Band, decided separately:** none.

Confidence: high. The facts are run and read. Rule Before 1 decides it ("If a needed fact is false, the comment is refuted"), and nothing points the other way. Regrade ruling 26 went the same way for a comment's own routes. The fact that would reverse it is a chained `.input()` with an array that compiles and reaches the helper, and the runs show none.
