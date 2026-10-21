# Runtime evidence

[`rollback.sh`](rollback.sh) clones a commit into a new scratch directory, installs `bench-tools`,
commits a second version that changes only the help summary, and installs that too. It then runs
both retained executables, rolls back, and verifies both.

The `Rust runtime` workflow runs it on each supported host. [`rollback.log`](rollback.log) is one local
run on `x86_64-unknown-linux-gnu` under WSL2, from when the script installed `usage-pilot`; its header names the commit and host. It predates the
removal of the Python usage adapter, so it also shows a behavior corpus run with each executable.
Version one's SHA-256 matches builds from two other checkout paths of the same crate tree on the same host.
