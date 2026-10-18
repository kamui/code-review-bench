package internal

// Probe for q-soba-195 group P3, copied into internal/ of a checkout and run with
// `go test ./internal/ -run TestProbe -v -count=1`. It prints what the start-up provider check
// counts for a token provider configured in several ways.

import (
	"os"
	"strings"
	"testing"
)

func TestProbeProviderCount(t *testing.T) {
	clear := func() {
		for _, kv := range os.Environ() {
			name, _, _ := strings.Cut(kv, "=")
			for _, prefix := range []string{"GITHUB_", "GITEA_", "GITLAB_", "BITBUCKET_", "AZURE_DEVOPS_", "SOURCEHUT_"} {
				if strings.HasPrefix(name, prefix) {
					t.Setenv(name, "")
					os.Unsetenv(name)
				}
			}
		}
	}

	cases := []struct {
		label    string
		provider string
		env      map[string]string
	}{
		{"Gitea, API URL only", providerNameGitea, map[string]string{envGiteaAPIURL: "https://gitea.example.invalid/api/v1"}},
		{"Gitea, token only", providerNameGitea, map[string]string{envGiteaToken: "t"}},
		{"Gitea, API URL and token", providerNameGitea, map[string]string{envGiteaAPIURL: "https://gitea.example.invalid/api/v1", envGiteaToken: "t"}},
		{"Sourcehut, API URL only", providerNameSourcehut, map[string]string{envSourcehutAPIURL: "https://git.sr.ht.example.invalid/query"}},
		{"Sourcehut, token only", providerNameSourcehut, map[string]string{envSourcehutToken: "t"}},
		{"GitHub, token only", providerNameGitHub, map[string]string{envGitHubToken: "t"}},
	}

	for _, c := range cases {
		clear()

		for k, v := range c.env {
			t.Setenv(k, v)
		}

		n, err := checkProvider(c.provider)
		t.Logf("%-28s checkProvider(%q) = (%d, %v); checkProvidersDefined() = %v", c.label, c.provider, n, err, checkProvidersDefined())
	}
}
