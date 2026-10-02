# Lead log (Opus 5.5 High, Stage 2)

Decision trail kept while sourcing. Status: shortlisted / rejected / pending. "Base" means the true merge-base of the review head, not GitHub's current `base.sha`.

| Lead | Bucket | Status | Note |
| --- | --- | --- | --- |
| django#16943 (mappings for choices) | A | shortlisted | `django.utils.choices` imports `django.db.models.enums` at module level at head dfe3475f; cycle breaks `from django import forms`; ticket #34807 release blocker; fixed by Fellow nessita in #17215. Base 1ac39767. |
| django#17914 (PostgreSQL pool) | B | shortlisted | Setters moved to module-level functions; subclass overrides of `ensure_timezone` silently bypassed and `ensure_role` removed at head fad334e1; #35688 release blocker (Sarah Boyce); pre-merge felixxm asked timgraham for 3rd-party backend review. Base bcccea3e. |
| kubernetes#115082 (plugin_evaluation_total) | C | shortlisted | `WithLabelValues` in per-node Filter loop; #117592 pprof throughput <100 pods/s at scale (tosi3k, sig-scalability); fix #117594 caches metric. Base eabb7083, head 608f4808. |
| kubernetes#142495 (kube-scheduler/framework deps) | A | rejected (weak) | Self-approved cleanup of dependency closure (+25 CEL pkgs). No stated obligation or material consequence; introducing PR not traced. |
| django ticket #36520 (parse_header_parameters 5x) | C | rejected | Constant per-call cost (500 ns -> 2.65 us), not growth with a supported dimension. |
| django ticket #35442 (only() N+1) | C | rejected | Long-standing behaviour (v4.2), no introducing PR identified. |
| kubernetes#132132 (paginated lists fall back to etcd, 1.33) | C | pending | Need introducing PR (1.33 snapshot/consistent-list work) and measurement. |
| django#10673 (MigrationWriter serializer registry) | A | shortlisted | Module-level `from django.db.migrations.writer import MigrationWriter` in contrib/postgres/apps.py; docs forbid model imports at AppConfig module level; #30111 release blocker (Nick Pope "Relates to e192223e"). Base 3c01fe30, head e192223e. Reproduction conditions pending. |
| django#17554 (fetch modes) | B | shortlisted | Every iteration path passes `fetch_mode=` to documented `Model.from_db()` override hook; #37259 reasoned dispute (Jacob Walls vs Adam Johnson) ending "treat this as a breaking change", release blocker. Large PR. |
| kubernetes#119779 (run all PreFilter) | C | shortlisted | Per-pod loop over all nodes allocating Status + map insert; #124709 pprof (alculquicondor), scheduler_perf 120.8 -> 552.6 pods/s at 15k nodes after fix. Base 3be9a8cc, head 09abd6be. |
| kubernetes#115082 | C | backup | Same subsystem as #119779; per-evaluation WithLabelValues; weaker growth argument. |
| kubernetes#112450 (client-go Holders) | A/C | backup | liggitt: "broke some assumptions of the aggregation layer"; transport cache grows per sync (#117250). Leak with time, contract between client-go cache and callers. |
| astro#16297 regression (getCollection image traversal) | C | pending/rejected for now | Strong measurements (100s->32s; 17->46 min) but introducing PR between 5.18 and 6.x not identified. |
| rclone#8570 goroutine leak | C | rejected | Reliability leak; introducing change unclear; CPU issue (#8569) unresolved attribution. |
| ripgrep#2750 | C | rejected | Pathological 9k-pattern ignore file; no introducing PR. |
| grpc-go#7184 (ALPN enforcement) | E | rejected | Not clean: GetConfigForClient configs lacked NextProtos under enforcement (#7709, purnesh42H "We need to do the same modifications"). |
| django#13829 (Origin CSRF check) | E | rejected | Not clean: #32578 DisallowedHost unhandled in `_origin_verified()` fixed a week later. Excellent review record otherwise. |
| requests#6965 (.netrc hostname) | E | rejected | Clean-looking but approval-only public review; reasoning private in GHSA. |
| grpc-go#8985 (strict :path) | E | shortlisted | Security fix; post-merge double-slash bypass claim refuted with test (#9120, easwars "We are already handling this correctly"); no later path-check fix. Approval-only pre-merge. |
| hono#4353 (Fetch Metadata CSRF) | E | shortlisted | usualoma redesign to opt-in; OPTIONS deliberately kept unsafe (later #5250 is the deferred enhancement); Copilot "missing Origin" objection refuted at head. 13-month window. |
| astro#17250 (origin check at dispatch) | E | shortlisted | Security fix; ematipico custom-fetch objection answered by matthewp (dispatch coverage, defence in depth). Short window (3 months). |
| trpc#5839 (connectionParams) | (security positive) | noted | GHSA-pj3v attributes WS DoS to #5839; not a clean control. |
| django#17196 (Message importable from contrib.messages) | A | alternate | LEVEL_TAGS evaluated at package import; MESSAGE_TAGS silently ignored (#34923 release blocker, felixxm "Regression in b7fe36ad"). Kept as A alternate to limit Django concentration. |
| kubernetes#112450 | A | shortlisted (A3, medium) | Removed documented "non-cacheable unless DialHolder" guard; liggitt attribution on #117250; Rancher #125818. Rancher attribution contested by aojea. |
| requests#6655 (TLS settings in pool selection) | B | shortlisted | send() bypasses public get_connection(); docker-py/requests-unixsocket break; nateprewitt "missed the other side ... I don't think what docker-py was doing is unreasonable". |
| graphql-js#3457 (simplify argument comparison) | C | shortlisted | Per-pair print() in O(n^2) conflict loop; #3955 2.5s@1000 -> 21s@3000; fix #3958 (IvanGoncharov) 12s -> 564ms. Self-merged, no review. |
| django#5686 / 917cc288 (list_editable resilience) | C | shortlisted | Formset queryset changed from page to get_queryset(); #28462 memory/time grows with table size; Tim Graham "reverting reintroduces possible data loss". |
| astro#16297 | C | rejected | Traversal per getCollection exists at astro@5.18.0; no introducing PR. |
| astro#17250 | E -> D-arch | moved | Strongest evidence is ematipico's architecture objection answered by matthewp; primary role D-arch. |
| kubernetes#141463 (copy const) | D-maint | shortlisted | Drift objection refuted by liggitt invariant "we would only ever expand the API constant". |
| grpc-go#9077 (defer Unlock) | D-scal | shortlisted | arjan-bal stack-growth objection; easwars benchmark +2.40% QPS, -2.63% p50, +3.07% p99, allocs equal. Post-merge debate. |
| kubernetes#121460 | D-scal | alternate | O(N^2) over CRD versions accepted because N small; no documented bound. |
| kubernetes#124917 (anonymous auth endpoints) | E | shortlisted | liggitt/enj sig-auth review; anonymous.go unchanged since; #130318 regression bisected to later da8dc433, not #124917. |
| grpc-go#7536 | D-arch | rejected | Issue, not PR; dfawley "we use what we need to use from xDS". |
| kubernetes#137782 | A | rejected | Reporter retracted; root cause protobuf/Go 1.26. |
| trpc#6823 | A | rejected | KATT: server import intentional; fix was build target (#6826). |
| bokeh GHSA-793v fix | E | rejected | Private fork merge, same-day follow-up fix. |
| trpc#7043 (prototype pollution fix) | E | rejected | "Fix bug", empty body; no public reasoning. |
| django#37259 via #17554 | B | shortlisted (B3) | Large PR; reasoned Fellow dispute. |
