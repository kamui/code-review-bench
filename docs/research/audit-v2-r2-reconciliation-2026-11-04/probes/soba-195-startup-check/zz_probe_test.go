package internal

// Probe for the reconciliation round audit-v2-r2-reconciliation-2026-11-04.
// Counts how many times each *_FILE secret is opened, with a Linux inotify watch
// for IN_OPEN and IN_CLOSE_NOWRITE on each secret file (open and close alternate,
// so the kernel does not merge two opens into one event).

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"syscall"
	"testing"
	"unsafe"
)

var probeVars = []string{
	envBitBucketEmail, envBitBucketAPIToken, envBitBucketUser, envBitBucketKey, envBitBucketSecret,
	envAzureDevOpsUserName, envAzureDevOpsPAT, envGitHubToken, envGitLabToken,
	envGiteaToken, envGiteaAPIURL, envSourcehutToken, envSourcehutAPIURL, envGitHubOrgs,
}

func probeClearEnv(t *testing.T) {
	for _, v := range probeVars {
		for _, name := range []string{v, v + "_FILE"} {
			t.Setenv(name, "")
			_ = os.Unsetenv(name)
		}
	}
}

type openCount struct{ n int64 }

func (o *openCount) Load() int64 { return o.n }

// serveFile writes content to a file, points NAME_FILE at it and watches it for opens.
func serveFile(t *testing.T, name, content string) *watched {
	path := filepath.Join(t.TempDir(), strings.ToLower(name))
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
	fd, err := syscall.InotifyInit1(syscall.IN_NONBLOCK | syscall.IN_CLOEXEC)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := syscall.InotifyAddWatch(fd, path, syscall.IN_OPEN|syscall.IN_CLOSE_NOWRITE); err != nil {
		t.Fatal(err)
	}
	w := &watched{fd: fd}
	t.Cleanup(func() { _ = syscall.Close(fd) })
	t.Setenv(name+"_FILE", path)
	return w
}

type watched struct {
	fd    int
	opens openCount
}

// drain reads the queued events and adds the opens to the count.
func (w *watched) drain() {
	buf := make([]byte, 64*1024)
	for {
		n, err := syscall.Read(w.fd, buf)
		if err != nil || n <= 0 {
			return
		}
		for off := 0; off+syscall.SizeofInotifyEvent <= n; {
			ev := (*syscall.InotifyEvent)(unsafe.Pointer(&buf[off]))
			if ev.Mask&syscall.IN_OPEN != 0 {
				w.opens.n++
			}
			off += syscall.SizeofInotifyEvent + int(ev.Len)
		}
	}
}

func report(label string, files map[string]*watched, extra string) {
	for _, w := range files {
		w.drain()
	}
	names := make([]string, 0, len(files))
	for n := range files {
		names = append(names, n)
	}
	sort.Strings(names)
	parts := []string{}
	total := int64(0)
	for _, n := range names {
		o := files[n].opens.Load()
		total += o
		parts = append(parts, fmt.Sprintf("%s_FILE opened %d", n, o))
	}
	fmt.Printf("PROBE %s: %s; total %d%s\n", label, strings.Join(parts, ", "), total, extra)
}

func errText(err error) string {
	if err == nil {
		return "nil"
	}
	return strings.ReplaceAll(err.Error(), "\n", " | ")
}

// The situation of unit #22: BitBucket API-token auth, BITBUCKET_EMAIL_FILE set, BITBUCKET_API_TOKEN missing.
func TestProbeBitBucketEmailFileTokenMissing(t *testing.T) {
	probeClearEnv(t)
	t.Setenv(envGitHubToken, "ghp_example")
	email := serveFile(t, envBitBucketEmail, "someone@example.com")
	err := checkProvidersDefined()
	report("#22 start-up check checkProvidersDefined(), BITBUCKET_EMAIL_FILE set, token missing, GITHUB_TOKEN set",
		map[string]*watched{envBitBucketEmail: email}, ", returned "+errText(err))
	before := email.opens.Load()
	displayStartupConfig()
	email.drain()
	fmt.Printf("PROBE #22 displayStartupConfig() in the same setup opened BITBUCKET_EMAIL_FILE %d more time(s)\n", email.opens.Load()-before)
}

