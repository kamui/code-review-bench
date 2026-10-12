package redis2

// Shared helpers for the ruling probes of seaweedfs/seaweedfs#10735.
// Dropped into weed/filer/redis2/ at the commit before the change and at its head.
// Nothing here asserts: each scenario prints what happened so the two commits can be compared.

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/redis/go-redis/v9"

	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

var errProbeInjected = errors.New("probe: injected connection failure")

func probeAddr(t *testing.T) string {
	t.Helper()
	addr := os.Getenv("PROBE_REDIS_ADDR")
	if addr == "" {
		t.Skip("set PROBE_REDIS_ADDR to a scratch redis-server")
	}
	return addr
}

type probeTrace struct {
	mu    sync.Mutex
	lines []string
}

func (tr *probeTrace) add(format string, args ...any) {
	tr.mu.Lock()
	defer tr.mu.Unlock()
	line := fmt.Sprintf(format, args...)
	tr.lines = append(tr.lines, strings.ReplaceAll(line, "\x00", `\x00`))
}

func (tr *probeTrace) dump(t *testing.T) {
	tr.mu.Lock()
	defer tr.mu.Unlock()
	for i, line := range tr.lines {
		t.Logf("  %2d. %s", i+1, line)
	}
}

type probeRule struct {
	label string
	cmd   string
	when  string
	match func(cmd redis.Cmder) bool
	act   func(ctx context.Context, cmd redis.Cmder) error
	fired bool
}

type probeHook struct {
	actor string
	trace *probeTrace
	rules []*probeRule
}

var probeTraced = map[string]bool{"get": true, "set": true, "del": true, "exists": true, "zrem": true, "zadd": true, "zrangebylex": true, "eval": true, "evalsha": true}

func (h *probeHook) DialHook(next redis.DialHook) redis.DialHook { return next }

func (h *probeHook) ProcessPipelineHook(next redis.ProcessPipelineHook) redis.ProcessPipelineHook {
	return next
}

func (h *probeHook) fire(ctx context.Context, cmd redis.Cmder, when string) error {
	for _, rule := range h.rules {
		if rule.fired || rule.when != when || rule.cmd != cmd.Name() {
			continue
		}
		if rule.match != nil && !rule.match(cmd) {
			continue
		}
		rule.fired = true
		if err := rule.act(ctx, cmd); err != nil {
			return err
		}
	}
	return nil
}

func (h *probeHook) ProcessHook(next redis.ProcessHook) redis.ProcessHook {
	return func(ctx context.Context, cmd redis.Cmder) error {
		if !probeTraced[cmd.Name()] {
			return next(ctx, cmd)
		}
		if err := h.fire(ctx, cmd, "before"); err != nil {
			cmd.SetErr(err)
			h.trace.add("%-9s %s   [NOT SENT to redis]", h.actor+":", probeFormat(cmd))
			return err
		}
		err := next(ctx, cmd)
		cmd.SetErr(err)
		h.trace.add("%-9s %s", h.actor+":", probeFormat(cmd))
		if injected := h.fire(ctx, cmd, "after"); injected != nil {
			h.trace.add("%-9s   ^ redis applied the command above, but the client is handed: %v", "", injected)
			return injected
		}
		return err
	}
}

func probeGotNil(cmd redis.Cmder) bool { return cmd.Err() == redis.Nil }

// probeFormat prints a command and its reply, with the serialized entry bytes left out.
func probeFormat(cmd redis.Cmder) string {
	args := []string{}
	for i, arg := range cmd.Args() {
		if cmd.Name() == "set" && i == 2 {
			args = append(args, "<entry bytes>")
			continue
		}
		args = append(args, fmt.Sprint(arg))
	}
	reply := ""
	switch typed := cmd.(type) {
	case *redis.IntCmd:
		reply = fmt.Sprint(typed.Val())
	case *redis.StatusCmd:
		reply = typed.Val()
	case *redis.StringSliceCmd:
		reply = fmt.Sprint(typed.Val())
	case *redis.StringCmd:
		reply = "<entry bytes>"
	case *redis.Cmd:
		reply = fmt.Sprint(typed.Val())
	}
	if err := cmd.Err(); err == redis.Nil {
		reply = "(nil: no such key)"
	} else if err != nil {
		reply = "ERROR " + err.Error()
	}
	return strings.ToUpper(args[0]) + " " + strings.Join(args[1:], " ") + "  ->  " + reply
}

