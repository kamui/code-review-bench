# Measurement record

Expected claim before timing: an all-dead first listing issues GET plus two additional serial commands per member at head; later listings benefit from reclamation. Timing is a controlled illustration, not a production latency claim.

Five alternating base/head runs per shape. Connection primed by PING; each case seeded using InsertEntry with TtlSec=1 and waited 1.2 seconds. The timer surrounds the complete store ListDirectoryEntries call, including default logging. Every return was checked for error. Every first listing returned no files. Remaining members were checked outside the timer: N at base, zero at head. A live-entry control returned ok at both commits.

| Names | Artificial command wait | Base median (range), ms | Head median (range), ms |
| --- | --- | --- | --- |
| 64 | 0 us | 8.829 (8.239–10.256) | 21.661 (20.153–23.701) |
| 64 | 1000 us | 82.370 (80.452–83.878) | 241.478 (237.892–243.078) |
| 1024 | 1000 us | 1289.561 (1282.452–1309.529) | 3844.139 (3821.977–3849.465) |

Limiter check: the delay hook deliberately adds one awaited 1ms sleep after every synchronous Redis call, not on setup. Counts are 1+N at base and 1+3N at head. Thus the imposed minimum at N=1024 is 1.025s and 3.073s; observed times exceed, and do not beat, those bounds. The same workload at delay=0 takes about 9ms and 22ms for 64 names, so sleep dominates the delayed case. No production network RTT was measured and the operating system may round sleeps upward. No profile was used to claim a production bottleneck. The hook checks actual command completion and index state, so it did not time discarded or lazy work.

Host check: 24 logical CPUs; recorded load 2.51/1.15/1.00 before timing. The visible process list was idle in this process namespace; other activity could not be excluded. Alternating sides reduces drift. No production connection-pool, cluster, Sentinel, competing load, full HTTP deadline, or user frequency was measured. This is not a choice between production deployments or an end-to-end HTTP benchmark.

The two-command pipeline experiment is a single diagnostic run per patch, not a speedup benchmark. It demonstrates 64 pipeline executions instead of 128 independent cleanup waits. It does not establish safety on every transport.

Verdict: the first store listing is slower in these controlled workloads; production severity and intended tradeoff remain unsettled. Do not extrapolate a fixed threefold production slowdown or a client deadline failure.
