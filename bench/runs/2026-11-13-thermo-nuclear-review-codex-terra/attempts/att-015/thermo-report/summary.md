No actionable findings.

The local limit is 1024, matching the previous validation constant and the API server's event message validation. The truncation retains the same byte-length comparison, suffix, and slice boundary. The scheduler subtree has no remaining direct imports of the removed validation package, so removing its import-restriction exception is consistent with the checked-in imports.

The focused `TestSchedulerFailedSchedulingReasons` test passed. `git diff --check main...review-head` also passed.