type probeEnv struct {
	t      *testing.T
	addr   string
	trace  *probeTrace
	direct *redis.Client
	dir    util.FullPath
	rules  []*probeRule
}

func newProbeEnv(t *testing.T, id, title string) *probeEnv {
	t.Helper()
	addr := probeAddr(t)
	direct := redis.NewClient(&redis.Options{Addr: addr})
	if err := direct.Ping(context.Background()).Err(); err != nil {
		t.Fatalf("connect to %s: %v", addr, err)
	}
	env := &probeEnv{
		t:      t,
		addr:   addr,
		trace:  &probeTrace{},
		direct: direct,
		dir:    util.FullPath(fmt.Sprintf("/probe_%s_%d", id, time.Now().UnixNano())),
	}
	t.Cleanup(func() {
		ctx := context.Background()
		direct.ConfigSet(ctx, "maxmemory", "0")
		keys, _ := direct.Keys(ctx, string(env.dir)+"*").Result()
		if len(keys) > 0 {
			direct.Del(ctx, keys...)
		}
		direct.Close()
	})
	t.Logf("=== %s: %s", id, title)
	t.Logf("directory %s", env.dir)
	return env
}

func (env *probeEnv) client(actor, addr string, opts *redis.Options, rules ...*probeRule) *redis.Client {
	if opts == nil {
		opts = &redis.Options{}
	}
	opts.Addr = addr
	client := redis.NewClient(opts)
	client.AddHook(&probeHook{actor: actor, trace: env.trace, rules: rules})
	env.rules = append(env.rules, rules...)
	env.t.Cleanup(func() { client.Close() })
	return client
}

func (env *probeEnv) store(client *redis.Client) *UniversalRedis2Store {
	return &UniversalRedis2Store{Client: client}
}

func (env *probeEnv) quietStore() *UniversalRedis2Store { return env.store(env.direct) }

func probeEntry(path util.FullPath) *filer.Entry {
	now := time.Now()
	return &filer.Entry{FullPath: path, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: 0644}}
}

func probeDirEntry(path util.FullPath) *filer.Entry {
	now := time.Now()
	return &filer.Entry{FullPath: path, Attr: filer.Attr{Crtime: now, Mtime: now, Mode: os.ModeDir | 0755}}
}

// seedOrphan leaves the state a Redis TTL expiry or an eviction leaves: the name is still in the
// directory index, its value key is gone.
func (env *probeEnv) seedOrphan(name string) {
	env.t.Helper()
	ctx := context.Background()
	store := env.quietStore()
	if err := store.InsertEntry(ctx, probeDirEntry(env.dir)); err != nil {
		env.t.Fatalf("seed directory: %v", err)
	}
	path := env.dir.Child(name)
	if err := store.InsertEntry(ctx, probeEntry(path)); err != nil {
		env.t.Fatalf("seed %s: %v", path, err)
	}
	if err := env.direct.Del(ctx, string(path)).Err(); err != nil {
		env.t.Fatalf("drop value of %s: %v", path, err)
	}
	env.t.Logf("seeded: %q is in the directory index, its value key is gone (as after a Redis TTL expiry)", name)
}

type probeLister interface {
	ListDirectoryEntries(ctx context.Context, dirPath util.FullPath, startFileName string, includeStartFile bool, limit int64, eachEntryFunc filer.ListEachEntryFunc) (string, error)
}

