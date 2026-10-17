package redis2

// Probe P1 for seaweedfs/seaweedfs#10735: the redis_cluster2 section of the scaffolded filer.toml
// names the replica-read switch `readOnly`; the store's Initialize reads `useReadOnly`.
// Dropped into weed/filer/redis2/ at the commit before the change and at its head.
//
// Part 1 loads the scaffold file of the commit under test (weed/command/scaffold/filer.toml), with
// one edit per case, into viper and calls RedisCluster2Store.Initialize(config, "redis_cluster2."),
// the same call weed/filer/configuration.go makes for an enabled store. It prints the go-redis
// ClusterOptions the store ends up with.
// Part 2 (PROBE_CLUSTER_ADDRS set: a real Redis cluster, 3 masters and 3 replicas) writes one
// entry with the store's InsertEntry, then reads it 30 times with FindEntry, and counts on every
// node how many GET commands it served (INFO commandstats after CONFIG RESETSTAT).
// Part 3 (same cluster) holds back replication on the replica of the entry's slot with
// CLIENT PAUSE ... WRITE, creates a new file and reads it at once, to show what a read served by a
// replica can return. Nothing asserts.

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"
	"github.com/spf13/viper"

	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

func p1ScaffoldText(t *testing.T) string {
	t.Helper()
	_, here, _, _ := runtime.Caller(0)
	path := filepath.Join(filepath.Dir(here), "..", "..", "command", "scaffold", "filer.toml")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read scaffold: %v", err)
	}
	return string(raw)
}

// p1Section returns the scaffold with the [redis_cluster2] section edited: enabled, the cluster's
// addresses, and the given replacement lines applied inside that section only.
func p1Section(t *testing.T, text string, addrs []string, edits map[string]string, extra []string) string {
	t.Helper()
	start := strings.Index(text, "[redis_cluster2]\n")
	if start < 0 {
		t.Fatalf("no [redis_cluster2] section in the scaffold")
	}
	end := strings.Index(text[start+1:], "\n[")
	if end < 0 {
		end = len(text) - start - 1
	}
	section := text[start : start+1+end]
	lines := strings.Split(section, "\n")
	out := []string{}
	skipAddrs := false
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if skipAddrs {
			if trimmed == "]" {
				skipAddrs = false
			}
			continue
		}
		if strings.HasPrefix(trimmed, "addresses = [") {
			quoted := []string{}
			for _, a := range addrs {
				quoted = append(quoted, strconv.Quote(a))
			}
			out = append(out, "addresses = ["+strings.Join(quoted, ", ")+"]")
			skipAddrs = !strings.HasSuffix(trimmed, "]")
			continue
		}
		if trimmed == "enabled = false" {
			out = append(out, "enabled = true")
			continue
		}
		replaced := false
		for old, repl := range edits {
			if trimmed == old {
				out = append(out, repl)
				replaced = true
				break
			}
		}
		if !replaced {
			out = append(out, line)
		}
	}
	out = append(out[:1], append(extra, out[1:]...)...)
	return text[:start] + strings.Join(out, "\n") + text[start+1+end:]
}

func p1Load(t *testing.T, text string) *util.ViperProxy {
	t.Helper()
	v := viper.New()
	v.SetConfigType("toml")
	if err := v.ReadConfig(strings.NewReader(text)); err != nil {
		t.Fatalf("parse filer.toml: %v", err)
	}
	// util.GetViper() sets these on the filer's own viper.
	v.AutomaticEnv()
	v.SetEnvPrefix("weed")
	v.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))
	return &util.ViperProxy{Viper: v}
}

type p1Case struct {
	name  string
	edits map[string]string
	extra []string
	env   map[string]string
}

var p1Cases = []p1Case{
	{name: "scaffold as shipped (enabled, addresses set)", edits: nil},
	{name: "the documented key: readOnly = true", edits: map[string]string{"readOnly = false": "readOnly = true"}},
	{name: "the key the code reads: useReadOnly = true (not in the scaffold)", extra: []string{"useReadOnly = true"}},
	{name: "the other documented key: routeByLatency = true", edits: map[string]string{"routeByLatency = false": "routeByLatency = true"}},
	{name: "the documented environment variable WEED_REDIS_CLUSTER2_READONLY=true, scaffold unchanged", env: map[string]string{"WEED_REDIS_CLUSTER2_READONLY": "true"}},
}

func p1Addrs() []string {
	if raw := os.Getenv("PROBE_CLUSTER_ADDRS"); raw != "" {
		return strings.Split(raw, ",")
	}
	return []string{"localhost:30001", "localhost:30002", "localhost:30003"}
}

func p1Store(t *testing.T, c p1Case) (*RedisCluster2Store, string) {
	t.Helper()
	for k := range map[string]bool{"WEED_REDIS_CLUSTER2_READONLY": true, "WEED_REDIS_CLUSTER2_USEREADONLY": true} {
		os.Unsetenv(k)
	}
	for k, v := range c.env {
		os.Setenv(k, v)
	}
	t.Cleanup(func() {
		for k := range c.env {
			os.Unsetenv(k)
		}
	})
	text := p1Section(t, p1ScaffoldText(t), p1Addrs(), c.edits, c.extra)
	cfg := p1Load(t, text)
	store := &RedisCluster2Store{}
	if err := store.Initialize(cfg, "redis_cluster2."); err != nil {
		t.Fatalf("Initialize: %v", err)
	}
	return store, text
}

