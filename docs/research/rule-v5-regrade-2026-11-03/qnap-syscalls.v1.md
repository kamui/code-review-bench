# A cache restore that failed on the QNAP kernel

The `rebuilt` pilot on the second host ([host-move.v1.md](host-move.v1.md)) mapped two batches and stopped at the third. Preparing `2026-10-08-last-push-claude-opus/p-hono-5067` (`attempt-1`) failed while restoring the frozen cache archive `0e0e2ff5`: `tar` reported `Cannot change mode to rwxr-xr-x: Bad address` for every symlink under `node_modules/.bin` and left mode-0000 placeholder files in their place. The failure came before any reservation or session, so it cost nothing. The attempt is kept unchanged.

## Cause

glibc 2.41 changes the mode of a symlink with `fchmodat2`, syscall 452, and falls back to the older method when the kernel answers `ENOSYS`. Linux 5.10 has no `fchmodat2`, but the QNAP kernel, 5.10.60-qnap, answers syscall 452 with `EFAULT`, even for a regular file, and syscall 451 the same way. The container's seccomp profile, Docker 27.1.2's default, allows both names, so the calls reached the kernel and glibc never fell back. The first host's kernel has `fchmodat2`, so it never saw this.

The same restore succeeded, with every symlink in place, when the command ran under a filter that answered syscall 452 with `ENOSYS`. tar, node, go, git, bubblewrap and `grading probe-claude` all ran unchanged under a filter that answered every syscall from 441 up with `ENOSYS`.

## Change

The container's seccomp profile now answers with `ENOSYS` every syscall that Linux 5.10 lacks, from `epoll_pwait2` (441) up, as a stock 5.10 kernel would. The change only removes calls. After the container was recreated with it, syscalls 451 and 452 return `ENOSYS`, the restore succeeds with plain `tar`, and `grading probe-claude` passes.

The batch was retried in `attempt-2`, beside the failed attempt, as [Failures and restarts](../../grading-readiness.md#failures-and-restarts) requires. The queues then ran at three workers.
