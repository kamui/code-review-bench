# Linux Claude container fixture

This is the dummy-only feasibility gate for [#36](https://github.com/kamui/code-review-bench/issues/36), part of [#35](https://github.com/kamui/code-review-bench/issues/35). It exercises the native Claude client and its default tool catalog. It does not dispatch a benchmark, use a subscription or admit a production review arm.

The separately versioned v4 fixture adds bounded execution from preparation onward, other-UID protected data, an immutable Git command boundary, and independent layer controls. The historical v1–v3 recipes and evidence remain unchanged. Fixture success supplies evidence for #36; downstream production admission and the broader resource/recovery work in #44 remain separate.

## V4 bounded fixture

```sh
python3 bench/tools/claude_container_probe_v4.py run \
  --output /tmp/claude-container-v4-new \
  --claude /absolute/path/to/claude-2.1.289 \
  --claude-version 2.1.289 \
  --claude-sha256 a186b99e4a9c88366cd49df2f7dad56c61fc306ef0140b19ee64b7c42a8d1348 \
  --oci-runtime /absolute/path/to/crun-1.30.1 \
  --oci-runtime-sha256 86d1e6a0e76945975d3aebfab39cbc6a26eea15f1c3fc66b6776d19e5dc346a0
```

Run from a session with delegated CPU, memory and process controllers. Both preparation and the native client check effective `cpu.max=200000 100000`, `memory.max=1073741824` and `pids.max=256`. The container has private namespaces, no network, a read-only root, zero bounding capabilities and `NoNewPrivs=1`. Engine execution has a deadline; temporary filesystems, shared memory, logs and native output have size limits. The controller stops and removes only its owned containers and retains output, inspection, hashes and cleanup results.

Synthetic Git seed initialization, cloning and repository-dependent preparation execute in bounded containers before the client starts. Seed initialization uses a fresh home and disables system Git configuration. The repository script receives only PATH, HOME and LANG (Python can add LC_CTYPE), probes a present host-file canary, and scans accessible process environments for a synthetic inherited credential. Neither ambient input is supplied to the container. Its source, observations and effective limits are retained.

The protected directory and its files belong to another mapped UID in the immutable image. The native client's UID cannot read or chmod them, including through symlinks, irrespective of scoped native approvals. Dropped capabilities and the read-only mount prevent taking ownership or changing the protection. All four work roots remain writable. The attempt metadata and authoritative settings are read-only; explicit writable runtime exceptions are the temporary directory, fresh client state, and fixed output files. Runtime tools/libraries, the synthetic seed, and nonsensitive client support paths remain readable as required. No host home, credential store, engine socket, or benchmark checkout is mounted.

Native Bash uses the pinned client's inner filesystem, process, credential and network sandbox. Native Git and worktree operations can execute without Bash, so an immutable `/usr/bin/git` wrapper starts the real Git inside an additional bubblewrap boundary with private PID/network namespaces, a hidden broker and home, closed inherited descriptors, zero capabilities, a read-only root, and the four writable work roots. It retains only Git repository-location variables after resolving them under those roots; provider credentials and executable-search overrides are absent. Git's recursive update-ref/reset calls need those variables to initialize faithful worktrees. When the Bash sandbox has already hidden the home, the wrapper leaves that absent path alone.

The default model-visible catalog is compared with a fresh host baseline for the same pinned client and translated settings. The baseline executes Read only and supplies no host-mode admission. Native Read, Write, Edit, Grep, Glob, NotebookEdit, Bash, EnterWorktree, ExitWorktree and Agent worktree isolation execute. The fixture validates worktree HEADs and actual child writes, rather than accepting tool acknowledgments. Workflow remains visible and is refused under `dontAsk`; bare deny rules were rejected because they remove these tools from the catalog. Native filesystem/search routes consume fixed arguments, and scheduled/worker prompts use the same tool permissions; the fixture introduces no repository command hook outside the tested Bash and Git boundaries.

| Profiles | Attribution |
| --- | --- |
| `native`, `inner-control` | Disable only the inner sandbox: the inner-only file, broker TCP/Unix and sibling environment become reachable, while other-UID protected data and immutable settings remain denied. |
| `git-control` | Remove only the Git wrapper from the inner-off fixture: an actual repository fsmonitor callback reaches both brokers with copied dummy placeholders. The wrapped callback executes and cannot reach either. |
| `native-policy-control` | Bare native file approvals read the present home/symlink/notebook canaries; guarded native calls refuse them. |
| `file-guard`, `file-control` | With native approvals widened and the inner sandbox off, other-UID reads fail. Substitute a readable protected directory and both direct/symlink reads succeed; its read-only mount still rejects Write/chmod. |
| `root-guard`, `root-control` | With the inner sandbox off, a world-writable image canary rejects writes on the read-only root and accepts them only in a throwaway writable-root control. |
| `network-guard`, `network-control` | With the inner sandbox off, network-none cannot reach a present dummy host endpoint. A private rootless slirp network with host-loopback access reaches that endpoint. This weaker profile is never an admission mode. |
| `unavailable`, `cancel`, `deadline` | Missing bubblewrap refuses before provider access. Controller cancellation and engine timeout stop an actually started native Bash call and preserve its output. |

The broker controls include direct TCP/Unix, copied dummy authentication, an inherited socket and `/proc` access to it, and a sibling process environment. The inherited descriptor probes show that ordinary commands receive closed descriptors, both with and without the inner sandbox. A positive descriptor control runs in the trusted wrapper; actual native provider requests succeed separately. Direct TCP probes print a connection-attempt marker before the guarded connection refusal and the control's successful broker request. Repository settings that try to widen the policy are excluded with `--setting-sources ""`. Native mutation is denied. Inner Bash can write a private shadow at the hidden policy pathname; that shadow does not change the client's actual settings or subsequent permissions. With the inner sandbox off, a shell write reaches the real read-only policy mount and fails. The client checks authoritative settings equality after execution.

Whole-client Landlock is a separate failed optional profile. Kernel ABI 3 accepts the ruleset, but its filesystem restrictions prevent the native inner bubblewrap from establishing UID maps/mounts. A separately weakened diagnostic gets past UID mapping and then fails the mount operation. These failures are compatibility evidence, not successful confinement, and do not alter the v4 inner policy.

The slirp control uses Podman's documented [`allow_host_loopback=true` option](https://docs.podman.io/en/v3.4.1/markdown/podman-run.1.html). An earlier host-network diagnostic exposed the host cgroup mount and failed resource verification; it is retained and excluded from the passing matrix.

The [v4 manifest](../bench/fixtures/2026-10-06-linux-claude-container-v4/manifest.json) retains development failures and the separately failed Landlock profile. [Attempt 014](../bench/fixtures/2026-10-06-linux-claude-container-v4/attempt-014-receipt.json) passes all fourteen profiles, with confined seed/preparation, unchanged seed content, actual kernel limits, and verified container/image removal. Its archived runtime matches the delivered source. Attempt 013 remains unchanged: its direct TCP probe failed while reading hidden metadata before opening a socket, so that result did not establish TCP isolation. The replacement reads accessible scratch metadata and checks the connection-attempt marker in both profiles. The cleanup repair requires an explicit container-exists exit of 1 before treating failed inspection as absence; three regression tests and an actual repeated cleanup check cover it. The [Git identity](../bench/fixtures/2026-10-06-linux-claude-container-v4/git-identity.json) identifies the real Git inside the immutable image separately from its wrapper. Archives retain exact native/provider requests, settings, worktree metadata, cancellation output and engine inspections; every stored artifact was verified against its original hash.

## Historical v1–v3 fixture

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

## Historical v3 admission requirement

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

The October 4 historical invocation lacked delegated cgroups and explicitly used:

```sh
  --cgroups-disabled --oci-runtime /tmp/issue-36-crun-1.30.1/crun
```

That compatibility mode leaves namespace, capability, seccomp, read-only-root and privilege controls intact. It records that resource admission is not established. The default fixture requests CPU, memory and PID limits and refuses when the host cannot provide them. There is no automatic fallback or privileged or blanket-unconfined mode. Resource admission and laptop concurrency remain [#44](https://github.com/kamui/code-review-bench/issues/44); this profile is not evidence for that gate.

The seccomp profile is copied from Podman's installed default and hashed before execution. The passing profile requires no custom seccomp allowance for the native inner sandbox. Linux still shares the host kernel, so this result does not establish protection against kernel compromise or support for other clients, methods or platforms.

## Resource prerequisite probe

`bench/tools/claude_container_resources.py` checks an existing immutable fixture image without starting Claude or a provider. Run it from the intended delegated user session. It requests the fixture's CPU, memory and process limits, reads `cpu.max`, `memory.max` and `pids.max` inside the container, and refuses absent, unlimited or mismatched values. Requested engine settings alone cannot pass. The receipt explicitly keeps `issue36_complete` false; resource success would still leave the native-client acceptance matrix and independent review pending.

```sh
python3 bench/tools/claude_container_resources.py \
  --output /tmp/claude-resource-probe-new \
  --image sha256:IMMUTABLE_FIXTURE_IMAGE_ID \
  --oci-runtime /tmp/issue-36-crun-1.30.1/crun \
  --oci-runtime-sha256 86d1e6a0e76945975d3aebfab39cbc6a26eea15f1c3fc66b6776d19e5dc346a0
```

The probe uses no bind mounts or network, retains commands, engine inspection and stdout/stderr, and removes only its own container after inspection. It preserves failed output directories. An engine deadline bounds execution to 30 seconds; the controller waits up to 45 seconds and stops an owned running container before removal. `/tmp`, `/run`, `/var/tmp`, shared memory and engine logs have explicit size limits. The v3 native fixture and its archived recipe hashes remain unchanged.

On the WSL host checked on October 6, a detached `systemd-run --user` service reached the delegated memory and process controllers. A separate partial probe observed `memory.max=1073741824` and `pids.max=256`, with `cpu.max` absent. Requesting `Delegate=cpu memory pids` and `CPUQuota=200%` on a user service did not supply the missing ancestor CPU controller. The full capped launch failed in crun before executing the image's command.

The system manager owns the ancestors above `user@1000.service`, as described in the installed [systemd 249 resource-control documentation](https://github.com/systemd/systemd/blob/v249/man/systemd.resource-control.xml). The stock unit has `Delegate=pids memory`. On this version, delegation setters apply only when creating transient units, so `set-property` cannot change delegation on this existing service. CPU accounting was already enabled and did not enable the CPU controller.

For this host, whose service CPU weight was unset and whose quota was unlimited, the targeted runtime activation to try in an authenticated host terminal is:

```sh
sudo systemctl set-property --runtime user@1000.service CPUWeight=100
```

This selects the documented default CPU weight and requests the CPU controller through a setter supported for running units. It changes no CPU quota or persistent configuration and needs no user-manager restart. Noninteractive authorization and passwordless sudo were refused; the user ran the command in an authenticated terminal. CPU then appeared in every system-owned ancestor. Enabling `+cpu` in the user-owned `user@1000.service` and `app.slice` subtrees allowed the bounded probe to run. It observed `cpu.max=200000 100000`, `memory.max=1073741824` and `pids.max=256` inside the container, then verified removal. These observations establish this host's prerequisite only. Inspect the actual ancestry and effective values on another host; do not use cgroups-disabled execution to satisfy this prerequisite.

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