func probeListVia(lister probeLister, ctx context.Context, dir util.FullPath) ([]string, error) {
	names := []string{}
	_, err := lister.ListDirectoryEntries(ctx, dir, "", true, 100, func(entry *filer.Entry) (bool, error) {
		_, name := entry.FullPath.DirAndName()
		names = append(names, name)
		return true, nil
	})
	return names, err
}

func (env *probeEnv) state(label string, names ...string) {
	env.t.Helper()
	ctx := context.Background()
	indexKey := genDirectoryListKey(string(env.dir))
	indexExists, _ := env.direct.Exists(ctx, indexKey).Result()
	members, _ := env.direct.ZRange(ctx, indexKey, 0, -1).Result()
	dirExists, _ := env.direct.Exists(ctx, string(env.dir)).Result()
	parts := []string{
		fmt.Sprintf("directory entry exists=%d", dirExists),
		fmt.Sprintf("index key exists=%d members=%v", indexExists, members),
	}
	for _, name := range names {
		exists, _ := env.direct.Exists(ctx, string(env.dir.Child(name))).Result()
		parts = append(parts, fmt.Sprintf("value of %q exists=%d", name, exists))
	}
	env.t.Logf("STATE %s: %s", label, strings.Join(parts, "; "))
}

func (env *probeEnv) laterListing(label string) {
	env.t.Helper()
	names, err := probeListVia(env.quietStore(), context.Background(), env.dir)
	env.t.Logf("LATER LISTING %s: names=%v err=%v", label, names, err)
}

// aftermath walks what the owner of the entry can do next.
func (env *probeEnv) aftermath(name string) {
	env.t.Helper()
	ctx := context.Background()
	store := env.quietStore()
	path := env.dir.Child(name)

	env.laterListing("(a fresh listing, healthy redis)")
	env.laterListing("(a second fresh listing)")

	entry, err := store.FindEntry(ctx, path)
	env.t.Logf("FindEntry(%s): found=%v err=%v", path, entry != nil && err == nil, err)

	err = store.UpdateEntry(ctx, probeEntry(path))
	env.t.Logf("UpdateEntry(%s) (what an overwrite of an existing path runs): err=%v", path, err)
	env.laterListing("(after the UpdateEntry)")
	env.state("after UpdateEntry", name)

	err = store.DeleteFolderChildren(ctx, env.dir)
	env.t.Logf("DeleteFolderChildren(%s) (what a recursive delete of the directory runs): err=%v", env.dir, err)
	env.state("after DeleteFolderChildren", name)

	err = store.InsertEntry(ctx, probeEntry(path))
	env.t.Logf("InsertEntry(%s) (a delete followed by a re-create reaches this): err=%v", path, err)
	env.laterListing("(after the InsertEntry)")
}

func (env *probeEnv) report(names []string, err error) {
	env.t.Helper()
	env.t.Logf("command order as seen by redis:")
	env.trace.dump(env.t)
	env.t.Logf("THE LISTING RETURNED: names=%v err=%v", names, err)
	env.notReached()
}

// notReached names every planned step whose trigger command was never issued at this commit.
func (env *probeEnv) notReached() {
	env.t.Helper()
	for _, rule := range env.rules {
		if !rule.fired {
			env.t.Logf("NOT REACHED at this commit (the listing never issued the %s it hangs on): %s", strings.ToUpper(rule.cmd), rule.label)
		}
	}
}

// probeProxy is a TCP relay between one client and redis, so that real transport faults (not
// errors made up inside the client) can be placed at a chosen point.
type probeProxy struct {
	t      *testing.T
	target string
	addr   string

	mu    sync.Mutex
	ln    net.Listener
	conns map[net.Conn]struct{}

	// withhold: bytes from redis are read and thrown away, so the client waits for a reply.
	withhold atomic.Bool
	// dropNextReply: the next reply from redis is thrown away once and that connection is closed.
	dropNextReply atomic.Bool
	// delayReplies: every reply is held back this long before it is relayed.
	delayReplies atomic.Int64
}

