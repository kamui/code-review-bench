# Notes: u35

- The hard call is says_what. c3 lists mutations (verify_mode, ALPN, check_hostname=False) "on the shared object". That describes what the code does. It does not say what goes wrong for GT-i5 (a ValueError, or bad certificates accepted in other threads).
- The rule's credited example ("verify flags ... leak to every other Session") is close. If the comment's opening "leaks into every other request" is read as covering c3, the answer could flip to yes. That opening clause is tied to client certificates, though, and the rule says not to add a result the comment does not state.
- Even if check_hostname=False did reach other requests, by GT-i5's mechanism that alone is only a step. The harm needs CERT_NONE written to the context, and the comment never says that.
- Precedent: second-05 ("Requests Q2 does not recover GT-i5", no/yes) has the same outcome shape, but its comment text is not shown, so I cannot tell whether it is the same shape.
- identifies_cause is clear: the comment faults the shared context that urllib3 mutates, which is GT-i5's mechanism.
- I opened no files other than rule.md, earlier-rulings.md and cases/u35.md.
