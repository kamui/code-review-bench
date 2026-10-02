package main
import (
 "fmt"
 "time"
 "github.com/golang/protobuf/ptypes"
 "google.golang.org/grpc/codes"
 "google.golang.org/grpc/encoding"
 _ "google.golang.org/grpc/encoding/proto"
 "google.golang.org/grpc/status"
 "google.golang.org/grpc/internal/binarylog"
 old "google.golang.org/grpc/reflection/grpc_testing_not_regenerate"
 "google.golang.org/protobuf/types/known/durationpb"
)
func main() {
 m:=&old.SearchRequestV3{Query:"claim-probe"}
 codec:=encoding.GetCodec("proto")
 b,err:=codec.Marshal(m);fmt.Printf("codec marshal bytes=%d error=%v\n",len(b),err)
 fmt.Printf("codec unmarshal error=%v\n",codec.Unmarshal([]byte{10,1,120},&old.SearchRequestV3{}))
 s,err:=status.New(codes.Internal,"probe").WithDetails(m)
 if err!=nil {fmt.Printf("status error=%v\n",err)} else {d:=s.Details()[0];_,ok:=d.(*old.SearchRequestV3);fmt.Printf("detail type=%T original_type=%v\n",d,ok)}
 ml:=binarylog.NewTruncatingMethodLogger(1000,1000)
 entry:=ml.Build(&binarylog.ServerMessage{Message:m});fmt.Printf("binary log length=%d bytes=%d\n",entry.GetMessage().GetLength(),len(entry.GetMessage().GetData()))
 for _,seconds:=range []int64{10000000000,-10000000000,-1} {
  d:=&durationpb.Duration{Seconds:seconds};v,e:=ptypes.Duration(d)
  fmt.Printf("duration seconds=%d old=%s old_error=%v new=%s new_error=%v\n",seconds,v,e,d.AsDuration(),d.CheckValid())
  func(){defer func(){if e:=recover();e!=nil{fmt.Printf("ticker panic=%v\n",e)}}(); tick:=time.NewTicker(d.AsDuration());tick.Stop()}()
 }
}
