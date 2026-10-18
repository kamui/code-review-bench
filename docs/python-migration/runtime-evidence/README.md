# Runtime evidence

[`rollback.sh`](rollback.sh) clones a commit into a new scratch directory, installs `usage-pilot`,
commits a second version that changes only the usage message, and installs that too. It then runs
both retained executables, runs the behavior corpus with each one, rolls back, and verifies both.

The `Rust runtime` workflow runs it on each supported host. [`rollback.log`](rollback.log) is one local
run on `x86_64-unknown-linux-gnu` under WSL2; its header names the commit and host. Version one's
SHA-256 matches builds from two other checkout paths of the same crate tree on the same host.
