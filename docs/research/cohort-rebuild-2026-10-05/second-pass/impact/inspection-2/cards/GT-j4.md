# Impact card GT-j4

Pinned head `7dc04a7e94654dfad6ef1289dfe01a0a206fff3b`, base `2abb2d5cd19740be37272dac6ad7fdd36244ae54`.

Eligibility: Approved by a saved human ruling (R1). The ruling does not decide impact.

## Family

**Middleware turns a validated branded string input into a property object type, so passing it to a string function fails to compile.**

Obligation: A procedure whose validator produces a branded string must retain that value's assignability to both string and its branded string type after middleware that leaves the input unchanged. The correction to primitive input inference must cover this supported validator output as well as an unbranded string. Any design that meets this satisfies it; the patch shape is not prescribed.

Trigger: Use a whole procedure input declared with .input(z.string().brand<'id'>()), then .use(o => o.next()), and pass the resolver's input to a function accepting string or string & BRAND<'id'>. Run: both calls fail with TS2345 at the head using TypeScript 5.1.3 and Zod 3.20.2. Read: CreateProcedureReturnInput passes the parsed input through Overwrite in packages/server/src/core/internals/procedureBuilder.ts:42-44. At packages/server/src/core/internals/utils.ts:11-20, both branded operands pass the object checks and enter the property-copying block. No invalid runtime input, generic middleware factory or context typed any is needed.

Mechanism: Read: at the head, Zod represents the validated string as string & BRAND<'id'>, where BRAND is an object with a symbol property. Overwrite tests whether both operands extend object and then copies their properties. Run: Branded extends object evaluates to true, and the resulting resolver type retains the string members and brand property but is not assignable to string or the branded string type. Read: the commit before the change used extends any checks and copied the same properties for these operands. Run: the branded resolver fails at both commits, while the branded procedure without middleware compiles at both. The plain-string middleware control fails before the change and compiles at the head. Direct server calls executed despite the diagnostics return HELLO with and without middleware, and the branded pass-through returns hello, at both commits. The runtime string is intact.

## Inspection

Domain: correctness

Attribution (new-obligation): Run: this branded-string failure occurs at both the commit before the change and the head; no previously working branded resolver is shown to break. Read: the ruling requires correction of this older fault in the helper being changed. The new obligation is to cover branded strings in the primitive-input inference repair. The originating issue and added test cover plain strings, and the pre-merge discussion does not mention brands.

Consequence: Run: a server developer cannot compile a resolver that passes its validated branded string input to an ordinary string function or to a function requiring the branded string. TypeScript reports TS2345 and shows an object type with string members that is not assignable to string. This blocks those operations until the application code or library typing changes. When the saved probe executes despite the diagnostics, the direct server caller returns the expected string values with no runtime exception. The evidence shows a compiler rejection, not a rejected request or corrupted value. The same branded operations failed before the change.

Exposure: Run: the trigger is a whole input validated as a Zod branded string, middleware placed after that validator that carries its input type forward, and a resolver operation requiring string assignability. Ordinary .use(o => o.next()) is sufficient. The branded procedure without middleware compiles, and unbranded string input with middleware compiles at the head. Read: the pinned validator documentation promises inferred input and output types, and Zod 3.20.2 documents type-only brands. Reported: issue #3602, filed on 2023-01-11 before the 2023-11-10 merge, shows use of branded procedure inputs, but concerns the separate distinction between unbranded client input and branded parsed output. It does not report this resolver failure. No exact occurrence or acknowledgement was found in the saved searches and fetched threads; that coverage is limited. The prevalence of the triggering setup was not measured.

Controls: Run: the compiler reveals the failure; omitting middleware avoids it in the control. Read: the dossier identifies moving middleware before validation as a possible workaround when the middleware does not need parsed input. That does not serve middleware that consumes the parsed input, and the saved probe did not test this reordering. No library setting that preserves the type was found in the inspected path. Read: the change merged on 2023-11-10 and shipped later that day in 10.43.3. On 2023-11-17, seven days after merge, maintainers merged #5039 for generic context regressions and #5057 for middleware input requirements, released in 10.43.4 and 10.43.6 respectively. Neither names this branded-string failure. Run: it persists in 10.43.4, 10.43.6 and the later checked 10.45.2. Persistence does not establish deliberate acceptance.

Reversibility: Run: removing middleware restores string assignability in the control, and the actual string values remain intact even on the affected path. Returning to the commit before the change does not cure this fault. Read: affected developers can change the procedure composition where the middleware's role permits, or bypass the type error at the affected operation; those application changes were not executed as recovery probes. No checked later release restores the branded resolver's string assignability. The evidence establishes no permanent loss of application data or runtime value.

Grouping (confirmed): Read: the ruling accepts N1 as one separate family. Run: both failing argument types arise from the same branded resolver type at both commits. Read: GT-j1, GT-j2 and GT-j3 require different operand shapes. N2 concerns optional property markers and requires a distinct correction even though it shares Overwrite.

Evidence limits:

- Run: saved focused compiler executions use real server source at the commit before the change and the head, TypeScript 5.1.3 and Zod 3.20.2. They check branded input with and without middleware, ordinary and branded string parameters, the object gate and the plain-string control. Saved direct server calls execute those procedures despite compiler errors. The same probes at 10.43.4, 10.43.6 and 10.45.2 retain the branded failure. A saved source comparison finds no difference between the head and the 10.43.3 server source.
- Not run: a full repository test suite or distribution build, an editor session, a browser client, HTTP transport, Node 18 execution, moving middleware before validation, or a cast-based recovery. Runtime calls used Bun 1.3.10 on source; the host had Node 24.21.0, while the project declares Node ^18.17.0. The frequency of affected applications was not measured. No new probes were executed to prepare this record.
- Read: the pinned Overwrite diff, procedure builder and Zod brand definition; validator documentation; Zod 3.20.2 brand documentation; the pre-merge issue and discussion; the saved release notes and later fixes. The issue and added test address plain strings, not branded strings. The dossier and saved public searches found no exact acknowledgement, with limited search coverage.
- Reported: the author of issue #3602 used a branded string procedure input before merge. That report concerns what the client must pass, not this loss of string assignability inside the resolver. It establishes use of brands, not the frequency or an observed downstream occurrence of N1.

## Evidence

- E1
- E2
- E3
- E4
- E5
- E6
- E7
- E8
- E9
- E10
- E11
- E12
- E13
- E14
- E15
- E16
- E17
- E18
- E19
- E20
- E21
- E22
- E23
- E24
- E25
- E26
- E27
- E28
- E29
- E30
- R1

Assign `serious`, `other-material` or `unknown` under the pinned boundary, with the rule that decides it. The card states no label, no reviewer priority and no count of reviews that found the family.
