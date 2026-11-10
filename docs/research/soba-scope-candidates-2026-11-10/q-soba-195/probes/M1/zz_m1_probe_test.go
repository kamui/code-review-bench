package internal

import (
	"bytes"
	"log"
	"os"
	"testing"
)

// Prints the lines displayStartupConfig writes for three settings, with the log prefix removed
// so that base and head output can be compared line for line.
func TestM1ProbeStartupLog(t *testing.T) {
	scenarios := []struct {
		name string
		env  map[string]string
	}{
		{"each provider's guard variable set", map[string]string{
			"GIT_BACKUP_DIR": "/backups", "GITHUB_TOKEN": "x", "GITEA_TOKEN": "x", "GITLAB_TOKEN": "x",
			"BITBUCKET_EMAIL": "x", "AZURE_DEVOPS_USERNAME": "x", "SOURCEHUT_PAT": "x",
		}},
		{"Sourcehut token only", map[string]string{"GIT_BACKUP_DIR": "/backups", "SOURCEHUT_PAT": "x"}},
		{"BitBucket OAuth set only", map[string]string{
			"GIT_BACKUP_DIR": "/backups", "BITBUCKET_USER": "x", "BITBUCKET_KEY": "x", "BITBUCKET_SECRET": "x",
		}},
	}

	saved := logger
	defer func() { logger = saved }()

	for _, sc := range scenarios {
		for _, k := range sobaEnvVarKeys {
			if k != envPath {
				_ = os.Unsetenv(k)
			}
		}

		for k, v := range sc.env {
			t.Setenv(k, v)
		}

		var buf bytes.Buffer
		logger = log.New(&buf, "", 0)

		displayStartupConfig()

		t.Logf("M1-PROBE scenario: %s\n%s", sc.name, buf.String())
	}
}
