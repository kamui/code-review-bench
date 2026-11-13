No actionable findings.

Reviewed the committed changes to provider selection, startup configuration and validation, scheduling, notifications, tests, and the Dockerfile. The provider-backed integration tests require credentials, and Docker is unavailable in this environment.

The offline `go test ./internal/ -count=1` run did not finish and was interrupted after more than a minute without output. The first attempt failed before running tests because the Go shim could not install its configured toolchain; the second used an installed Go binary.
