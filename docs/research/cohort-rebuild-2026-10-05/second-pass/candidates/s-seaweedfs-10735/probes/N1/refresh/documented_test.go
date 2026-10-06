package redis2
import (
 "context"
 "fmt"
 "os"
 "testing"
 "time"
 "github.com/redis/go-redis/v9"
 "github.com/seaweedfs/seaweedfs/weed/filer"
 "github.com/seaweedfs/seaweedfs/weed/util"
)
func TestDocumentedWay(t *testing.T) {
 ctx:=context.Background()
 c:=redis.NewClient(&redis.Options{Addr:os.Getenv("PROBE_REDIS_ADDR")})
 defer c.Close()
 must:=func(err error){if err!=nil {t.Fatal(err)}}
 must(c.ConfigSet(ctx,"maxmemory","0").Err())
 must(c.ConfigSet(ctx,"maxmemory-policy","noeviction").Err())
 must(c.FlushDB(ctx).Err())
 s:=&UniversalRedis2Store{Client:c}
 entry:=func(p string, dir bool)*filer.Entry{
  e:=&filer.Entry{FullPath:util.FullPath(p),Attr:filer.Attr{Crtime:time.Now()},Content:[]byte("first")}
  if dir {e.Mode=os.ModeDir|0755};return e
 }
 list:=func(p string)[]string{
  var names []string
  _,err:=s.ListDirectoryEntries(ctx,util.FullPath(p),"",false,100,func(e *filer.Entry)(bool,error){names=append(names,e.Name());return true,nil})
  must(err);return names
 }
 must(s.InsertEntry(ctx,entry("/normal",true)))
 must(s.InsertEntry(ctx,entry("/normal/f",false)))
 fmt.Printf("ordinary-insert-list=%v\n",list("/normal"))
 e,err:=s.FindEntry(ctx,"/normal/f");must(err);e.Content=[]byte("second")
 must(s.UpdateEntry(ctx,e));e,err=s.FindEntry(ctx,e.FullPath);must(err)
 fmt.Printf("ordinary-update-read=%q list=%v\n",e.Content,list("/normal"))
 must(s.DeleteFolderChildren(ctx,"/normal"));must(s.DeleteEntry(ctx,"/normal"))
 must(s.InsertEntry(ctx,entry("/normal",true)))
 fmt.Printf("ordinary-delete-recreate-list=%v\n",list("/normal"))
 must(s.InsertEntry(ctx,entry("/normal/f",false)))
 must(c.Del(ctx,"/normal/f").Err());list("/normal")
 must(s.InsertEntry(ctx,entry("/normal/f",false)))
 fmt.Printf("documented-insert-recreation-list=%v\n",list("/normal"))
}
