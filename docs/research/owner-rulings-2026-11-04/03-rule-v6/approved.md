# Rule version 6: wording the owner approved

Each piece was shown to the owner as below. On 2026-11-04 the owner asked for piece 1's first draft to be restated plainly ("use $bro on the wording, then revise from there and then re-present it to me"), and approved the restated wording.

## Piece 1 (section 1), approved

Question as shown: "Piece 1, plain-language version (shown above): when the claim list splits 'X is wrong, so Y breaks' into two claims, the grader grades them as one, gives both the same answers, and notes it; claims about different failures with different causes stay separate. Accept this wording?"
Options shown: "Accept (Recommended)", "Change the wording", "Keep rule v5". Option chosen: "Accept (Recommended)": "Use this plain wording for piece 1 in rule version 6."

> **When a comment's cause and its result are split into two claims, grade them as one.**
>
> A review comment often says "X is wrong, so Y breaks." Sometimes the claim list splits that into two claims, one for X and one for Y. When it does, grade the two together, as one claim, for every question. They count as one thought when they're in the same sentence, or when the second sentence clearly refers back to the first ("so", "this means", "in both cases"). Give both claims the same answers, and note in each that you graded them together.
>
> If you're the one splitting a comment into claims, don't separate an explanation from the result it explains, even when the result is in the next sentence.
>
> *Example:* a comment says a database cursor is connected to the very database being dropped. The next sentence starts "In both cases" and says the drop fails because the pool keeps the session open. Graded together, it says the drop fails because the cursor is connected to the database being dropped. That's true, and it says what breaks. Blaming the pool is a wrong detail; it doesn't make the point false.
>
> *Don't combine* two claims that describe different failures with different causes. Grade those separately (section 3, "A claim can't borrow another claim's failure").

## Piece 2 (section 3), approved

The owner asked for the plain version to be restated again ("do the same, use $bro and revise then re-present it"), and approved the restated wording.

Question as shown: "Piece 2, plainer version (shown above): 'Don't mix up two separate points': grade each unrelated point on what it says, and don't give it credit for a failure only the other point describes; a cause plus its result is one point (section 1). The example is a made-up cache-key / time-zone pair. Accept?"
Options shown: "Accept (Recommended)", "Use a real example", "Change the wording". Option chosen: "Accept (Recommended)": "Use this wording for piece 2."

> **Don't mix up two separate points.**
>
> Sometimes one comment makes two unrelated points, each with its own cause. Grade each point only on what it says. Don't give one point credit for a failure that only the other point describes.
>
> This doesn't apply to a cause and the result the comment ties to it. That's a single point, and section 1 says to grade it as one.
>
> *Example:* a comment says the new cache key leaves out the user's id. Further down, it says the cleanup job deletes live entries because it compares times in two different time zones. Those are two separate points. The deleted entries don't count toward the cache-key point.

## Piece 3 (section 3), approved

Question as shown: "Piece 3 (shown above): in the credit section, add 'If section 1 had you grade two claims together, treat them as one claim here too: their combined words are "its own words."' Accept?"
Options shown: "Accept (Recommended)", "Change the wording". Option chosen: "Accept (Recommended)": "Use this wording for piece 3."

> After "Judge the claim by its own words." in section 3: If section 1 had you grade two claims together, treat them as one claim here too: their combined words are "its own words."
