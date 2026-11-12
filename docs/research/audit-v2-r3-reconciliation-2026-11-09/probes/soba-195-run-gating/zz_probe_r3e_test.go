package internal

import (
	"fmt"
	"os"
	"strings"
	"testing"
)

// Copied into internal/ of each commit. soba's own code is not modified.

func probeClearEnv() {
	for _, kv := range os.Environ() {
		name := strings.SplitN(kv, "=", 2)[0]
		for _, prefix := range []string{"GIT_", "GITHUB_", "GITLAB_", "GITEA_", "BITBUCKET_", "AZURE_DEVOPS_", "SOURCEHUT_", "SOBA_"} {
			if strings.HasPrefix(name, prefix) {
				os.Unsetenv(name)
			}
		}
	}
}

func TestProbeR3E(t *testing.T) {
	dir := t.TempDir()

	// Case A: the comment's example. Azure DevOps user set, PAT missing, another provider (GitHub) configured.
	probeClearEnv()
	t.Setenv("GIT_BACKUP_DIR", dir)
	t.Setenv("AZURE_DEVOPS_USERNAME", "someone")
	t.Setenv("GITHUB_TOKEN", "x")
	fmt.Printf("PROBE A checkProvidersDefined() = %v\n", checkProvidersDefined())
	fmt.Printf("PROBE A Run() = %v\n", Run())
	fmt.Println("PROBE A end (any 'backing up Azure DevOps repos' line above would mean AzureDevOps ran)")

	// Case B: a claim item the comment names without a result. Gitea token set, API URL missing, no other provider.
	probeClearEnv()
	t.Setenv("GIT_BACKUP_DIR", dir)
	t.Setenv("GITEA_TOKEN", "x")
	fmt.Printf("PROBE B checkProvidersDefined() = %v\n", checkProvidersDefined())
	fmt.Printf("PROBE B runProviderBackups() failed = %d\n", runProviderBackups())
}
