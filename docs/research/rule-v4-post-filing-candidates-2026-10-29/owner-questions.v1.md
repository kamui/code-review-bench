# Questions for the owner, prepared 2026-10-29

Each question is written to be shown alone. Every fact is marked *run* (executed on this machine at both commits, probe saved), *read* (in the diff, the source or a fetched record) or *reported* (someone's claim not checked). The dossiers, cases and probes are in each pull request's folder. Question 0 comes first, because it decides how the other four are recorded.

## Question 0: how this round's answers are recorded

**The facts.** You chose on 2026-10-29 that Claude Opus 5.5 writes the fact sheets and two blind Claude sessions (Opus and Sonnet) answer. The session that asks you is also Claude. The record rules say a blind answer must come from a different model family than the asking session's. `bench-tools ruling route` refuses all ten of this round's records for that reason alone (*run*). With the family field set aside, every record passes, and every one goes to you, so nothing would be settled without you (*run*, on throwaway copies that were then deleted). `ruling summary` checks every record in the repository, so saving a refused record would break it for every round. All first answers are already committed, before any question: mine at `620dd06a0` and the blind ones in the commit after it.

**Question:** "This round's blind answers are Claude, like the session asking you, and the record tool refuses a blind answer from the asker's model family. How should these four rulings be recorded?"

- **"Allow it for this round, with your receipt (Recommended)"**: a small tool change lets a record carry your saved receipt that permits same-family blind answers for one named round. Such a record always goes to you and counts nothing toward agents settling decisions later. The records are then written as usual.
- **"Record without blind parties"**: the records keep only my first answer. The blind answers stay saved as files beside them and are shown to you, but are not in the record. No tool change.
- **"Skip the records this round"**: ask from the committed answer files and save your answers in the round's files only. No before or after records, so this round adds nothing to the learning count.

## Question 1: Django PR 17914, a subclass's time zone override no longer runs

**What the change did.** The pull request adds an optional connection pool for PostgreSQL. Along the way it rewrote how Django sets up each new connection. Before, setup called the backend's own `ensure_timezone()` method. A custom database backend built on Django's PostgreSQL backend (a subclass) could override that method, to choose its own session time zone or to skip time zone SQL its server cannot run. After, setup calls a new module function directly, so an override never runs (*read*, `django/db/backends/postgresql/base.py`, head line 376, where the base called `self.ensure_timezone()`).

**What a user sees** (*run*, PostgreSQL 16 on this machine). A subclass that sets its sessions to Asia/Tokyo gets Tokyo before the change, so midnight UTC reads as hour 9. After the change it gets UTC (hour 0), with or without the pool, and no error. Anything the database computes in local time, such as today's date or an hour of the day, shifts by the zone difference.

**The known problem it may belong to.** GT-v5 (serious) is the same skipped override for a QuestDB backend, which uses it to avoid SQL QuestDB cannot run, so connecting fails. *Run* (emulated, no real QuestDB): it connects before the change and fails after. The three grouping tests all hold (*run*): both fail on line 376, one fix (call the method again there) cures both, and one cause sentence fits both. GT-v5's saved outcome already says "Preserve the backend's timezone override during physical connection initialization", but its title names only the QuestDB failure. GT-v11 (serious, a skipped `ensure_role()` override) is a different method on other lines; the same fix leaves it failing (*run*).

**Promised?** Django's docs allow subclassing a backend "to modify its behavior, features, or configuration", and second-pass ruling 21 read that as covering methods without a leading underscore (*read*). `ensure_timezone` has none, and the override ran at every connect before the change (*run*).

**Maintainers** (*read*, after the merge). Ticket 35688 (August 2024, a QuestDB backend) was accepted as a release-blocking "Regression in fad334e1…", this change's commit. Django 5.1.1 then added new overridable hooks; its release note says it "Restored, following a regression in Django 5.1, the ability to override the timezone and role setting behavior". A subclass that overrides only `ensure_timezone()` is still not called at connect on current `main`. Nobody spoke of a subclass choosing its own time zone. These later facts confirm the weak point; they are not the reason for the answer.

**Not run:** a real QuestDB server, a real third-party backend, the pooled variants under any patch (no patch cures those for either case).

**First answers.** All three: the same fault as GT-v5. Me medium (no earlier ruling widened a family this way), blind Opus high, blind Sonnet medium (grouping is yours). Label if kept apart: serious from all three, because sessions silently run in the wrong time zone with nothing pointing at the cause (S4). Against serious: only subclasses that override this method are hit, and you labelled GT-v16, a narrow pooled time zone case, other-material.

**Proposed wording if joined:** GT-v5 becomes "Connection setup no longer runs a subclass's `ensure_timezone()` override, so a backend that uses it to skip unsupported SQL fails to connect, and one that uses it to choose a time zone silently gets another." Its serious label stays.

**Question:** "A Django backend subclass's own time zone choice is silently ignored after this change, from the same skipped method call as GT-v5. Is it part of GT-v5?"

- "Part of GT-v5, widen its wording (Recommended)"
- "A separate new problem, serious"
- "A separate new problem, other-material"
- "Not a problem"

## Question 2: Django PR 17914, the pool's persistent-connection check runs only once

**What the change did.** The new pool refuses `CONN_MAX_AGE` above 0, which keeps a connection open between requests. The error is "Pooling doesn't support persistent connections." The check runs only when an alias's pool is first built (*read*, head lines 208 to 212). The pool is shared by every connection object for that alias.

**What a user sees** (*run*). With settings fixed at start-up, the first query raises the error, as built. After a connection with `CONN_MAX_AGE = 0` has built the pool, a second connection object for the same alias whose settings say 60 gets no error. That can be a `copy()` of a connection with its settings edited, or the live `DATABASES` entry changed while running. It then keeps its pool slot across requests, and with a one-slot pool the next thread fails after 2 seconds with "couldn't get a connection after 2.00 sec". Before the change the pool option did not exist.

**Promised?** No. Django's docs say "You shouldn't alter settings in your applications at runtime" (*read*). Before the cut-off, two maintainers said on PR 17594, about this same per-alias pool, that changing the `DATABASES` entry after the fact is not supported (*read*). `copy()` is not in Django's documentation; its docstring says "For tests that require two connections to the same database" (*read*). Django's own test of this check edits a copy's settings, but before the pool exists, where the check fires (*read*). Regrade ruling 19 made a pool fault that needed settings changed at run time a suggestion.

**Maintainers** (*read*): no acknowledgement; the check is unchanged on `main`.

**Not run:** a full request cycle under a web server (the end-of-request cleanup was called directly).

**First answers.** All three: a suggestion, outside supported use. Me medium, blind Opus low, blind Sonnet medium. All name the same open point: whether "don't change settings at run time" also covers editing a copied connection's settings. Label if ruled a problem: other-material from all three, because nothing is harmed in the normal setup and the result is a held slot and a timeout.

**Question:** "Django's new pool check misses a persistent-connection setting only when settings change after start-up, which Django says not to do. Is that a problem?"

- "Not a problem: outside supported use (Recommended)"
- "A new problem, other-material"
- "A new problem, serious"

## Question 3: ripgrep PR 2957, loading completion by `eval` fails under two zsh options

**What the change did.** The pull request lets zsh users load ripgrep's completion with `source <(rg --generate complete-zsh)`. To make that work, the generated script's last line became a check that reads `$funcstack[1]`, the name of the function or file now running, to decide whether to register completion or run it (*read*).

**The two settings.** `nounset` makes reading an unset variable an error (off by default). `no_eval_lineno` stops `eval` from adding an entry to `funcstack` (the default adds one). With both set, at the top level of a prompt, `funcstack` is empty (*read*, the zsh manual).

**What a user sees** (*run*, interactive zsh 5.8.1 at both commits). With both options set, `eval "$(rg --generate=complete-zsh)"` typed at the prompt prints "zsh: funcstack[1]: parameter not set", registers nothing, and Tab completes nothing. The message names neither ripgrep nor the options. Before the change the same typed `eval` also failed, with another message. Under the same options, the same `eval` in `.zshrc` works, and so do the FAQ's `source` line (typed or in `.zshrc`) and its `fpath` file. zsh's own completion works under both options.

**Promised?** Announced: the description says "Now, you can dynamically source completions in zsh by running" the `source <(…)` line, and the author used `eval "$(rg --generate=complete-zsh)"` in `.zshrc` in the review thread (*read*). Promised 5 says a promise is not limited to the one way shown and covers valid platform settings under which the platform's own feature works. Nothing in the docs mentions `eval` or shell options (*read*).

**How common:** not measured. A code search found a few public configurations that set one of the options, and none that sets both by default (*read*).

**Other tools** (*run*, 2026 versions, after the cut-off): `rustup` and `uv` fail the same way; `gh` and `kubectl` print the error but still register.

**Known problems.** The grouping tests keep it apart from GT-n4 to GT-n7: no cause sentence fits both (*run*). Giving the read a default (`${funcstack[1]-}`) cures this case only.

**Maintainers** (*read*): nobody mentioned shell options; no report or fix was found.

**Nearest rulings:** second-pass 9 (the new check prints an error under the `KSH_ARRAYS` option: a problem) and rule-v4 ruling 23 (a valid setting breaks the new loading route: a problem, other-material).

**First answers.** All three: a problem, low confidence. All name the same open point: whether an `eval` typed at the prompt is an ordinary way of the announced `source` loading, when it fails only under two non-default options and the documented line works. Label: other-material from all three (me medium, Opus medium, Sonnet high), because nothing that worked before breaks and the documented ways work.

**Question:** "With two non-default zsh options, loading ripgrep's completion by `eval` typed at the prompt fails, while the documented `source` line works. Is that a problem?"

- "A new problem, other-material (Recommended)"
- "A new problem, serious"
- "Not a problem: outside supported use"

## Question 4: Hono PR 5067, two more headers now give an empty form

**What the change did.** The pull request (Fixes #5060) makes `parseBody()` accept form types in any letter case. Before, it treated a request as a multipart form when the `Content-Type` header started with `multipart/form-data`. Now the text before the first `;`, trimmed and lower-cased, must equal `multipart/form-data` exactly (*read*, `src/utils/body.ts`, head lines 104 to 106). Anything else returns `{}` without reading the body.

**Already on the answer key.** GT-p6 (other-material, your ruling 16): a header missing the `;` before `boundary` now gives `{}` with no error, where Node raised one. The ground was that a check that reported invalid input is promised. Your ruling 14 joined a second header form on the same line to an existing problem.

**Two new cases** (*run*, Node 24.21.0, Bun 1.3.10 and Deno 2.8.3, in process and over a real socket):

- **(a) a comma inside the type**, `multipart/form-data,foo; boundary=X`. HTTP's grammar does not allow it (*read*, RFC 9110). Before: Node and Deno threw a TypeError (HTTP 500 unless caught); Bun read the fields. After: `{}` on all three, HTTP 200.
- **(b) a well-formed different type name**, `multipart/form-data-extra; boundary=X`. Before: Node threw "Content-Type was not one of …", the same message it gives `text/plain`; Deno threw; Bun read the fields. After: `{}`, exactly as `multipart/mixed`, `text/plain` and JSON get at both commits. Hono's docs, its migration guide, a test and maintainers since 2022 (#428, #487) say `parseBody()` reads only the two form types and returns `{}` for others (*read*).

**Grouping** (*run*): both fail on GT-p6's line, one prefix-test expression restores the earlier behaviour for GT-p6 and both cases, and one cause sentence fits all three. Whether to group turns on whether each is a fault: (a) is invalid input, like GT-p6; (b) is valid input of a type Hono says it does not read.

**Not established:** Cloudflare Workers was not run; how often clients send these headers was not measured, and nobody was found sending either.

**Maintainers** (*read*): no comment before or after the merge, no later issue, and the line is unchanged on `main`.

**First answers.** (a): all three say part of GT-p6 (me high, Opus medium, Sonnet medium; both blind sessions noted that on Bun only a tolerated read was lost, which GT-p6's wording already treats as not promised). (b): all three say a suggestion, outside supported use (me low, Opus medium, Sonnet medium). Labels: other-material from all three for each, which is GT-p6's label.

**Proposed wording if (a) joins:** GT-p6 becomes "`parseBody()` returns `{}` with no error for a multipart request whose Content-Type is malformed before its first `;`, such as `multipart/form-data boundary=X` (no `;`) or `multipart/form-data,foo; boundary=X` (a comma inside the type), where on Node and Deno it raised the runtime's error before the change." Its other-material label stays.

**Question:** "Two more Hono headers now get an empty form with no error: (a) a malformed one with a comma in the type, and (b) a well-formed type name Hono says it doesn't read. How should they be ruled?"

- "(a) joins GT-p6 with wider wording, (b) is not a problem (Recommended)"
- "Both join GT-p6"
- "Neither is a problem"
