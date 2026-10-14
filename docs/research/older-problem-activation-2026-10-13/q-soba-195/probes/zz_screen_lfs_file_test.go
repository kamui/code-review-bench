package internal

import (
	"bytes"
	"log"
	"os"
	"path/filepath"
	"testing"
)

func TestScreenLFSFileVariable(t *testing.T) {
	lfsVars := []string{
		envGitHubBackupLFS, envGitLabBackupLFS, envGiteaBackupLFS,
		envBitBucketBackupLFS, envAzureDevOpsBackupLFS, envSourcehutBackupLFS,
	}

	flagFile := filepath.Join(t.TempDir(), "lfs")
	if err := os.WriteFile(flagFile, []byte("true\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	for _, v := range lfsVars {
		t.Setenv(v, "")
		os.Unsetenv(v)
		t.Setenv(v+"_FILE", flagFile)

		val, exists := GetEnvOrFile(v)
		t.Logf("FILE-ONLY  %s_FILE=<file containing \"true\">, %s unset: GetEnvOrFile=(%q,%v) envTrue=%v",
			v, v, val, exists, envTrue(v))

		os.Unsetenv(v + "_FILE")
		t.Setenv(v, "true")

		val, exists = GetEnvOrFile(v)
		t.Logf("PLAIN      %s=true, %s_FILE unset: GetEnvOrFile=(%q,%v) envTrue=%v",
			v, v, val, exists, envTrue(v))

		os.Unsetenv(v)
	}
}

func TestScreenStartupLogWithLFSFile(t *testing.T) {
	flagFile := filepath.Join(t.TempDir(), "lfs")
	if err := os.WriteFile(flagFile, []byte("true\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	run := func(label string) {
		var buf bytes.Buffer

		saved := logger
		logger = log.New(&buf, "", 0)

		displayStartupConfig()

		logger = saved

		t.Logf("startup log, %s:\n%s", label, buf.String())
	}

	t.Setenv(envGitBackupDir, t.TempDir())
	t.Setenv(envGitHubToken, "dummy-token")

	t.Setenv(envGitHubBackupLFS, "")
	os.Unsetenv(envGitHubBackupLFS)
	t.Setenv(envGitHubBackupLFS+"_FILE", flagFile)
	run("GITHUB_BACKUP_LFS_FILE set, GITHUB_BACKUP_LFS unset")

	os.Unsetenv(envGitHubBackupLFS + "_FILE")
	t.Setenv(envGitHubBackupLFS, "true")
	run("GITHUB_BACKUP_LFS=true, GITHUB_BACKUP_LFS_FILE unset")
}