// The only three-parameter provider, BitBucket OAuth, through the start-up check.
func TestProbeBitBucketOAuthThroughStartupCheck(t *testing.T) {
	for _, tc := range []struct {
		label  string
		secret string
	}{
		{"complete", "secret-value"},
		{"secret file blank", ""},
	} {
		t.Run(tc.label, func(t *testing.T) {
			probeClearEnv(t)
			t.Setenv(envGitHubToken, "ghp_example")
			files := map[string]*watched{
				envBitBucketUser:   serveFile(t, envBitBucketUser, "user"),
				envBitBucketKey:    serveFile(t, envBitBucketKey, "key"),
				envBitBucketSecret: serveFile(t, envBitBucketSecret, tc.secret),
			}
			err := checkProvidersDefined()
			report("#20 start-up check checkProvidersDefined(), BitBucket OAuth via files ("+tc.label+")", files, ", returned "+errText(err))
		})
	}
}

// Azure DevOps, the one two-parameter provider that reaches the double loop.
func TestProbeAzureDevOpsThroughStartupCheck(t *testing.T) {
	for _, tc := range []struct {
		label string
		pat   string
	}{
		{"complete", "pat-value"},
		{"PAT file blank", ""},
	} {
		t.Run(tc.label, func(t *testing.T) {
			probeClearEnv(t)
			files := map[string]*watched{
				envAzureDevOpsUserName: serveFile(t, envAzureDevOpsUserName, "user"),
				envAzureDevOpsPAT:      serveFile(t, envAzureDevOpsPAT, tc.pat),
			}
			err := checkProvidersDefined()
			report("start-up check checkProvidersDefined(), Azure DevOps via files ("+tc.label+")", files, ", returned "+errText(err))
		})
	}
}

// What the double-loop code does when handed a three-parameter provider directly.
func TestProbeThreeParamDirect(t *testing.T) {
	for _, tc := range []struct {
		label     string
		secret    string
		setSecret bool
	}{
		{"all three set", "secret-value", true},
		{"secret file blank", "", true},
		{"secret not set at all", "", false},
	} {
		t.Run(tc.label, func(t *testing.T) {
			probeClearEnv(t)
			files := map[string]*watched{
				envBitBucketUser: serveFile(t, envBitBucketUser, "user"),
				envBitBucketKey:  serveFile(t, envBitBucketKey, "key"),
			}
			if tc.setSecret {
				files[envBitBucketSecret] = serveFile(t, envBitBucketSecret, tc.secret)
			}
			fn, n := probeDirectThreeParam()
			report(fn+"(BitBucketOAuth) called directly ("+tc.label+")", files, fmt.Sprintf(", returned %d", n))
		})
	}
}

// The situation of units #29 and #36: what checkProvidersDefined returns, for every
// combination of the two BitBucket sets and one other provider, over many map orders.
func TestProbeCheckProvidersDefinedResults(t *testing.T) {
	sets := []string{"none", "partial", "complete"}
	for _, api := range sets {
		for _, oauth := range sets {
			for _, gh := range []bool{false, true} {
				probeClearEnv(t)
				switch api {
				case "partial":
					t.Setenv(envBitBucketEmail, "someone@example.com")
				case "complete":
					t.Setenv(envBitBucketEmail, "someone@example.com")
					t.Setenv(envBitBucketAPIToken, "token")
				}
				switch oauth {
				case "partial":
					t.Setenv(envBitBucketUser, "user")
					t.Setenv(envBitBucketKey, "key")
				case "complete":
					t.Setenv(envBitBucketUser, "user")
					t.Setenv(envBitBucketKey, "key")
					t.Setenv(envBitBucketSecret, "secret")
				}
				if gh {
					t.Setenv(envGitHubToken, "ghp_example")
				}
				seen := map[string]int{}
				for i := 0; i < 500; i++ {
					seen[errText(checkProvidersDefined())]++
				}
				keys := []string{}
				for k, v := range seen {
					keys = append(keys, fmt.Sprintf("%q x%d", k, v))
				}
				sort.Strings(keys)
				fmt.Printf("PROBE results api-token=%s oauth=%s github=%v: %s\n", api, oauth, gh, strings.Join(keys, "; "))
			}
		}
	}
}
