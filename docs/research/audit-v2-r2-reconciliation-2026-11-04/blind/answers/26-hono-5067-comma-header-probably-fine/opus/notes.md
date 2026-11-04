# Notes

- u33 and u34 have the same content (same comment, sentence, known problem and facts), so I gave them the same answer.
- The hard part is that the comment states a concrete wrong output ("now gets `{}`" for `multipart/form-data,foo`) but then says "The change is probably fine". The rule covers withdrawn statements. It does not cover a hedge, and it does not say whether a hedge removes the "why this matters".
- The comment describes the old behaviour as "sent to the form parser", not "raised an error". GT-p6's harm is the lost error, and reading the fields is not promised (3a). I credited the stated result ({} with no error) without adding the old error, but the owner could read the implied harm as the unpromised lost fields.
- Cause: the comment names the line-106 switch from startsWith to equality, but its only objection is that the PR does not call the change out. So I can't tell whether it objects to the cause itself.
- Precedent: regrade-25 (same PR, no credit) describes new behaviour with no output named. That is a different shape from this case, which names an input and states a wrong output. No ruling covers a hedged claim that states an output.
- No other files were seen besides rule.md, earlier-rulings.md and the two case files.
