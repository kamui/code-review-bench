package grpc

import (
 "context"
 "errors"
 "net"
 "sync"
 "testing"
 "time"
 "google.golang.org/grpc/backoff"
 "google.golang.org/grpc/balancer"
 "google.golang.org/grpc/connectivity"
 "google.golang.org/grpc/credentials/insecure"
 "google.golang.org/grpc/resolver"
)

var p1TransportReturned = func(context.Context) {}

type p1Strategy struct { mu sync.Mutex; indices []int; events chan int }
func (b *p1Strategy) Backoff(n int) time.Duration {
 b.mu.Lock(); b.indices=append(b.indices,n); b.mu.Unlock()
 b.events<-n
 return 20*time.Millisecond
}
func p1Wait(t *testing.T, ch <-chan struct{}, name string) {
 t.Helper()
 select {case <-ch: case <-time.After(3*time.Second): t.Fatal("timeout: "+name)}
}
func p1State(t *testing.T, ac *addrConn, want connectivity.State) {
 t.Helper()
 deadline:=time.Now().Add(3*time.Second)
 for time.Now().Before(deadline) {
  ac.mu.Lock(); state:=ac.state; ch:=ac.stateChan; ac.mu.Unlock()
  if state==want {return}
  select {case <-ch: case <-time.After(10*time.Millisecond):}
 }
 t.Fatalf("state did not become %v",want)
}
func TestP1(t *testing.T) {
 for _, mode:=range []string{"completed-retained","completed-superseded","incomplete-superseded"} {
 t.Run(mode,func(t *testing.T){
  lis,err:=net.Listen("tcp","127.0.0.1:0"); if err!=nil {t.Fatal(err)}
  defer lis.Close()
  srv:=NewServer(); defer srv.Stop()
  if mode=="incomplete-superseded" {
   go func(){ c,e:=lis.Accept(); if e==nil {defer c.Close(); buf:=make([]byte,1024); for {if _,e=c.Read(buf);e!=nil{return}}} }()
  } else {go srv.Serve(lis)}
  replacementDial:=make(chan struct{},1); releaseReplacement:=make(chan struct{})
  cc,err:=NewClient("passthrough:///unused",WithTransportCredentials(insecure.NewCredentials()),WithConnectParams(ConnectParams{Backoff:backoff.Config{BaseDelay:time.Millisecond,Multiplier:2,MaxDelay:time.Second},MinConnectTimeout:time.Second}),WithContextDialer(func(ctx context.Context, addr string)(net.Conn,error){
   if addr=="replacement" {select {case replacementDial<-struct{}{}:default:}; select{case <-releaseReplacement:return nil,errors.New("replacement deliberately unavailable"); case <-ctx.Done():return nil,ctx.Err()}}
   return (&net.Dialer{}).DialContext(ctx,"tcp",addr)
  }))
  if err!=nil {t.Fatal(err)}
  defer cc.Close()
  cc.mu.Lock(); ac,err:=cc.newAddrConnLocked([]resolver.Address{{Addr:lis.Addr().String()}},balancer.NewSubConnOptions{}); cc.mu.Unlock()
  if err!=nil {t.Fatal(err)}
  ac.acbw=&acBalancerWrapper{ac:ac,ccb:cc.balancerWrapper,stateListener:func(balancer.SubConnState){}}
  bs:=&p1Strategy{events:make(chan int,20)}
  ac.dopts.bs=bs
  ac.backoffIdx=5
  oldCtx:=ac.ctx
  returned:=make(chan struct{}); releaseOld:=make(chan struct{})
  p1TransportReturned=func(ctx context.Context){if ctx==oldCtx {close(returned); <-releaseOld}}
  oldDone:=make(chan struct{})
  go func(){defer close(oldDone); ac.connect()}()
  if mode=="incomplete-superseded" {
   p1State(t,ac,connectivity.Connecting)
  } else {p1Wait(t,returned,"transport returned after SETTINGS")}
  if mode=="completed-retained" {
   close(releaseOld); p1Wait(t,oldDone,"retained reset")
   ac.mu.Lock(); idx:=ac.backoffIdx; state:=ac.state; ac.mu.Unlock()
   t.Logf("SETTINGS received; retained transport: index=%d state=%s",idx,state)
   if idx!=0 || state!=connectivity.Ready {t.Fatal("retained control mismatch")}
   return
  }
  ac.updateAddrs([]resolver.Address{{Addr:"replacement"}})
  p1Wait(t,replacementDial,"replacement dial")
  ac.mu.Lock(); before:=ac.backoffIdx; ac.mu.Unlock()
  if mode=="completed-superseded" {close(releaseOld)}
  p1Wait(t,oldDone,"obsolete attempt returned")
  ac.mu.Lock(); after:=ac.backoffIdx; ready:=ac.transport!=nil; ac.mu.Unlock()
  t.Logf("mode=%s old_ctx=%v before=%d after=%d usable_transport=%v",mode,oldCtx.Err(),before,after,ready)
  want:=0; if mode=="incomplete-superseded" {want=5}
  if before!=5 || after!=want || ready {t.Fatal("superseded result mismatch")}
  close(releaseReplacement)
  p1State(t,ac,connectivity.Idle)
  nextDone:=make(chan struct{}); go func(){defer close(nextDone); ac.connect()}()
  p1Wait(t,nextDone,"subsequent failure")
  bs.mu.Lock(); indices:=append([]int(nil),bs.indices...); bs.mu.Unlock()
  t.Logf("Backoff calculator input sequence=%v; seeded index=5; each completed failed backoff increments once",indices)
  expected:=want+1
  if len(indices)!=3 || indices[0]!=5 || indices[1]!=5 || indices[2]!=expected {t.Fatalf("indices=%v want [5 5 %d]",indices,expected)}
 })
 }
}
