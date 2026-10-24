package internal

// Probe for group P1: what soba's start-up check does with a partial BitBucket credential set.
// Copied into internal/ of a checkout and run with
//   go test ./internal -run TestZZProbeBitBucketPartial -v -count=1
// It calls checkProvider and checkProvidersDefined directly with controlled variables.

import (
	"fmt"
	"log"
	"os"
	"testing"
)

var zzProbeVars = []string{
	"GITHUB_TOKEN", "GITLAB_TOKEN", "GITEA_APIURL", "GITEA_TOKEN", "SOURCEHUT_APIURL", "SOURCEHUT_PAT",
	"AZURE_DEVOPS_USERNAME", "AZURE_DEVOPS_PAT",
	"BITBUCKET_EMAIL", "BITBUCKET_API_TOKEN", "BITBUCKET_USER", "BITBUCKET_KEY", "BITBUCKET_SECRET",
}

func zzProbeClear() {
	for _, v := range zzProbeVars {
		_ = os.Unsetenv(v)
		_ = os.Unsetenv(v + "_FILE")
	}
}

func TestZZProbeBitBucketPartial(t *testing.T) {
	if logger == nil {
		logger = log.New(os.Stdout, "soba: ", 0)
	}

	cases := []struct {
		name string
		env  map[string]string
	}{
		{"BITBUCKET_EMAIL only", map[string]string{"BITBUCKET_EMAIL": "me@example.invalid"}},
		{"BITBUCKET_EMAIL and GITHUB_TOKEN", map[string]string{"BITBUCKET_EMAIL": "me@example.invalid", "GITHUB_TOKEN": "t"}},
		{"BITBUCKET_USER, BITBUCKET_KEY and GITHUB_TOKEN", map[string]string{"BITBUCKET_USER": "me", "BITBUCKET_KEY": "k", "GITHUB_TOKEN": "t"}},
		{"BITBUCKET_EMAIL and BITBUCKET_API_TOKEN (complete)", map[string]string{"BITBUCKET_EMAIL": "me@example.invalid", "BITBUCKET_API_TOKEN": "t"}},
		{"AZURE_DEVOPS_USERNAME and GITHUB_TOKEN", map[string]string{"AZURE_DEVOPS_USERNAME": "me", "GITHUB_TOKEN": "t"}},
	}

	for _, c := range cases {
		zzProbeClear()

		for k, v := range c.env {
			_ = os.Setenv(k, v)
		}

		fmt.Printf("=== %s\n", c.name)

		for _, p := range []string{providerNameBitBucketAPIToken, providerNameBitBucketOAuth, providerNameAzureDevOps} {
			n, err := checkProvider(p)
			fmt.Printf("checkProvider(%q) = %d, %q\n", p, n, zzErrString(err))
		}

		fmt.Printf("checkProvidersDefined() = %q\n", zzErrString(checkProvidersDefined()))
	}

	zzProbeClear()
}

func zzErrString(err error) string {
	if err == nil {
		return "<nil>"
	}

	return err.Error()
}
