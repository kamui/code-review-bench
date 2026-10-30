package main

import (
	"context"
	"encoding/json"
	flag "github.com/seaweedfs/seaweedfs/weed/util/fla9"
	"fmt"
	"net"
	"os"
	"strconv"
	"sync/atomic"
	"time"

	r "github.com/redis/go-redis/v9"
	"github.com/seaweedfs/seaweedfs/weed/filer"
	old "github.com/seaweedfs/seaweedfs/weed/filer/redis"
	"github.com/seaweedfs/seaweedfs/weed/filer/redis2"
	"github.com/seaweedfs/seaweedfs/weed/pb"
	"github.com/seaweedfs/seaweedfs/weed/util"
)

var ctx = context.Background()

type trace struct {
	active bool
 stale bool
	delay time.Duration
	counts map[string]int
}

func (h *trace) DialHook(next r.DialHook) r.DialHook {
	return func(c context.Context, network, addr string) (net.Conn, error) { return next(c, network, addr) }
}
func (h *trace) ProcessHook(next r.ProcessHook) r.ProcessHook {
	return func(c context.Context, cmd r.Cmder) error {
		if h.stale && cmd.Name()=="get" {
   cmd.(*r.StringCmd).SetErr(r.Nil);return r.Nil
  }
  if h.stale && cmd.Name()=="exists" {
   cmd.(*r.IntCmd).SetVal(0);return nil
  }
  err := next(c, cmd)
		if h.active {
			h.counts[cmd.Name()]++
			time.Sleep(h.delay)
		}
		return err
	}
}
func (h *trace) ProcessPipelineHook(next r.ProcessPipelineHook) r.ProcessPipelineHook {
 return func(c context.Context,cmds []r.Cmder) error {
  err:=next(c,cmds)
  if h.active {h.counts["pipeline"]++;time.Sleep(h.delay)}
  return err
 }
}
func must(err error) { if err != nil { panic(err) } }
func emit(x map[string]interface{}) { b, err := json.Marshal(x); must(err); fmt.Println(string(b)) }
func entry(p string, ttl int32, age time.Duration) *filer.Entry {
	now := time.Now().Add(-age)
	return &filer.Entry{FullPath: util.FullPath(p), Attr: filer.Attr{Mode: 0644, Crtime: now, Mtime: now, TtlSec: ttl}}
}
func names(s filer.FilerStore, p string, limit int64) ([]string, error) {
	got := []string{}
	_, err := s.ListDirectoryEntries(ctx, util.FullPath(p), "", false, limit, func(e *filer.Entry) (bool,error) {got=append(got,e.Name()); return true,nil})
	return got,err
}

type observedStore struct {
	*redis2.Redis2Store
	callbacks int
	deletes int
}
func (s *observedStore) ListDirectoryEntries(c context.Context, p util.FullPath, start string, incl bool, limit int64, fn filer.ListEachEntryFunc) (string,error) {
	return s.Redis2Store.ListDirectoryEntries(c,p,start,incl,limit,func(e *filer.Entry)(bool,error){s.callbacks++; return fn(e)})
}
func (s *observedStore) DeleteEntry(c context.Context,p util.FullPath) error {s.deletes++;return s.Redis2Store.DeleteEntry(c,p)}

func lifecycle(c *r.Client) {
	for _, mode := range []string{"physical", "logical", "mtime-first-control", "no-physical-expiry-control"} {
		must(c.FlushDB(ctx).Err())
		st := &observedStore{Redis2Store: &redis2.Redis2Store{}}
		st.Client=c
		var notifications atomic.Int64
		f:=filer.NewFiler(pb.ServerDiscovery{},nil,"","","","","",255,func(){notifications.Add(1)})
		f.DirBucketsPath="/buckets"
		f.SetStore(st)
		parent:=entry("/buckets/b/p",0,0);parent.Mode=os.ModeDir|0755
		must(st.InsertEntry(ctx,parent))
		e:=entry("/buckets/b/p/f",1,0)
		e.Extended=map[string][]byte{"X-Seaweedfs-Expires-S3":[]byte("true")}
		if mode=="logical" {e.Crtime=time.Now().Add(-3*time.Second);e.Mtime=e.Crtime}
		if mode=="mtime-first-control" {e.TtlSec=10;e.Mtime=time.Now().Add(-20*time.Second)}
		must(st.InsertEntry(ctx,e))
		if mode=="physical" {time.Sleep(1200*time.Millisecond)}
		if mode=="no-physical-expiry-control" {must(c.Persist(ctx,string(e.FullPath)).Err());time.Sleep(1200*time.Millisecond)}
		before,_:=c.Exists(ctx,string(e.FullPath)).Result()
		notifications.Store(0);st.callbacks=0;st.deletes=0
		got,more,err:=f.ListDirectoryEntries(ctx,"/buckets/b/p","",false,10,"","","")
		must(err)
		after,_:=c.Exists(ctx,string(e.FullPath)).Result()
		parentAfter,_:=c.Exists(ctx,string(parent.FullPath)).Result()
		members,_:=c.ZRange(ctx,"/buckets/b/p\x00",0,-1).Result()
		emit(map[string]interface{}{"probe":"P1","mode":mode,"s3Flag":e.IsExpireS3Enabled(),"valueBefore":before,"valueAfter":after,"callbackCount":st.callbacks,"filerDeleteCalls":st.deletes,"metadataNotifications":notifications.Load(),"parentAfter":parentAfter,"membersAfter":members,"returned":len(got),"more":more,"error":nil})
	}
}

