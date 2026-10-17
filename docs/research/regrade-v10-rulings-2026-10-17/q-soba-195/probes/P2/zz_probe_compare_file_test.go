package internal

// Probe for q-soba-195 group P2, copied into internal/ of a checkout and run with
// `go test ./internal/ -run TestProbe -v -count=1`. Each provider is called with an empty
// backup directory: the provider builds its githosts host, which logs the compare method it
// will use, and the host's Backup() then returns at once without any network call.
// Azure DevOps is the exception, handled below.

import (
	"bytes"
	"errors"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/hashicorp/go-retryablehttp"
)

type probeNoNetwork struct{}

func (probeNoNetwork) RoundTrip(*http.Request) (*http.Response, error) {
	return nil, errors.New("probe: network disabled")
}

var probeProviders = []struct {
	name    string
	compare string
	creds   map[string]string
	run     func(string) *ProviderBackupResults
	logs    bool
}{
	{"GitHub", envGitHubCompare, map[string]string{envGitHubToken: "t"}, GitHub, true},
	{"Gitea", envGiteaCompare, map[string]string{envGiteaToken: "t", envGiteaAPIURL: "http://127.0.0.1:9/api/v1"}, Gitea, true},
	{"GitLab", envGitLabCompare, map[string]string{envGitLabToken: "t"}, Gitlab, true},
	{"BitBucket", envBitBucketCompare, map[string]string{envBitBucketEmail: "e@example.invalid", envBitBucketAPIToken: "t"}, Bitbucket, true},
	{"Azure DevOps", envAzureDevOpsCompare, map[string]string{envAzureDevOpsUserName: "u", envAzureDevOpsPAT: "t", envAzureDevOpsOrgs: "o"}, AzureDevOps, true},
	{"Sourcehut", envSourcehutCompare, map[string]string{envSourcehutToken: "t"}, Sourcehut, false},
}

func probeCapture(f func()) string {
	var buf bytes.Buffer

	saved := logger
	logger = log.New(&buf, "", 0)

	defer func() { logger = saved }()

	f()

	return buf.String()
}

func probeClearEnv(t *testing.T) {
	t.Helper()

	for _, kv := range os.Environ() {
		name, _, _ := strings.Cut(kv, "=")
		for _, prefix := range []string{"GITHUB_", "GITEA_", "GITLAB_", "BITBUCKET_", "AZURE_DEVOPS_", "SOURCEHUT_", "GIT_BACKUP_DIR"} {
			if strings.HasPrefix(name, prefix) {
				t.Setenv(name, "")
				os.Unsetenv(name)
			}
		}
	}
}

func TestProbeCompareMethodFromFile(t *testing.T) {
	refsFile := filepath.Join(t.TempDir(), "refs")
	if err := os.WriteFile(refsFile, []byte("refs\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	for _, p := range probeProviders {
		for _, mode := range []string{"FILE", "PLAIN"} {
			probeClearEnv(t)

			for k, v := range p.creds {
				t.Setenv(k, v)
			}

			if mode == "FILE" {
				t.Setenv(p.compare+"_FILE", refsFile)
			} else {
				t.Setenv(p.compare, "refs")
			}

			startup := strings.TrimSpace(probeCapture(displayStartupConfig))

			// githosts writes its own log line to stdout; the test's -v output shows it right
			// after this marker.
			os.Stdout.WriteString("    >>> " + p.name + " " + mode + ": githosts says:\n")
			// Azure DevOps refuses an empty backup directory before it logs the method, so it gets a
			// real one and an HTTP client whose every request fails at once, without a network call.
			dir := ""
			if p.name == "Azure DevOps" {
				dir = t.TempDir()
				httpClient = retryablehttp.NewClient()
				httpClient.RetryMax = 0
				httpClient.Logger = nil
				httpClient.HTTPClient.Transport = probeNoNetwork{}
			}

			res := p.run(dir)
			httpClient = nil

			t.Logf("%s %s: provider result error: %v", p.name, mode, res.Results.Error)
			t.Logf("%s %s: setting=%s, start-up log: %q", p.name, mode,
				map[string]string{"FILE": p.compare + "_FILE=<file holding \"refs\">", "PLAIN": p.compare + "=refs"}[mode],
				grepLine(startup, p.name+" compare method"))
		}
	}
}

func grepLine(s, sub string) string {
	for _, l := range strings.Split(s, "\n") {
		if strings.Contains(l, sub) {
			return l
		}
	}

	return "(none)"
}

// Other settings the start-up log reads through GetEnvOrFile: what the backup's own readers return.
func TestProbeOtherSettingsFromFile(t *testing.T) {
	dir := t.TempDir()
	write := func(name, content string) string {
		p := filepath.Join(dir, name)
		if err := os.WriteFile(p, []byte(content), 0o600); err != nil {
			t.Fatal(err)
		}

		return p
	}

	probeClearEnv(t)
	t.Setenv(envGitHubToken, "t")
	t.Setenv(envGiteaToken, "t")
	t.Setenv(envGitHubOrgs+"_FILE", write("orgs", "org-a,org-b\n"))
	t.Setenv(envGiteaBackups+"_FILE", write("keep", "7\n"))

	startup := probeCapture(displayStartupConfig)
	t.Logf("GITHUB_ORGS_FILE=<file holding org-a,org-b>: start-up log %q; getOrgsListFromEnvVar=%q",
		grepLine(startup, "Organistations"), getOrgsListFromEnvVar(envGitHubOrgs))
	t.Logf("GITEA_BACKUPS_FILE=<file holding 7>: start-up log %q; getBackupsToRetain=%d",
		grepLine(startup, "backups to keep"), getBackupsToRetain(envGiteaBackups))
}
