M1 is a written promise under promised-4a even though the stock runner exits successfully and actual use after teardown was not found.
Pooling's ordinary retention of returned connections does not contradict the explicit teardown promise to close connections and restore pre-test conditions.
Grouping test 1: shared pool machinery contributes, but teardown skips its close at a different lifecycle site from GT-v2's setup, worker, and fallback sites.
Grouping test 2: neither targeted fix cures both faults; fix-identity also leaves retained sessions and cloning failures.
Grouping test 3: a broad sentence about stale alias pools fits both, but it does not overcome the distinct sites and independently necessary fixes.
The separate-fix precedent in second-pass 6 and last-push 7 supports keeping M1 separate; this is not one event failing at different places solely because of timing.
Confidence is medium because the complete answer needs independent grouping and promise/delivery rules, rather than one deciding rule.
The reversal facts—successful restoration and a common targeted cure—are settled against reversal by the runs; later evidence was not used.
No files outside the permitted inputs were opened or encountered.
