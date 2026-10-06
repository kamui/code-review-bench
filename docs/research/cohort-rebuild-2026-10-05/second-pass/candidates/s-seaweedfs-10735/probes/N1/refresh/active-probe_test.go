package redis2

import (
	"context"
	"fmt"
	"github.com/redis/go-redis/v9"
	"github.com/seaweedfs/seaweedfs/weed/filer"
	"github.com/seaweedfs/seaweedfs/weed/util"
	"os"
	"strconv"
	"strings"
	"testing"
	"time"
)

func TestDirectoryOrphanProbe(t *testing.T) {
	ctx := context.Background()
	c := redis.NewClient(&redis.Options{Addr: os.Getenv("PROBE_REDIS_ADDR")})
	defer c.Close()
	must := func(e error) {
		if e != nil {
			t.Fatal(e)
		}
	}
	for _, prefix := range []string{"", "sw:"} {
		for _, trigger := range []string{"DEL", "allkeys-lru"} {
			must(c.ConfigSet(ctx, "maxmemory", "0").Err())
			must(c.FlushDB(ctx).Err())
			s := &UniversalRedis2Store{Client: c, keyPrefix: prefix}
			entry := func(p string, dir bool) *filer.Entry {
				e := &filer.Entry{FullPath: util.FullPath(p), Attr: filer.Attr{Crtime: time.Now()}}
				if dir {
					e.Mode = os.ModeDir | 0755
				}
				return e
			}
			must(s.InsertEntry(ctx, entry("/a", true)))
			sub := entry("/a/sub", true)
			if trigger == "allkeys-lru" {
				sub.Extended = map[string][]byte{"probe-padding": []byte(strings.Repeat("x", 2*1024*1024))}
			}
			must(s.InsertEntry(ctx, sub))
			if trigger == "allkeys-lru" {
				time.Sleep(1200 * time.Millisecond)
			}
			must(s.InsertEntry(ctx, entry("/a/sub/f", false)))
			if trigger == "DEL" {
				must(c.Del(ctx, prefix+"/a/sub").Err())
			} else {
				must(c.ConfigSet(ctx, "maxmemory-policy", "allkeys-lru").Err())
				must(c.ConfigSet(ctx, "maxmemory-samples", "10").Err())
				must(c.Get(ctx, prefix+"/a").Err())
				must(c.ZRange(ctx, prefix+"/a\x00", 0, -1).Err())
				must(c.ZRange(ctx, prefix+"/a/sub\x00", 0, -1).Err())
				must(c.Get(ctx, prefix+"/a/sub/f").Err())
				info, err := c.Info(ctx, "memory").Result()
				must(err)
				var used int64
				for _, line := range strings.Split(info, "\n") {
					if strings.HasPrefix(line, "used_memory:") {
						used, err = strconv.ParseInt(strings.TrimSpace(strings.TrimPrefix(line, "used_memory:")), 10, 64)
						must(err)
					}
				}
				must(c.ConfigSet(ctx, "maxmemory", strconv.FormatInt(used-512*1024, 10)).Err())
				fmt.Printf("pressure-write-error=%v\n", c.Set(ctx, "pressure", "x", 0).Err())
				fmt.Printf("active-memory-limit=%v\n", c.ConfigGet(ctx, "maxmemory").Val())
				fmt.Printf("eviction value=%d parent-index=%d child-index=%d child-value=%d\n", c.Exists(ctx, prefix+"/a/sub").Val(), c.Exists(ctx, prefix+"/a\x00").Val(), c.Exists(ctx, prefix+"/a/sub\x00").Val(), c.Exists(ctx, prefix+"/a/sub/f").Val())
				if c.Exists(ctx, prefix+"/a/sub").Val() != 0 || c.Exists(ctx, prefix+"/a\x00", prefix+"/a/sub\x00", prefix+"/a/sub/f").Val() != 3 {
					t.Fatal("did not reach isolated directory eviction")
				}
				stats, err := c.Info(ctx, "stats").Result()
				must(err)
				for _, line := range strings.Split(stats, "\n") {
					if strings.HasPrefix(line, "evicted_keys:") {
						fmt.Println(strings.TrimSpace(line))
					}
				}
			}
			names := func(p string) []string {
				var found []string
				_, err := s.ListDirectoryEntries(ctx, util.FullPath(p), "", false, 100, func(e *filer.Entry) (bool, error) { found = append(found, e.Name()); return true, nil })
				must(err)
				return found
			}
			fmt.Printf("native-before-list parent-index=%v child-index=%v\n", c.ZRange(ctx, prefix+"/a\x00", 0, -1).Val(), c.ZRange(ctx, prefix+"/a/sub\x00", 0, -1).Val())
			fmt.Printf("prefix=%q trigger=%s parent-list=%v\n", prefix, trigger, names("/a"))
			fmt.Printf("parent-index-after-list=%v\n", c.ZRange(ctx, prefix+"/a\x00", 0, -1).Val())
			must(s.DeleteFolderChildren(ctx, "/a"))
			must(s.DeleteEntry(ctx, "/a"))
			fmt.Printf("delete-error=nil child-index-exists=%d grandchild-value-exists=%d\n", c.Exists(ctx, prefix+"/a/sub\x00").Val(), c.Exists(ctx, prefix+"/a/sub/f").Val())
			_, err := s.FindEntry(ctx, "/a/sub/f")
			fmt.Printf("grandchild-direct-read-error=%v\n", err)
			must(s.InsertEntry(ctx, entry("/a", true)))
			must(s.InsertEntry(ctx, entry("/a/sub", true)))
			fmt.Printf("after-recreate child-list=%v\n", names("/a/sub"))
		}
	}
}
