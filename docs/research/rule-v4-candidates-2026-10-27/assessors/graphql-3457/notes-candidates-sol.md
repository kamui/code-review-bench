The main uncertainty is whether documented string equivalence also promises equivalence for static field merging; I lean yes under promised-4a.
The accepted AST type and separate parser tests support both forms, but alone do not promise merging their combination.
Under before-2, the functional rewrite of the comparison keeps this in scope despite identical base behavior; first-06 is the closest scope precedent.
For before-4, the cases share comparison machinery, but the isolated fixes cure different causes independently: retained string-form metadata, rounded name ordering, and repeated wrapper work.
String normalization leaves GT-w1 and GT-w2 intact; their respective fixes leave mixed-string rejection intact, so I keep this separate.
No supplied ruling resolves the same promise question; regrade-10 concerns performance grouping instead.
Later implementation choices neither establish nor withdraw a promise at the cutoff.
No files outside the permitted rule, earlier rulings, and case files were opened.
