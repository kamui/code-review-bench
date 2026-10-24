package internal

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// Counts the openat calls on a secret file through a marker file opened before and after.
func TestProbeBitBucketEmailReads(t *testing.T) {
	for _, kv := range os.Environ() {
		k := strings.SplitN(kv, "=", 2)[0]
		if strings.HasPrefix(k, "BITBUCKET") || strings.HasPrefix(k, "AZURE") || strings.HasPrefix(k, "GITHUB") || strings.HasPrefix(k, "GITLAB") || strings.HasPrefix(k, "GITEA") || strings.HasPrefix(k, "SOURCEHUT") {
			os.Unsetenv(k)
		}
	}
	dir := t.TempDir()
	secret := filepath.Join(dir, "probe-bb-email")
	if err := os.WriteFile(secret, []byte("a@example.com"), 0o600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("BITBUCKET_EMAIL_FILE", secret)
	os.ReadFile("/probe-marker-start")
	_ = checkProvidersDefined()
	os.ReadFile("/probe-marker-mid")
	var b strings.Builder
	t.Setenv("AZURE_DEVOPS_USERNAME_FILE", secret)
	_ = checkUserAndPasswordProviderOrProvider(&b)
	os.ReadFile("/probe-marker-end")
}
