package internal

import (
	"fmt"
	"os"
	"testing"

	"github.com/stretchr/testify/require"
)

// Same setup as TestFailureIfGitBackupDirUndefined; prints the error Run() returns.
func TestM2ProbeRunError(t *testing.T) {
	envBackup := backupEnvironmentVariables()
	defer restoreEnvironmentVariables(envBackup)

	preflight()
	resetGlobals()

	defer resetBackups()

	unsetEnvVarsExcept([]string{envPath})

	_ = os.Setenv(envGitHubToken, "ABCD1234")

	defer os.Unsetenv(envGitHubToken)

	err := Run()
	if err == nil {
		t.Logf("M2-PROBE Run() returned nil")
		return
	}

	t.Logf("M2-PROBE Run() error: %q", err.Error())
}

// Same setup, with the assertion the comment proposes for line 819.
func TestM2ProbeProposedAssertion(t *testing.T) {
	envBackup := backupEnvironmentVariables()
	defer restoreEnvironmentVariables(envBackup)

	preflight()
	resetGlobals()

	defer resetBackups()

	unsetEnvVarsExcept([]string{envPath})

	_ = os.Setenv(envGitHubToken, "ABCD1234")

	defer os.Unsetenv(envGitHubToken)

	require.ErrorContains(t, Run(), fmt.Sprintf("environment variable %s must be set", envGitBackupDir))
}