func newProbeProxy(t *testing.T, target string) *probeProxy {
	t.Helper()
	proxy := &probeProxy{t: t, target: target, conns: map[net.Conn]struct{}{}}
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("proxy listen: %v", err)
	}
	proxy.addr = ln.Addr().String()
	proxy.serve(ln)
	t.Cleanup(proxy.down)
	return proxy
}

func (p *probeProxy) serve(ln net.Listener) {
	p.mu.Lock()
	p.ln = ln
	p.mu.Unlock()
	go func() {
		for {
			clientConn, err := ln.Accept()
			if err != nil {
				return
			}
			serverConn, err := net.Dial("tcp", p.target)
			if err != nil {
				clientConn.Close()
				continue
			}
			p.mu.Lock()
			p.conns[clientConn] = struct{}{}
			p.conns[serverConn] = struct{}{}
			p.mu.Unlock()
			go func() {
				io.Copy(serverConn, clientConn)
				serverConn.Close()
			}()
			go func() {
				buf := make([]byte, 64*1024)
				for {
					n, err := serverConn.Read(buf)
					if n > 0 {
						if p.dropNextReply.CompareAndSwap(true, false) {
							clientConn.Close()
							serverConn.Close()
							return
						}
						if delay := p.delayReplies.Load(); delay > 0 {
							time.Sleep(time.Duration(delay))
						}
						if !p.withhold.Load() {
							if _, werr := clientConn.Write(buf[:n]); werr != nil {
								serverConn.Close()
								return
							}
						}
					}
					if err != nil {
						clientConn.Close()
						return
					}
				}
			}()
		}
	}()
}

// down is a redis that has become unreachable: the listener is gone and open connections are cut.
func (p *probeProxy) down() {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.ln != nil {
		p.ln.Close()
		p.ln = nil
	}
	for conn := range p.conns {
		conn.Close()
	}
	p.conns = map[net.Conn]struct{}{}
}

func (p *probeProxy) up() {
	p.mu.Lock()
	stillUp := p.ln != nil
	p.mu.Unlock()
	if stillUp {
		return
	}
	ln, err := net.Listen("tcp", p.addr)
	if err != nil {
		p.t.Fatalf("proxy relisten: %v", err)
	}
	p.serve(ln)
}

// recreateAfterMiss places a complete InsertEntry of the same path right after the listing's GET
// came back empty, which is the window the pull request's second commit was written for.
func (env *probeEnv) recreateAfterMiss(name string) *probeRule {
	return &probeRule{label: "the re-create", cmd: "get", when: "after", match: probeGotNil, act: func(ctx context.Context, cmd redis.Cmder) error {
		inserter := env.store(env.client("inserter", env.addr, nil))
		err := inserter.InsertEntry(context.Background(), probeEntry(env.dir.Child(name)))
		env.trace.add("%-9s InsertEntry(%s) returned err=%v", "inserter:", name, err)
		return nil
	}}
}

// recursiveDelete issues the store calls the filer issues to delete a directory recursively:
// list the children, DeleteFolderChildren, then DeleteEntry of the directory itself
// (weed/filer/filer_delete_entry.go, doBatchDeleteFolderMetaAndData then doDeleteEntryMetaAndData).
func (env *probeEnv) recursiveDelete() {
	ctx := context.Background()
	deleter := env.store(env.client("deleter", env.addr, nil))
	names, err := probeListVia(deleter, ctx, env.dir)
	env.trace.add("%-9s the filer's listing before the delete saw names=%v err=%v", "deleter:", names, err)
	err = deleter.DeleteFolderChildren(ctx, env.dir)
	env.trace.add("%-9s DeleteFolderChildren returned err=%v", "deleter:", err)
	err = deleter.DeleteEntry(ctx, env.dir)
	env.trace.add("%-9s DeleteEntry(directory) returned err=%v", "deleter:", err)
}
