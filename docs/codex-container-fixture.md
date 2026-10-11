# Linux Codex container fixture

This dummy-only feasibility gate covers [#37](https://github.com/kamui/code-review-bench/issues/37), part of [#35](https://github.com/kamui/code-review-bench/issues/35). Its entry prerequisite is the passing [Claude v4 receipt](../bench/fixtures/2026-10-06-linux-claude-container-v4/attempt-014-receipt.json). It runs real pinned Codex tools against a local fake Responses provider, with fresh homes and dummy authentication. It makes no live model calls, loads no historical reviews, and changes no benchmark runner or production admission policy. Explicit host execution remains [#61](https://github.com/kamui/code-review-bench/issues/61).

## Run

Use Linux x86_64, rootless Podman, Python 3, Git, Bash, ripgrep, bubblewrap, and the exact Codex release's `codex` and adjacent `codex-code-mode-host` binaries. The controller refuses binary mismatches and records every staged executable/library, the default seccomp profile, image identity, kernel, effective limits, configuration, provider requests, native outputs, and thread lineage.

```sh
python3 bench/tools/codex_container_probe.py run \
  --output /tmp/codex-container-new \
  --codex /absolute/path/to/codex-0.161.0/bin/codex \
  --oci-runtime /absolute/path/to/crun-1.30.1 \
  --include-weaker-controls
```

The pins are Codex `0.161.0`, SHA-256 `9a820c17865fa825d04db416818679a9d63bd72e50835c396f496e5684626c9c`; code-mode host `e87096087c02aa6ab90a5a85031659665aa0bf3ee20c07ca0b8ab701c83fcd5b`; and crun `1.30.1`, SHA-256 `86d1e6a0e76945975d3aebfab39cbc6a26eea15f1c3fc66b6776d19e5dc346a0`. The fake provider's frozen model identifier is `gpt-6.1-sol`; zero usage is recorded for every response.

Run from a delegated user session, following the [resource prerequisite](claude-container-fixture.md#resource-prerequisite-probe). Preparation and native execution both require observed `cpu.max=200000 100000`, `memory.max=1073741824`, and `pids.max=256`. The default outer boundary has private namespaces, network-none, a read-only root, zero capabilities, `NoNewPrivs=1`, and Podman's default seccomp. Engine execution expires after 150 seconds; client execution after 120. Temporary filesystems, shared memory, logs, and native file output have size bounds. Each owned container is inspected and verified absent before the next row; the imported image is removed and its absence checked at completion.

Use a new output directory each time. Existing evidence is never overwritten. Exit 0 requires the full matrix, including explicitly selected weaker controls. Omitting `--include-weaker-controls` runs only the protected rows and retains an incomplete, failed gate. The flag authorizes disposable attribution tests only: it neither creates a production outer-only tier nor selects a fallback after a failure.

## Independent native policy

The native approval setting is `never`. Filesystem confinement is a named profile extending `:workspace`, with read-only runtime support, four writable work roots, a writable temporary root, and explicit deny entries for `/broker` and the inner-only cache canary. The authoritative configuration is a read-only bind, even though fresh `.codex` session state is writable. Fresh homes exclude personal configuration; project guidance, skills discovery, apps, plugins, hooks, and memories are disabled. The only supplemental instructions are the common empty-harness policy and the selected synthetic skill. Repository configuration deliberately asks for weaker permissions; it must remain excluded.

Network is independent of approval and filesystem scope. The old runner's `workspace-write` with unrestricted network is not this fixture's policy. Plain native `network.enabled=false` also blocks the required localhost exercise. This pinned fixture instead sets native network enabled, explicitly enables the managed proxy in `limited` mode, permits new local bindings, and supplies no allowed outbound domains or Unix sockets. Saved turn contexts therefore report `network_access=true`; the limited proxy and outer network-none jointly implement the tested offline scope. Native tools can create and use their own localhost listener but cannot reach the existing broker listener or host endpoint. This is a versioned fixture policy change, not an assertion that `never` disables network.

Codex `0.161.0` exposes its native tools through the `exec` code-mode tool. The fake provider requests the actual catalog, `exec_command`, and `apply_patch`; it does not replace them with controller or MCP operations. Fresh same-client host baselines execute a harmless native command and capture the catalog separately for exec, built-in review, and a spawned worker. Built-in review naturally omits goal tools, so comparison uses the corresponding route. Host baselines provide no host-mode admission.

## Matrix and compatibility

| Row | Required observation |
| --- | --- |
| `native` | Actual inspection, four root writes, private cache patch, reproduction copy, localhost use, and alternate Python execution; protected reads, out-of-root writes, broker routes, configuration mutation, inherited descriptors, and host egress fail. |
| `review` | Real `codex review --uncommitted` creates its built-in child thread, executes the same tools, and preserves the effective policy and `never`. |
| `worker` | Native collaboration spawns a fresh worker on the pinned model, waits for completion, and executes the same exercise in that worker. Saved parent/thread IDs and forwarded subagent headers establish lineage. |
| `unavailable` | A deliberately unusable bubblewrap causes actual tool attempts to refuse. Neither the exercise nor native file patch executes, and saved policy remains confined. A successful CLI exit and final text alone cannot establish execution. |
| `inner-control` | Explicitly remove only native inner restrictions. The inner-only file, broker TCP/Unix and proc-root routes, and synthetic sibling environment become accessible. Other-UID data, immutable root/configuration, inherited descriptors, and outer egress remain denied. |
| `git-control` | Remove the immutable Git wrapper from the inner-off fixture. An actual repository fsmonitor callback and its alternate Python executable reach the brokers; wrapped callbacks execute and cannot reach them. |
| `file-control` | Substitute a readable protected directory in the inner-off fixture. Direct, symlink, and alternate-executable reads succeed, attributing the default denial to other-UID protection. |
| `root-control` | Make only the throwaway image root writable in the inner-off fixture. The world-writable root canary accepts a write. |
| `network-control` | Enable only private rootless slirp with host-loopback access in the inner-off fixture. It reaches a present dummy host endpoint that network-none cannot reach. |

The trusted client has a connected dummy broker descriptor. A positive control consumes its complete HTTP response. Native commands and Git callbacks probe that descriptor, parent proc paths, accessible descriptors, alternate executables, and copied dummy authorization independently. The native inner PID/mount/network namespaces differ from the trusted wrapper's namespaces; explicit inner-off rows retain the wrapper's namespaces. Both levels retain the outer capability and privilege restrictions.

Two narrow runtime accommodations are pinned in the receipt. Codex's sandbox helper is staged as the immutable `codex-linux-sandbox` alias because the client refuses automatic helper bootstrap inside a temporary home. The bounded `/tmp` tmpfs uses mode `1777`; Podman's default root-owned `0775` prevented the non-root native sandbox from creating its mount registry. Neither accommodation weakens resource caps, seccomp, privileges, protected data, or the read-only image.

The Claude fixture's nested Git bubblewrap cannot establish loopback inside Codex's managed-proxy seccomp: creating its `AF_NETLINK` socket is denied. This Codex fixture uses a separately versioned immutable [Git wrapper](../bench/tools/codex_fixture_git.py). It closes inherited descriptors, strips the environment, validates Git-location variables against the work roots, hides the broker/home, gives Git a private PID namespace and read-only root, and applies inherited offline seccomp instead of requesting another network namespace. The x86_64 filter denies socket creation and io_uring setup, and kills alternate architecture/x32 syscall paths. The outer default seccomp is unchanged. Actual fsmonitor and alternate-executable controls establish broker separation with the wrapper enabled and disabled. This accommodation applies only to this disposable fixture.

The kernel receipt records Landlock ABI availability and the actual namespace/seccomp state. This profile uses the pinned client's bubblewrap and seccomp route; ABI availability alone is not proof that Landlock was applied. It does not claim protection against kernel compromise, live subscription compatibility, other versions, other architectures, macOS, or downstream production admission.

## Evidence and checks

The [manifest](../bench/fixtures/2026-10-11-linux-codex-container/manifest.json) retains failed development attempts, their diagnostics and replacement lineage, and the source-matching passing matrix. [Attempt 020](../bench/fixtures/2026-10-11-linux-codex-container/attempt-020-receipt.json) passes all nine profiles with zero paid calls and verified container/image removal. Archives include the exact controller/wrapper, native/provider outputs, clean-context settings, session lineage, repository callbacks, resource observations, and engine inspections. They exclude native binaries and rebuildable rootfs images; the manifest pins their identities and retains original workspace paths. All 23 archives and their 7,907 selected members match original hashes and modes. No failed or prototype worker row is reused as passing evidence.

The [v1 admission revalidation](../bench/fixtures/2026-10-11-linux-codex-container/admission-revalidation-v1.json) records a stricter evaluator that requires the forbidden native patch to reject in protected and read-only controls and to succeed in `root-control`. It replays all nine profiles from the hash-verified Attempt 020 archive and rejects eight contradictory patch results, including the built-in review and worker results. This is new admission evidence over the retained native execution, with separate execution and evaluator hashes; it does not rerun the client or alter the original receipts. After restoring the archive, reproduce it with `python3 bench/fixtures/2026-10-11-linux-codex-container/source/revalidate-admission-v1.py`.

The [shared asset manifest](../bench/evidence/manifests/codex-container-2026-10-11-v1.json) pins the immutable release and asset identities. The [cold-restoration receipt](../bench/evidence/receipts/codex-container-2026-10-11-v1-restoration.json) records a remote download into an empty temporary directory and verifies archive bytes and permissions independently of the original workspaces. Restore the payloads before inspecting them:

```sh
python3 bench/tools/evidence_store.py fetch \
  --manifest bench/evidence/manifests/codex-container-2026-10-11-v1.json
python3 -m unittest discover -s bench/tools -p test_codex_container_probe.py -v
bun run verify:import
```

The focused tests refuse missing tools, successful final text without execution, weakened effective policy, changed approval/configuration, catalog omissions, actual broker leaks, missing inner namespaces, and successful or unexplained forbidden patch outcomes. The root control requires a successful native patch. Tests also cover encrypted native-worker routing and inherited offline seccomp in an alternate process. They use local dummy sockets only; environments that block Unix socket binding need an allowed local test session.
