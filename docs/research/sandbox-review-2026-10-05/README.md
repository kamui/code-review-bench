# Review of the local sandbox proposal

Reviewed [#35](https://github.com/kamui/code-review-bench/issues/35), its discussion, and all eleven native sub-issues, #36 through #46. Repository baseline: `d29a5b874ddd6ed8f656263b19f036bd469c0e7b`. Research date: 2026-10-05, America/New_York.

Keep rootless Podman as the default implementation target. The epic describes a reasonable boundary, but the checked-out repository does not implement or prove it yet. Before implementation, extend containment to provisioning, settle the client-versus-command broker boundary, correct the layered negative controls, and add the user's explicit unsandboxed mode. No alternative reviewed currently establishes an easier, equivalent replacement for this repo's complete requirements. See the [alternative assessment](alternatives.md).

This review describes the code and tickets before the accepted amendments. The follow-up [sandbox execution plan](../../sandbox-execution.md) records those amendments, including the new [host-backend task #61](https://github.com/kamui/code-review-bench/issues/61). Runtime behavior, frozen evidence and admission decisions remain unchanged. No provider calls, image builds, container launches, or performance comparisons were run.

## What exists today

| Area | Evidence at the reviewed revision | Assessment |
| --- | --- | --- |
| Entire reviewer process | [`sandbox_command`](../../../bench/tools/codex_skill_runner.py) offers optional `bwrap-v1`, starting with a read-only bind of `/`, hiding selected directories, and binding the attempt read-write. It explicitly shares the network. | Useful isolation for existing policies, but broader than the proposed minimal container filesystem and offline command policy. |
| Credentials | [`claude_skill_runner.py`](../../../bench/tools/claude_skill_runner.py) copies `.credentials.json`; [`codex_skill_runner.py`](../../../bench/tools/codex_skill_runner.py) copies `auth.json`; [`dispatch.sh`](../../../bench/tools/dispatch.sh) does both for its arms. | Fresh homes and later deletion do not establish broker-only credentials during execution. |
| Native tools | [`review_isolation.py`](../../../bench/tools/review_isolation.py) configures Claude's Bash sandbox and file-tool hooks. The Codex skill runner requests workspace-write with networking enabled. | Different arms have different controls. A sandbox setting is not proof of whole-client confinement. |
| Clean context | [`clean_context.py`](../../../bench/tools/clean_context.py) refuses reused homes and configures ambient-context exclusions. | Preserve these checks in both execution modes. Context hygiene and OS confinement are separate guarantees. |
| Podman integration | No Podman launch implementation appears in the current runner paths; #41 proposes `podman_runtime.py`. All eleven child issues are open. | Implementation and compatibility remain unproved. |
| Provisioning | [`run_cell.py`](../../../bench/tools/run_cell.py) calls `provision.py prepare` before reviewer dispatch. [`provision.py`](../../../bench/tools/provision.py) executes build/post-clone/smoke commands through host `sh -c`; `command_env` copies `os.environ`. | The requested workflow boundary must begin before repository-dependent setup executes. |

The selected baseline tests passed with 16 native integration cases skipped. This establishes test health for those paths, not security certification. See [verification](verification.json).

## Findings to resolve before admission

### 1. High priority: put provisioning inside the boundary

Wrapping only the client leaves an earlier execution path on the host. `run_cell.py:463` invokes preparation before selecting a reviewer runner. `provision.py:282` executes shell commands; `command_env` at line 293 inherits the host environment; line 573 runs post-clone commands. Existing targets such as `j-trpc-5017` and `r-base-ui-5460` perform offline package installation there. Offline dependency setup can still execute code.

A synthetic probe called the existing environment and shell helpers with dummy data only. It successfully inherited a fake credential and modified a temporary file outside its clone. No actual target or provider was involved. This confirms the absence of containment in that helper path; it does not demonstrate a real credential leak.

Add `provision.py` explicitly to #41/#42. Keep trusted hashing, validation, and bounded data transfer in the controller. Execute repository-dependent preparation and smoke commands inside the selected backend with a minimal environment. Build dependency seeds in a separate confined preparation job with declared package access and no provider credentials. Verify immutable seeds before use; hashing does not make their executable contents trustworthy. Include setup processes in limits, cancellation, and evidence capture.

Grading has its own runtime and remains outside this rollout. Review preparation used by other workflows still needs an explicit scope statement; the epic cannot claim the entire repo is sandboxed.

### 2. High priority: prove broker separation before building the launcher

#36/#38/#40 require native clients and selected peers to use a provider broker while ordinary repository commands cannot reach or reuse it. One container, a shared UID, a placeholder token, or an allowed provider hostname does not make that distinction. A general command that can read the placeholder and reach the endpoint can issue requests with the same authority. A trusted binary path alone also fails if its launch accepts arbitrary configuration, executable search paths, or repository-controlled imports.

The existing feasibility gates correctly ask for real native-tool execution. Make their required mechanism explicit: a kernel-enforced difference in the filesystem, network, process, or descriptor access of the trusted client and its untrusted command descendants. Identify exactly how an approved CE peer enters the permitted domain without giving arbitrary Bash the same access. Do not prescribe a transport until this is demonstrated with the pinned clients.

Add dummy attacks for direct TCP and Unix-socket access, inherited descriptors, `/proc` access, sibling processes, copied placeholders, alternate client binaries, mutable peer scripts, and loader/environment injection. The approved client must succeed while those commands fail. Also test broker redirects, unsupported endpoints, cross-attempt grants, concurrent budget reservations, and expiry after controller loss.

A broker can protect real secrets and restrict operations. It cannot stop an authorized model request from carrying allowed repository content. State that residual exposure in the threat model. Native subscription refresh and billing remain unproved until the separately scoped live smoke test in #40 succeeds.

### 3. High priority: fix the negative-control contract

#36 currently requires a denied operation to succeed when the inner sandbox is disabled. That is valid only for a resource denied solely by that inner layer. If the outer container or proposed whole-client Landlock layer also denies it, continued denial is the correct result. The comment on #35 recognizes this conflict, but the child acceptance criterion has not incorporated it.

Use distinct synthetic controls:

| Control | Expected result |
| --- | --- |
| All layers enabled | Allowed tool work succeeds; forbidden work fails. |
| Inner-only restriction removed, outer policy unchanged | The inner-only canary becomes accessible; outer-protected canaries remain inaccessible. |
| Outer-only denial removed in a throwaway dummy fixture | The outer canary becomes accessible with other blocking layers accounted for. |
| Selected confinement mechanism unavailable | Admission refuses before auth or inference. |

Kernel Landlock restrictions are additive and inherited. Rights also depend on ABI support, and pre-existing descriptors need separate treatment. Whole-client Landlock is an optional additional layer, not a shortcut around the broker distinction. Pin required rights and fail if unavailable. [Kernel contract](https://docs.kernel.org/userspace-api/landlock.html).

### 4. Medium priority: make performance and recovery part of the first implementation

#43 and #44 already cover the important failure cases. Add a thin cancellation/export path and resource caps to the first runnable prototype. Do not wait until all adapters work to discover that stopping a controller leaves active requests or unrecoverable output.

The current `provision.prepare` holds one cache-root `prepare.lock` across clone creation, cache restoration, and post-clone execution. That is a concrete source of serialized preparation. Preserve its admission safety initially and measure queue wait separately. If it dominates, reserve disk and allocate private destinations under a short lock, then perform independent work outside it, with crash-safe reservations. Do not simply remove the lock.

Recovery should stop writers before final export, retain failed evidence, and verify the controller's copied bytes. Podman's copy documentation notes special symlink behavior and ignored permission errors for running rootless containers; successful `podman cp` alone is not a complete export receipt. [Copy semantics](https://docs.podman.io/en/latest/markdown/podman-cp.1.html).

### 5. Required scope change: support explicit unsandboxed execution

The user's request supersedes #35's Podman-only scope. Retain its prohibition on automatic fallback, and add an intentional `none` mode. Do not treat a failed Podman preflight as permission to run on the host.

## Proposed execution contract

Use one shared lifecycle with two small backend implementations. Existing adapters continue to own prompts, client/model/effort choices, normalization, and method rules. The shared lifecycle owns preparation, launch, limits, cancellation, collection, and receipts. Avoid a general plugin framework or additional backends in the first implementation.

The proposed user option is `--sandbox=podman|none`, defaulting to `podman`. This flag does not exist yet.

| Behavior | `podman` | `none` |
| --- | --- | --- |
| Prerequisites | Verified rootless engine and supported native-client profile. | Native clients and task tools; no Podman installation or running Machine required. |
| OS confinement supplied by the launcher | Outer container plus admitted native restrictions. | None. Explicitly disable launcher-controlled outer and native OS sandboxing through tested adapter configuration. |
| Missing sandbox capability | Refuse and explain the failed capability. | Run only because the user explicitly selected this mode. |
| Approvals | Preserve selected client permission policy. | Preserve approval policy independently; disabling isolation must not silently enable permission bypass. |
| Credentials and egress | Proven broker restrictions; ordinary commands offline. | Use explicit auth selection and disclose effective access. Same-user host processes cannot be promised secret or broker isolation. |
| Context and workspace | Fresh session/home, private work, verified inputs. | Retain the same bookkeeping and context defaults; they do not prevent host access. |
| Limits | Enforced controls plus admission and timeouts. | Record actual support; distinguish controller timeouts from unavailable kernel limits. |
| Evidence and comparisons | Record requested and observed policy. | Mark unsandboxed explicitly and keep it a separate comparison condition. |

If a frozen skill launches its own sandboxed peers, the launcher must not report complete `none` mode while those restrictions remain. Either support an explicitly versioned method/policy adaptation with honest lineage, or refuse that combination with the exact reason. Managed enterprise restrictions also remain authoritative. Do not silently edit frozen skills or use global client configuration as an override.

For local work, the explicit flag should be sufficient acknowledgment; display the effective mode before launch without repeated confirmation. For benchmark work, freeze the mode in the manifest. A CLI or environment mismatch must refuse rather than weaken the frozen policy. An intentional retry in another mode receives a new attempt and policy record. Preserve raw failed attempts and existing replacement rules.

Host execution cannot enforce blindness against malicious code reading the surrounding benchmark repository. Preserve the clean-context procedure, but record the absent OS boundary and do not advertise equivalent containment or mix results silently.

The smallest plan change is to amend #35 and #41/#42/#45/#46 and add one separately testable implementation task for the host backend. Its fixtures should prove operation with Podman absent, unchanged approval settings, observed absence of launcher/native sandboxing, honest handling of skill restrictions, and refusal to switch mode after a Podman error. No fallback is implemented by this report.

## Review of every child task

| Task | Assessment and concrete amendment |
| --- | --- |
| [#36: Linux Claude boundary](https://github.com/kamui/code-review-bench/issues/36) | Keep as first gate. Correct the negative controls; prove trusted client versus ordinary command broker access; include one repository-dependent setup command inside the boundary. |
| [#37: Codex confinement](https://github.com/kamui/code-review-bench/issues/37) | Keep. Test native file tools, actual built-in child threads, descendant processes and descriptors. Record the offline network policy separately from approval mode. |
| [#38: CE peers](https://github.com/kamui/code-review-bench/issues/38) | Keep as a blocker for CE. Prove an immutable, constrained peer launch, including loader/config injection and denied-grant branches. Shared UID or approval alone cannot establish admission. |
| [#39: doctor and macOS staging](https://github.com/kamui/code-review-bench/issues/39) | Keep. Split read-only diagnosis from explicit disposable conformance probes. Diagnose installed-version gaps and remote copy support; skip Podman checks entirely for explicit `none`. |
| [#40: credential broker](https://github.com/kamui/code-review-bench/issues/40) | Keep. Add atomic reservation of request/token allowances, protocol/redirect tests, and expiring grants independent of controller liveness. Do not claim exact dollar enforcement from request counts alone. |
| [#41: images and launcher](https://github.com/kamui/code-review-bench/issues/41) | Amend to a shared lifecycle with Podman and host implementations. Contain provisioning, use native architecture dependency seeds, enumerate writable mounts, and include basic recovery immediately. |
| [#42: runner integration](https://github.com/kamui/code-review-bench/issues/42) | Add `provision.py` and the host mode to schema, dispatch, observed receipts and filing checks. Reject mode mismatches. Keep grading behavior and frozen runners unchanged. |
| [#43: evidence and recovery](https://github.com/kamui/code-review-bench/issues/43) | Keep. Stop writers before final copying, verify required output completeness, test restart without network, and distinguish stopped processes from retained storage. Keep failed volumes/evidence accounted for. |
| [#44: limits and performance](https://github.com/kamui/code-review-bench/issues/44) | Keep concurrency 1/2/3. Measure preparation lock wait, transfers, dependency restoration, teardown and retained failed storage. Start measurements in the first prototype; keep this as the final validation task. |
| [#45: local launcher](https://github.com/kamui/code-review-bench/issues/45) | Add explicit `none`, concise effective-policy output, read-only doctor, inspect/status/cancel/recover operations, and actionable failures. Capture the index and working tree independently so staged/unstaged scopes survive; handle selected untracked files, worktrees and unsupported submodules explicitly. |
| [#46: readiness](https://github.com/kamui/code-review-bench/issues/46) | Report readiness by backend/client/method/platform. Keep all intended routes accounted for while distinguishing blockers from independently proved routes. An unsandboxed success is never a Podman conformance receipt. |

Keep the existing dependency gates for final admission. Interface design and small dummy probes can start earlier to reduce integration risk. For example, Linux broker fixtures need not wait for completed macOS setup, but cross-platform readiness still requires both. Do not make production claims from these partial results.

## Setup and performance recommendations

Use one existing suitable Podman Machine on macOS and guest-local storage for active attempts. Inspect its shares explicitly; documented Machine defaults can share the user's home. Keep residual VM exposure visible and avoid silently altering an existing Machine. [Machine volume documentation](https://docs.podman.io/en/latest/markdown/podman-machine-init.1.html).

The tested host has Podman **3.4.4** on WSL2, kernel **6.6.87.2**, x86-64. This is an inventory observation, not a supported-version decision. Choose the minimum supported version from actual conformance; do not copy current-doc flags into this installation without checking. For portable images on remote clients, test an explicit archive-load flow rather than assuming local `oci-archive:` execution transports work remotely. [Image loading](https://docs.podman.io/en/latest/markdown/podman-load.1.html).

Make all effective mounts and resources part of the receipt. Rootless operation does not imply zero container capabilities, and read-only root mode can still add writable tmpfs mounts. Enumerate and bound those mounts, use only required capabilities and a tested seccomp policy, and retain no-new-privileges. Include an engine-side deadline so a lost controller is not the only process responsible for termination. [Podman run controls](https://docs.podman.io/en/latest/markdown/podman-run.1.html).

Share verified immutable image layers and dependency seeds. Restore private writable caches for each attempt. Test copy-on-write or reflink reuse only where available; writable hardlinks into a shared seed are not equivalent. Use native ARM64/x86-64 images and match seeds to architecture, libc and toolchain. Separate optional toolchain layers where that reduces measured transfer/storage cost without changing task behavior.

Performance is **unmeasured** for the proposed implementation. Expected costs include cold image acquisition, macOS VM startup, dependency materialization, many small file operations, nested tool launches and evidence export. Their relative importance needs measurement.

Extend #44's fixture plan as follows:

1. Separate cold installation/image load, cold Machine start, warm attempt setup, tool execution, evidence verification and cleanup. Include lock/queue time in user-visible duration.
2. Run identical Python/Node/Go workloads and selected peers with dummy provider replies. Compare host and Podman modes as different security policies; claim equal-security comparisons only for alternatives that pass the same controls.
3. Alternate at least five runs per condition, at concurrency 1, 2 and 3, on each actual supported platform. Record hardware, versions, cache state, background load, median and range, completed work and failure count.
4. Capture peak memory, CPU throttling, pids, host/guest disk and inode growth, transfer bytes and retained failure storage. Include broker and VM costs in machine capacity calculations.
5. Verify output equivalence and actual commands. Report refused/OOM/timed-out attempts separately. A quick denial is not improved throughput.
6. Set defaults from observed peaks and explicit reserves. Keep disk admission atomic; label growth monitoring as best effort wherever a hard quota is unavailable.

For benchmark users, a normal launch should resolve and display policy, validate pins/capabilities, stage inputs, execute, collect evidence, file, and prune eligible rebuildable storage. Cancellation and recovery should use the same attempt identity and require no manual Podman commands. This is the practical usability bar for #35.
