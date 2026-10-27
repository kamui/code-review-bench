package transport_test

import (
 "fmt"
 "testing"
 "time"
 "github.com/golang/protobuf/ptypes"
 "google.golang.org/protobuf/types/known/durationpb"
)

func TestM1Values(t *testing.T) {
 for _, d := range []*durationpb.Duration{{}, {Seconds:-1}, {Nanos:-1}, {Seconds:-10000000000}, {Seconds:10000000000}, {Nanos:50000000}} {
  old, err := ptypes.Duration(d)
  value := d.AsDuration()
  fmt.Printf("seconds=%d nanos=%d old=%v olderr=%v CheckValid=%v AsDuration=%v ", d.Seconds,d.Nanos,old,err,d.CheckValid(),value)
  func() {
   defer func(){if p:=recover();p!=nil {fmt.Printf("ticker-panic=%v\n",p)}}()
   ticker:=time.NewTicker(value);ticker.Stop();fmt.Println("ticker-created")
  }()
 }
}
