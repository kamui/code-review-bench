# Runtime evidence

[`rollback.sh`](rollback.sh) clones a commit into a new scratch directory, installs `usage-pilot`,
commits a second version that changes only the usage message, and installs that too. It then runs
both retained executables, runs the behavior corpus with each one, rolls back, and verifies both.

[`rollback.log`](rollback.log) is one run of it on `x86_64-unknown-linux-gnu` under WSL2. Its header names the commit
and host. Version one's SHA-256 matches builds from two other checkout paths of the same crate tree
on the same host. Nothing here exercises another target.