func main() {
	flag.Parse()
	mode:=os.Getenv("PROBE_MODE")
	c:=r.NewClient(&r.Options{Addr:os.Getenv("PROBE_REDIS_ADDR"),MaxRetries:-1,Protocol:2,DisableIdentity:true})
	defer c.Close()
	must(c.Ping(ctx).Err())
	if mode=="P1" {lifecycle(c);return}
 if mode=="GT-s1-sim" {
  must(c.FlushDB(ctx).Err())
  st:=&redis2.Redis2Store{};st.Client=c
  must(st.InsertEntry(ctx,entry("/replica/f",0,0)))
  h:=&trace{stale:true};c.AddHook(h)
  got,err:=names(st,"/replica",10);must(err)
  h.stale=false
  again,err:=names(st,"/replica",10);must(err)
  emit(map[string]interface{}{"probe":mode,"forcedStaleListing":got,"afterCatchupListing":again,"valueExists":c.Exists(ctx,"/replica/f").Val(),"membersAfter":c.ZRange(ctx,"/replica\x00",0,-1).Val()})
  return
 }
 if mode=="PE-s6" {
  must(c.FlushDB(ctx).Err())
  st:=&redis2.Redis2Store{};st.Client=c
  dir:=entry("/parent/sub",10,20*time.Second);dir.Mode=os.ModeDir|0755
  must(st.InsertEntry(ctx,dir));must(st.InsertEntry(ctx,entry("/parent/sub/x",0,0)))
  got,err:=names(st,"/parent",10);must(err)
  must(st.DeleteFolderChildren(ctx,"/parent"));must(st.DeleteEntry(ctx,"/parent"))
  listAfter:=c.ZRange(ctx,"/parent/sub\x00",0,-1).Val()
  must(st.InsertEntry(ctx,entry("/parent/sub",0,0)))
  again,err:=names(st,"/parent/sub",10);must(err)
  emit(map[string]interface{}{"probe":"PE-s6","initialListing":got,"childIndexAfterParentDelete":listAfter,"recreatedListing":again})
  return
 }
 if mode=="PE-s1" {
  must(c.FlushDB(ctx).Err())
  st:=&redis2.Redis2Store{};st.Client=c
  must(st.InsertEntry(ctx,entry("/page/a",1,0)));must(st.InsertEntry(ctx,entry("/page/b",0,0)))
  time.Sleep(1200*time.Millisecond)
  got,err:=names(st,"/page",1);must(err)
  emit(map[string]interface{}{"probe":"PE-s1","listing":got,"liveValue":c.Exists(ctx,"/page/b").Val()})
  return
 }
	must(c.FlushDB(ctx).Err())
	count,_:=strconv.Atoi(os.Getenv("PROBE_COUNT")); if count==0 {count=3}
	delay,_:=strconv.Atoi(os.Getenv("PROBE_DELAY_US"))
	h:=&trace{counts:map[string]int{},delay:time.Duration(delay)*time.Microsecond};c.AddHook(h)
	var st filer.FilerStore
	var set bool
	if mode=="P2" {x:=&old.RedisStore{};x.Client=c;st=x;set=true} else {x:=&redis2.Redis2Store{};x.Client=c;st=x}
	for i:=0;i<count;i++ {must(st.InsertEntry(ctx,entry(fmt.Sprintf("/probe/f-%05d",i),1,0)))}
	if os.Getenv("PROBE_LOGICAL")=="1" {
		for i:=0;i<count;i++ {must(st.UpdateEntry(ctx,entry(fmt.Sprintf("/probe/f-%05d",i),10,20*time.Second)))}
	} else {time.Sleep(1200*time.Millisecond)}
	for pass:=1;pass<=2;pass++ {
		h.counts=map[string]int{};h.active=true;start:=time.Now()
		got,err:=names(st,"/probe",int64(count));elapsed:=time.Since(start);h.active=false;must(err)
		var remaining int64
		if set {remaining=c.SCard(ctx,"/probe\x00").Val()} else {remaining=c.ZCard(ctx,"/probe\x00").Val()}
		emit(map[string]interface{}{"probe":mode,"pass":pass,"count":count,"delay_us":delay,"elapsed_ms":float64(elapsed.Microseconds())/1000,"commands":h.counts,"returned":got,"remaining":remaining,"error":nil})
	}
	must(st.InsertEntry(ctx,entry("/live/ok",0,0)))
	got,err:=names(st,"/live",10);must(err)
	emit(map[string]interface{}{"probe":"live-control","returned":got,"error":nil})
}
