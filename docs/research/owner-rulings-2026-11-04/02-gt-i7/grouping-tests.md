# Do ground rule 4's three tests hold for the `cert_verify` route and GT-i7?

The owner chose on 2026-11-04 to widen GT-i7 to this route (round 2 reconciliation, audit call 5 and question 31). The brief for this round asked me to weigh whether the three tests hold and to say so plainly. The owner's choice stands whatever this page says. Ground rule 4: "Two symptoms are the same fault when all three hold: the same lines cause both, one fix in the project cures both, and one sentence about the cause is true of both."

The two routes:

- **GT-i7's route.** A subclass passes `ca_certs`, `ca_cert_dir` or `ca_cert_data` to its pool manager in `init_poolmanager`.
- **The `cert_verify` route.** A subclass overrides `cert_verify()`, calls the parent, and then sets `conn.ca_certs` on the pool when `verify is True`.

## Test 1: the same lines cause both

**Holds, with a caveat.**

- Both routes leak through the line the change added in `_urllib3_request_context()`, head lines 94 to 95: `pool_kwargs["ssl_context"] = _preloaded_ssl_context`. Without it, neither leaks (run: the base, and the one-context fix below).
- GT-i7's route also needs a second changed line. At the base, `cert_verify()` overwrote the pool's `ca_certs` with the default bundle on every verified request, so GT-i7's setting did nothing (run: at the base, GT-i7's adapter cannot reach its own server). At the head, `cert_verify()` no longer sets `ca_certs` for `verify=True` (head lines 297 to 317), so the setting survives and leaks.
- The `cert_verify` route does not need that second line. The subclass sets `conn.ca_certs` after the parent runs, so it took effect at the base too, privately.

So the line that makes each route leak is the same. GT-i7's route depends on one more changed line that this route does not. Ground rule 4 does not say whether "the same lines" means the lines that cause the leak or every line each route depends on.

## Test 2: one fix in the project cures both

**Holds (run).**

| Fix applied to the head | GT-i7's route: other sessions after | `cert_verify` route: other sessions after | Run |
| --- | --- | --- | --- |
| none | trust the private CA | trust the private CA | round 2 probe |
| `fix-i7.py`: skip the shared context when the pool manager carries CA settings | reject | trust the private CA | round 2 probe |
| `fix-one-context.py`: give each `verify=True` pool its own context loaded with the default bundle | reject | reject | this round, `probes/one-context-per-pool/` |

With the one-context fix, each adapter still reaches its own private-CA server (200) and the shared context's CA count stays at 121. One change to the line both routes share cures both. The 2.32.5 release fixed GT-i7 the same way, by removing the shared context (GT-i7's entry, read).

`fix-i7.py` is the smallest fix for GT-i7's own route, and it leaves this route leaking. Ruling 14's lesson says that does not decide grouping: "Group two cases by ground rule 4's three tests ... not by whether the smallest fix for one leaves the other."

## Test 3: one sentence about the cause is true of both

**Holds.** "CA material an adapter attaches to its own pools is loaded into the context every `verify=True` pool shares." Both routes reach urllib3's `ssl_wrap_socket()`, which calls `context.load_verify_locations(ca_certs, ca_cert_dir, ca_cert_data)` on the context it is given (read). GT-i7's own grouping reason already says "per-pool CA settings from an adapter are loaded into the context every verify=True pool shares".

## The rule's own examples

- *Joined:* requests, pyOpenSSL with GT-i6, "a TLS implementation switched on after import cannot reach the context built at import" (second-pass 1). That is another way to reach one fault through the same shared context. It is the closest example.
- *Kept separate:* tRPC's branded string and optional field, built by one block. There, "the smallest fix for each sits at a different line and cures only its own. One replacement of the block cures both. The user kept them separate." Here it differs: the smallest fix for the `cert_verify` route is the one-context fix, which cures both. Only GT-i7's own smallest fix leaves the other.

## Conclusion

The three tests hold. Test 1 holds for the line that makes each route leak, and GT-i7's route also depends on one more changed line. I read "the same lines" as the lines that cause the symptom, which is how ruling 14 applied it. If the owner reads it as every line each route depends on, test 1 does not strictly hold. The owner's choice to widen GT-i7 stands as the owner's either way.

One more difference for the owner to know: at the base GT-i7's route did nothing at all, while this route worked and stayed private. So the `cert_verify` route is a working setup that the change turned into a leak. GT-i7's route is a setup that never worked and now works while leaking. The result for other sessions is the same.
