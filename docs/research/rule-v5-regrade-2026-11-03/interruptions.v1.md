# Interrupted sessions

The Windows host reset three times while the `rebuilt` queue ran: at 2026-11-03T04:10Z, 04:24Z and 04:34Z. The Windows System log records each as Kernel-Power event 41 with bugcheck code 0, an unexpected reboot without a clean shutdown. Before the first two resets the WSL kernel log shows repeated segfaults of a `llama-server` from the Lemonade Docker containers, in `libggml-cpu-zen4.so`. The regrade does not use those containers. No segfault precedes the third reset.

Each reset cut off the same three Opus sessions, which left no dispatch receipt. The controller then refused to resume (`Reserved attempt has no receipt and is never dispatched again`).

| Batch | Attempt | Transcript span (UTC) | List-price estimate from the transcript |
| --- | --- | --- | --- |
| `2026-10-08-last-push-claude-opus/k-graphql-js-1582` | `attempt-1` | 04:04:12 to 04:08:29 | $1.14 |
| `2026-10-08-last-push-claude-opus/o-astro-16079` | `attempt-1` | 04:05:09 to 04:07:32 | $1.13 |
| `2026-10-08-last-push-claude-opus/p-hono-5067` | `attempt-1` | 04:07:23 to 04:08:01 | $0.72 |
| `2026-10-08-last-push-claude-opus/k-graphql-js-1582` | `attempt-2` | 04:20:39 to 04:22:30 | $0.95 |
| `2026-10-08-last-push-claude-opus/o-astro-16079` | `attempt-2` | 04:21:17 to 04:23:27 | $1.02 |
| `2026-10-08-last-push-claude-opus/p-hono-5067` | `attempt-2` | 04:21:33 to 04:21:58 | $0.66 |
| `2026-10-08-last-push-claude-opus/k-graphql-js-1582` | `attempt-3` | 04:30:04 to 04:31:46 | $1.04 |
| `2026-10-08-last-push-claude-opus/o-astro-16079` | `attempt-3` | 04:30:35 to 04:33:08 | $1.29 |
| `2026-10-08-last-push-claude-opus/p-hono-5067` | `attempt-3` | 04:30:49 to 04:33:24 | $1.40 |

The nine sessions cost about $9.35 together. The estimate prices each transcript's distinct assistant messages at Opus 5.5 list rates. Each attempt keeps its $3 reservation in the queue budget, as [Failures and restarts](../../grading-readiness.md#failures-and-restarts) requires, so $27 is held. Each attempt is kept unchanged. After each reset, the next attempt beside it retried the batch.

The Claude CLI on the host updated to 2.1.317 at 03:45Z. The queue dispatches from its own pinned copy of 2.1.316 (`client.json`), so the update did not reach the sessions.

The queue stopped after the third reset with 23 of its 173 batches mapped and $36.07 of priced usage. Its directory, `.local/regrade-v5/queues/rebuilt`, holds the nine unsettled attempts and stays on the original host. The `selected` and `plain` queues never started. The regrade continues on another host, which needs its own queues for the 235 batches left.
