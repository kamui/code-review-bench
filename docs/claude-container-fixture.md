# Linux Claude container fixture

This is the dummy-only feasibility gate for [#36](https://github.com/kamui/code-review-bench/issues/36), part of [#35](https://github.com/kamui/code-review-bench/issues/35). It exercises the native Claude client and its default tool catalog. It does not dispatch a benchmark, use a subscription or admit a production review arm.

Run it on Linux as an unprivileged user with rootless Podman, Git, Python 3, Bash, ripgrep, bubblewrap, socat and a native Claude binary. The fixture builds a throwaway local image from those binaries and their libraries. It copies the system Python standard library, not site packages or a host toolchain directory. It records every copied executable/library hash, the rootfs archive hash, local image identity, kernel, OCI runtime, seccomp profile, settings and session IDs. It downloads nothing and refuses a Claude version or binary hash mismatch.

```sh
python3 bench/tools/claude_container_probe.py run \
  --output /tmp/claude-container-fixture-new \
  --claude /absolute/path/to/claude \
  --claude-version 2.1.289 \
  --claude-sha256 a186b99e4a9c88366cd49df2f7dad56c61fc306ef0140b19ee64b7c42a8d1348
```

Use a new output directory for every attempt. Existing evidence and homes are refused. Exit 0 means the complete fixture passed. Exit 1 retains a failed gate and its observations in `receipt.json`; exit 2 refuses unreadable or invalid input. A failed gate blocks downstream admission.

## Boundary and probes

The controller runs a fake provider on a Unix socket outside the container. A localhost relay inside the container forwards only to that socket. The provider has no upstream destination and returns predetermined native tool calls. The client receives a dummy API key in a fresh home. Its subprocess environment contains no inherited provider credentials. Ordinary repository commands must fail to reach both the relay and the socket with the inner sandbox enabled.

Podman runs the client, relay, hooks, dummy peer and native Agent worker behind private user, PID, network, IPC, UTS and mount namespaces, a read-only root, dropped capabilities and `no-new-privileges`. The network mode is `none`. Bind mounts contain only the attempt, read-only seed, dummy protected directory and fixture broker socket. No engine socket, host home or complete benchmark checkout is mounted.

Claude's native Bash sandbox stays enabled with `failIfUnavailable`, no unsandboxed retries and an empty network allowlist. File tools have a canonical-path PreToolUse hook, including symlink and recursive-search checks. The fixture deliberately installs repository settings that try to disable or widen these restrictions. CLI settings remain authoritative. This separation follows the client's documented [Bash and file-tool boundaries](https://code.claude.com/docs/en/sandboxing).

Each complete invocation starts fresh host and container sessions with the same client, model, effort, native catalog and settings constructor. Host paths translate to `/attempt`, `/protected` and `/broker` in the container. Each session has its own UUID and home receipt. The negative controls change only `sandbox.enabled`; all file-hook restrictions and container flags remain.

| Probe | Required observation |
| --- | --- |
| Native Read, Write and Edit | Inspect the seeded file and leave the edited scratch file on disk. |
| Bash, private localhost listener and Git worktree | Execute commands, bind/connect locally, and modify a separate faithful worktree. |
| Native protected reads/writes, symlink reads, Grep and Glob | Explicit native file-tool denial. |
| Protected and symlink shell reads | Denied with the inner sandbox; read the present dummy secret with it disabled. |
| Broker TCP and Unix socket | Denied to ordinary commands; reach the same fake broker with the inner sandbox disabled. |
| Process/environment | Scrub dummy provider auth and hide the dummy peer; expose that peer in the negative control. A private PID namespace can reuse the peer's number without exposing the peer. |
| Agent child shell | Execute the child tool, save its namespaces and status, and retain zero effective capabilities and `NoNewPrivs=1`. The outer process must have a zero capability bounding set. |
| Root and seed writes | Fail even with the inner sandbox disabled. The seed's before/after content hash must match. |
| Missing bubblewrap | Mask the actual dependency with a nonexecutable file; the client must exit unsuccessfully before any fake-provider request. |

A final answer, a client exit code of zero or an unexecuted shell request cannot pass. The gate checks provider-captured tool results and the actual scratch files. Container inspection, raw requests, stdout/stderr, settings and namespace/status observations remain in the attempt directory before disposable containers and images are removed.

## Verified profile

The [saved manifest](../bench/fixtures/2026-10-04-linux-claude-container/manifest.json) preserves ten attempts and their raw artifacts. [Attempt 010](../bench/fixtures/2026-10-04-linux-claude-container/attempt-010-receipt.json) passes all five profiles. Attempts 009 and 010 pass; earlier failures remain separate, including interpreter staging, permission gating, cgroup/runtime failures, incorrect probe assumptions and an invalid controller cleanup attempt.

The passing host is WSL2 Linux `6.6.87.2-microsoft-standard-WSL2`, Podman `3.4.4`, Claude `2.1.289` and crun `1.30.1`. The explicit OCI binary came from the [official crun release](https://github.com/containers/crun/releases/tag/1.30.1), with SHA-256 `86d1e6a0e76945975d3aebfab39cbc6a26eea15f1c3fc66b6776d19e5dc346a0`. Its path, hash and version are in the receipt. The system crun `0.17` could not create this hardened container; the recorded attempts retain that refusal.

This host lacks delegated cgroups. Its passing invocation explicitly used:

```sh
  --cgroups-disabled --oci-runtime /tmp/issue-36-crun-1.30.1/crun
```

That compatibility mode leaves namespace, capability, seccomp, read-only-root and privilege controls intact. It records that resource admission is not established. The default fixture requests CPU, memory and PID limits and refuses when the host cannot provide them. There is no automatic fallback or privileged or blanket-unconfined mode. Resource admission and laptop concurrency remain [#44](https://github.com/kamui/code-review-bench/issues/44); this profile is not evidence for that gate.

The seccomp profile is copied from Podman's installed default and hashed before execution. The passing profile requires no custom seccomp allowance for the native inner sandbox. Linux still shares the host kernel, so this result does not establish protection against kernel compromise or support for other clients, methods or platforms.

## Inspect evidence and regression tests

Each `attempt-NNN.tar.xz` contains the original per-session metadata and logs. The provider request JSON uses nested xz compression; native stream logs and engine inspections use gzip. The manifest records archive hashes and each artifact's original and stored hashes. No native binary or rootfs archive is committed. Full rebuildable workspaces and homes remain at the original paths recorded in the manifest.

```sh
tar -xJf bench/fixtures/2026-10-04-linux-claude-container/attempt-010.tar.xz -C /tmp
python3 -c 'import json,lzma; print(json.dumps(json.loads(lzma.open("/tmp/attempt-010/provider-requests.json.xz").read()), indent=2))'
python3 -m unittest discover -s bench/tools -p test_claude_container_probe.py -v
```

The eleven regression tests exercise canonical file boundaries, preserved evidence, binary-pin refusal and admission failures for absent tools, nonexecuted shells, leaked dummy data, missing native catalogs and ineffective negative controls. They make no provider calls and do not require Podman. `bun run test:bench` includes them through the existing test discovery.
