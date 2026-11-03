# Move to a second host

After the three resets in [interruptions.v1.md](interruptions.v1.md), the regrade moved from the Windows host `onigiri` (WSL2) to `siamese`, a Docker container on a QNAP NAS: an Intel Core i3-4150 with 2 cores and 4 threads, 16 GB of memory, Debian 13, Linux 5.10.60-qnap and Docker 27.1.2. The version 2 plans and authorizations grade the 235 batches left there. This file records what differs from version 1.

## Queues

The version 1 queue directories stay on `onigiri`. The `rebuilt` queue there holds the 23 mapped batches and the nine unsettled attempts. [`plan.v2.py`](plan.v2.py) planned the batches the methodology plan marks `missing`, leaving out the 23 it marks `current`: 150 `rebuilt`, 45 `selected` and 40 `plain`. Their queue directories, workspaces, cache root and archives end in `-2`.

## Cap

The approval of 2026-11-03 covers $320 across both hosts. The first host spent $36.07 at list price and holds $27 for the nine cut-off attempts until they are settled there, so [`authorize.v2.py`](authorize.v2.py) splits the remaining $256.93 among the three queues by the list price of each batch's latest Opus grade.

## Sandbox

Docker's default seccomp profile blocks user namespaces, so bubblewrap could not start and `bench-tools grading probe-claude` refused before any session. The container now runs with two changes to Docker's defaults:

- Docker 27.1.2's own default seccomp profile, with one added rule that allows `clone`, `unshare`, `setns`, `mount`, `umount2` and `pivot_root`. The other calls the default profile blocks stay blocked.
- `systempaths=unconfined`, which leaves `/proc` unmasked. Without it bubblewrap cannot mount the fresh `/proc` the grading sandbox asks for.

The container installs bubblewrap and socat from Debian when it starts. The grading sandbox is unchanged, and `grading probe-claude` passes with these settings.

## Runtime and client

`bench-tools` was rebuilt on the new host from revision `823b9ccc8`. Nothing under `rust/` or `bench/tools/` changed since the version 1 authorizations, but the binary differs because the build machine differs: it is `9423580c` (receipt `a4251d11`) where version 1 pinned `793d009b`.

Claude Code on the new host is 2.1.317. The version 1 queue dispatched from its copy of 2.1.316. Under the owner's standing rule to pin the local client when a queue starts grading, each version 2 queue pins the client installed at its first run, recorded in its `client.json`. The 23 batches graded on the first host used 2.1.316.

## Caches

The cache root `.local/regrade-v5-2/dependency-cache` holds:

- the 13 dependency archives from the published release `dependencies-2026-10-08-v1`, whose hashes match the archives the two cache replacement manifests name;
- the five toolchain archives from `toolchain-supplements-2026-10-25-v1`;
- the 15 target mirrors, rebuilt with `bench-tools provision mirror` from full clones of the upstream repositories. Each passed `bench-tools provision check`, which confirms the head and merge base, the absence of every negative SHA and the diff identity that `target.json` records.

Before the authorizations were written, all 235 batches passed `grade preflight-inputs` against this cache root, and then `grade preflight` with `claude-opus-5-5` and Claude Code 2.1.317.
