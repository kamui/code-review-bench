No actionable findings.

The refactored provider collection, startup logging, credential validation, and scheduling retain the prior behavior. The backup status title helper preserves the same success, partial failure, and failure cases. The Dockerfile changes preserve the installed packages and release download while consolidating the package installation layer and quoting the expanded tag.

Validation: `go test ./internal/ -count=1` passed using the offline execution settings specified for this review. Provider-backed tests skipped where credentials were unavailable; Docker build validation was unavailable under the execution policy.
