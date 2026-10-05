# Linux Claude container fixture

This is the dummy-only feasibility gate for [#36](https://github.com/kamui/code-review-bench/issues/36), part of [#35](https://github.com/kamui/code-review-bench/issues/35). It exercises the native Claude client and its default tool catalog. It does not dispatch a benchmark, use a subscription or admit a production review arm.

This partial delivery supplies the fixture, preserves its observed results and fixes ambient settings hooks. The v3 follow-up moves native file approvals into directory-scoped rules and adds hook-free controls. Issue #36 remains open, and downstream admission stays blocked pending independent review of the revised policy and its native permission-check/open contract.

Run it on Linux as an unprivileged user with rootless Podman, Git, Python 3, Bash, ripgrep, bubblewrap, socat and a native Claude binary. The fixture builds a throwaway local image from those binaries and their libraries. It copies the system Python standard library, not site packages or a host toolchain directory. It records every copied executable/library hash, the rootfs archive hash, local image identity, kernel, OCI runtime, seccomp profile, settings and session IDs. It downloads nothing and refuses a Claude version or binary hash mismatch.

```sh
python3 bench/tools/claude_container_probe.py run \
  --output /tmp/claude-container-fixture-new \
  --claude /absolute/path/to/claude \
  --claude-version 2.1.289 \
  --claude-sha256 a186b99e4a9c88366cd49df2f7dad56c61fc306ef0140b19ee64b7c42a8d1348
```

Use a new output directory for every attempt. Existing evidence and homes are refused. Exit 0 means the recorded probe matrix passed; it does not resolve the deferred admission requirement. Exit 1 retains a failed gate and its observations in `receipt.json`; exit 2 refuses unreadable or invalid input. A failed gate blocks downstream admission.

## Boundary and probes

The controller runs a fake provider on a Unix socket outside the container. A localhost relay inside the container forwards only to that socket. The provider has no upstream destination and returns predetermined native tool calls. The client receives a dummy API key in a fresh home. Its subprocess environment contains no inherited provider credentials. Ordinary repository commands must fail to reach both the relay and the socket with the inner sandbox enabled.

Podman runs the client, relay, hooks, dummy peer and native Agent worker behind private user, PID, network, IPC, UTS and mount namespaces, a read-only root, dropped capabilities and `no-new-privileges`. The network mode is `none`. Bind mounts contain only the attempt, read-only seed, dummy protected directory and fixture broker socket. No engine socket, host home or complete benchmark checkout is mounted.

Claude's native Bash sandbox stays enabled with `failIfUnavailable`, no unsandboxed retries and an empty network allowlist. Native Read and Edit approvals use absolute patterns over `work`, `reproduction`, `scratch` and `cache`, such as `Read(//attempt/work/**)`. Edit patterns cover Write and NotebookEdit. Grep and Glob are explicitly exposed through scoped `--allowedTools` entries; bare Read, Write, Edit, Grep and Glob approvals are absent from guarded profiles. The four working directories are supplied by trusted settings and explicit `--add-dir` flags. Runtime is available to Bash and the trusted hook but is not a native working directory. Native Read and Edit denials cover the disposable home and temp directories, including internal command files.

The canonical-path PreToolUse hook remains an additional check, including symlink and recursive-search checks. The fixture deliberately installs repository settings that try to disable or widen these restrictions and a harmless SessionStart callback that writes a scratch marker. `--setting-sources ""` excludes user, project and local settings while retaining the explicitly supplied policy and its trusted file hook. A repository callback marker fails admission in every profile. This separation follows the client's documented [Bash and file-tool boundaries](https://code.claude.com/docs/en/sandboxing).

Each complete invocation starts fresh host and container sessions with the same client, model, effort, native catalog and settings constructor. Host paths translate to `/attempt`, `/protected` and `/broker` in the container. Each session has its own UUID and home receipt. The original five profiles retain their controls: the inner-sandbox negative profiles change only `sandbox.enabled`, keeping the native policy, file hook and container flags.

Four additional profiles isolate native file enforcement. Host and container `native-files` profiles remove only the file hook. Their `native-files-negative` partners also replace scoped approvals and home/temp denials with bare file-tool approvals. All four retain the inner Bash sandbox and container controls. Positive Write, Read, Edit, Grep, Glob and NotebookEdit calls execute under every declared root. Direct reads and mutations outside those roots, searches of the present protected file, runtime/internal/temp reads, and native Agent file calls are checked independently. Raw results distinguish permission denials from unread-file preconditions. Symlinked-parent Write and Edit remain denied even with the native policy removed on this client; those two results are recorded as policy-independent denials, while the paired symlink Read and direct mutation controls must succeed.

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
| Repository settings callback | Its harmless scratch marker must remain absent. |

A final answer, a client exit code of zero or an unexecuted shell request cannot pass. The gate checks provider-captured tool results and the actual scratch files. Container inspection, raw requests, stdout/stderr, settings and namespace/status observations remain in the attempt directory before disposable containers and images are removed.

## Remaining admission requirement

The [native permissions documentation](https://code.claude.com/docs/en/permissions#symlinks) describes a recheck against the permission-approved file location when Read, Write and Edit open it. V1 and v2 depended on an earlier root check in the hook, followed by bare native approvals. Their evidence did not establish that the native recheck preserved the hook's approved location after a concurrent pathname replacement.

V3 instead proposes composition at the native checkpoint: directory-scoped permissions check the requested and resolved target paths, and the documented final-open check preserves that native approval. Hook-free static controls establish which native routes deny the synthetic targets on the pinned client. They do not prove absence of every concurrent bypass. Scoped allow rules are not an exclusive allowlist; default approvals, internal exceptions, search tools, NotebookEdit and inherited Agent permissions remain part of the independent contract assessment. The documentation describes Read-rule coverage of other reading tools as best effort.

Completing #36 requires independent acceptance of that composition and the full issue matrix against an applicable pinned-client contract. An unresolved route keeps admission blocked; use a separately scoped OS-boundary design or vendor clarification if the contract cannot be established. Resource admission remains a separate requirement under #44.

### Read-block compatibility decision

The proposed `blockReadsOutsideWorkingDirectories:true` setting was tested and rejected for this fixture. [Attempt 013](../bench/fixtures/2026-10-04-linux-claude-container-v3/attempt-013-receipt.json) denied the intended shell calls when only `sandbox.enabled` was disabled, so it could not satisfy the original negative control. A separate diagnostic removed the runtime deny rules and observed the same Bash denial. The [setting's documentation](https://code.claude.com/docs/en/settings-reference#permissions-blockreadsoutsideworkingdirectories) explains its effects on unsandboxed shell approval and its internal-file exceptions.

Removing that setting while retaining bare search approvals also failed a static protected-file search diagnostic. The selected v3 policy therefore retains `dontAsk`, scoped native approvals and scoped search activation, without the read-block setting or broad search approval. This is an explicit tested policy choice. It does not alter the original inner-sandbox-off controls or add a fallback to a failed run.

## Verified profile

The [saved manifest](../bench/fixtures/2026-10-04-linux-claude-container/manifest.json) preserves ten attempts and their raw artifacts. [Attempt 010](../bench/fixtures/2026-10-04-linux-claude-container/attempt-010-receipt.json) passes all five profiles. Attempts 009 and 010 pass; earlier failures remain separate, including interpreter staging, permission gating, cgroup/runtime failures, incorrect probe assumptions and an invalid controller cleanup attempt.

Those results belong to `claude-container-fixture-v1`. Subsequent local review confirmed that its `--setting-sources project` invocation loaded repository SessionStart hooks outside the Bash sandbox. The archives remain immutable; their passing gates do not establish authoritative hook isolation.

`claude-container-fixture-v2` excludes ambient settings sources and adds the harmless callback admission check. A pinned-client regression reproduced the old behavior, then passed with the fix while an explicitly configured hook and native Read still executed. The separate [v2 manifest](../bench/fixtures/2026-10-04-linux-claude-container-v2/manifest.json) records [attempt 011](../bench/fixtures/2026-10-04-linux-claude-container-v2/attempt-011-receipt.json): all five profiles passed with the same native catalog, absent repository callback markers and unchanged seed hash. All three disposable containers were verified absent, and image cleanup exited 0. Its 84 archived artifacts include repository settings, the exact controller source, native results, engine inspection and cleanup evidence. Native file-open behavior after a concurrent pathname replacement remains unresolved, so downstream admission is still blocked.

The passing host is WSL2 Linux `6.6.87.2-microsoft-standard-WSL2`, Podman `3.4.4`, Claude `2.1.289` and crun `1.30.1`. The explicit OCI binary came from the [official crun release](https://github.com/containers/crun/releases/tag/1.30.1), with SHA-256 `86d1e6a0e76945975d3aebfab39cbc6a26eea15f1c3fc66b6776d19e5dc346a0`. Its path, hash and version are in the receipt. The system crun `0.17` could not create this hardened container; the recorded attempts retain that refusal.

The separate [v3 manifest](../bench/fixtures/2026-10-04-linux-claude-container-v3/manifest.json) preserves attempts 012 through 015 and development diagnostics, with 617 hash-verified archived artifacts. Attempts 012 and 013 failed before completion. Attempt 014 passed the scoped-policy matrix. [Attempt 015](../bench/fixtures/2026-10-04-linux-claude-container-v3/attempt-015-receipt.json) passed all nine profiles with the final controller, including validation of the child catalog and explicit attribution of the policy-independent symlink mutation denials. Its recipe hash matches the current controller. Both passing attempts retained an unchanged seed; their five containers and imported image were independently verified absent after cleanup. All traffic stayed within the fake provider, with zero paid calls. V1 and v2 evidence is unchanged.

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

The twelve default regression tests exercise canonical file boundaries, preserved evidence, binary-pin refusal and admission failures for absent tools, nonexecuted shells, leaked dummy data, repository callback execution, missing native catalogs and ineffective negative controls. They make no provider calls and do not require Podman. `bun run test:bench` includes them through the existing test discovery.

The two optional tests run the pinned native client against a local fake provider with dummy authentication. One uses harmless callbacks in user, project and local settings, requires all their markers to remain absent, and verifies that the explicitly supplied callback and a native Read execute. The other runs hook-free native file checks and the policy-removed control, including all four roots, notebook edits and Agent permissions. Both make no paid calls and do not require Podman. The [saved final checks](../bench/fixtures/2026-10-04-linux-claude-container-v3/checks.json) record all fourteen focused tests passing, the broad suite's 525 tests with eighteen skips and the provisioning self-test. They also retain the first broad run's environment failure, before rerunning with the namespaces it requires.

```sh
BENCH_CLAUDE_CONTAINER_CLI=/absolute/path/to/claude-2.1.289 \
  python3 -m unittest discover -s bench/tools -p test_claude_container_probe.py -v
```