func p1SectionText(text string) string {
	start := strings.Index(text, "[redis_cluster2]\n")
	end := strings.Index(text[start+1:], "\n[")
	return text[start : start+1+end]
}

func p1NodeRoles(ctx context.Context, addrs []string) map[string]string {
	roles := map[string]string{}
	for _, a := range addrs {
		c := redis.NewClient(&redis.Options{Addr: a})
		role, err := c.Do(ctx, "ROLE").Slice()
		if err == nil && len(role) > 0 {
			roles[a] = fmt.Sprint(role[0])
		} else {
			roles[a] = "?"
		}
		c.Close()
	}
	return roles
}

func p1GetCalls(ctx context.Context, addr string) int {
	c := redis.NewClient(&redis.Options{Addr: addr})
	defer c.Close()
	info, _ := c.Info(ctx, "commandstats").Result()
	for _, line := range strings.Split(info, "\n") {
		if strings.HasPrefix(line, "cmdstat_get:calls=") {
			rest := strings.TrimPrefix(line, "cmdstat_get:calls=")
			n, _ := strconv.Atoi(rest[:strings.Index(rest, ",")])
			return n
		}
	}
	return 0
}

func TestProbeP1(t *testing.T) {
	ctx := context.Background()

	t.Run("1_options_the_store_ends_up_with", func(t *testing.T) {
		for i, c := range p1Cases {
			store, text := p1Store(t, c)
			opts := store.Client.(*redis.ClusterClient).Options()
			if i == 0 {
				t.Logf("the [redis_cluster2] section as loaded for case %q:\n%s", c.name, p1SectionText(text))
			}
			t.Logf("CASE %q -> ClusterOptions.ReadOnly=%v RouteByLatency=%v RouteRandomly=%v", c.name, opts.ReadOnly, opts.RouteByLatency, opts.RouteRandomly)
			store.Shutdown()
		}
	})

	addrsEnv := os.Getenv("PROBE_CLUSTER_ADDRS")
	if addrsEnv == "" {
		t.Logf("PROBE_CLUSTER_ADDRS not set: parts 2 and 3 not run")
		return
	}
	addrs := strings.Split(addrsEnv, ",")
	roles := p1NodeRoles(ctx, addrs)

	t.Run("2_which_nodes_serve_the_reads", func(t *testing.T) {
		for _, c := range p1Cases {
			store, _ := p1Store(t, c)
			dir := util.FullPath(fmt.Sprintf("/probe_P1_%d", time.Now().UnixNano()))
			path := dir.Child("f")
			if err := store.InsertEntry(ctx, probeEntry(path)); err != nil {
				t.Fatalf("InsertEntry: %v", err)
			}
			time.Sleep(300 * time.Millisecond)
			for _, a := range addrs {
				c := redis.NewClient(&redis.Options{Addr: a})
				c.ConfigResetStat(ctx)
				c.Close()
			}
			found := 0
			for i := 0; i < 30; i++ {
				if entry, err := store.FindEntry(ctx, path); err == nil && entry != nil {
					found++
				}
			}
			served := []string{}
			masterGets, replicaGets := 0, 0
			for _, a := range addrs {
				n := p1GetCalls(ctx, a)
				if n == 0 {
					continue
				}
				served = append(served, fmt.Sprintf("%s (%s): %d", a, roles[a], n))
				if roles[a] == "master" {
					masterGets += n
				} else {
					replicaGets += n
				}
			}
			t.Logf("CASE %q -> 30 FindEntry calls, %d found; GET served by masters=%d replicas=%d [%s]", c.name, found, masterGets, replicaGets, strings.Join(served, "; "))
			store.Client.Del(ctx, string(path))
			store.Client.Del(ctx, genDirectoryListKey(string(dir)))
			store.Shutdown()
		}
	})

	t.Run("3_a_read_right_after_a_create_while_the_replica_lags", func(t *testing.T) {
		for _, c := range p1Cases {
			store, _ := p1Store(t, c)
			cluster := store.Client.(*redis.ClusterClient)
			dir := util.FullPath(fmt.Sprintf("/probe_P1L_%d", time.Now().UnixNano()))
			path := dir.Child("f")
			finder := redis.NewClusterClient(&redis.ClusterOptions{Addrs: addrs, ReadOnly: true})
			replica, err := finder.SlaveForKey(ctx, string(path))
			if err != nil {
				t.Fatalf("SlaveForKey: %v", err)
			}
			if roles[replica.Options().Addr] == "master" {
				t.Fatalf("expected a replica for %s, got master %s", path, replica.Options().Addr)
			}
			_ = cluster
			if err := replica.Do(ctx, "CLIENT", "PAUSE", "3000", "WRITE").Err(); err != nil {
				t.Fatalf("pause replica: %v", err)
			}
			insertErr := store.InsertEntry(ctx, &filer.Entry{FullPath: path, Attr: filer.Attr{Crtime: time.Now(), Mtime: time.Now(), Mode: 0644}})
			_, findErr := store.FindEntry(ctx, path)
			replica.Do(ctx, "CLIENT", "UNPAUSE")
			time.Sleep(300 * time.Millisecond)
			_, findAfter := store.FindEntry(ctx, path)
			t.Logf("CASE %q -> replica %s held back; InsertEntry err=%v; FindEntry at once err=%v; FindEntry after the replica caught up err=%v", c.name, replica.Options().Addr, insertErr, findErr, findAfter)
			store.Client.Del(ctx, string(path))
			store.Client.Del(ctx, genDirectoryListKey(string(dir)))
			store.Shutdown()
			finder.Close()
		}
	})
}
