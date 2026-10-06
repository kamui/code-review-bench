import os, pathlib, shutil, subprocess, time
root=pathlib.Path.cwd()
out=root/'docs/research/cohort-rebuild-2026-10-05/second-pass/candidates/s-seaweedfs-10735/probes'
scratch=pathlib.Path('<scratch>/s-seaweedfs-10735')
clone=scratch/'refresh-clone'
redis=scratch/'refresh-redis-src/src/redis-server'
env=dict(os.environ,GOPATH=str(scratch/'refresh-go'),GOMODCACHE=str(scratch/'refresh-go/pkg/mod'),GOCACHE=str(scratch/'refresh-go-cache'),GOTOOLCHAIN='local',PROBE_REDIS_ADDR='127.0.0.1:16389',RUN_REDIS_TESTS='1',REDIS_ADDR='127.0.0.1:16389')
log=open(scratch/'refresh-redis.log','w')
server=subprocess.Popen([str(redis),'--port','16389','--bind','127.0.0.1','--save','','--appendonly','no','--dir',str(scratch)],stdout=log,stderr=subprocess.STDOUT)
def run(cmd, dest, sha):
 result=subprocess.run(cmd,cwd=clone,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
 dest.write_text('commit='+sha+'\ncommand='+' '.join(cmd)+'\nexit='+str(result.returncode)+'\n'+result.stdout)
 print(dest.relative_to(root),result.returncode,flush=True)
try:
 time.sleep(.4)
 for label,sha in [('head','6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1')]:
  subprocess.run(['git','checkout','--quiet','--detach',sha],cwd=clone,check=True)
  for group in ['N1','N3']:
   target=clone/'weed/filer/redis2/zz_refresh_test.go'
   shutil.copy(out/group/'refresh/active-probe_test.go',target)
   run(['go','test','./weed/filer/redis2','-run','Probe$','-v','-count=1'],out/group/'refresh'/('result-active-'+label+'.txt'),sha)
   target.unlink()
  if label=='head':
   for group in ['N1','N3']:
    target=clone/'weed/filer/redis/zz_refresh_test.go'
    shutil.copy(out/group/'refresh/active-comparable_test.go.txt',target)
    run(['go','test','./weed/filer/redis','-run','Probe$','-v','-count=1'],out/group/'refresh/result-active-comparable-head.txt',sha)
    target.unlink()
 versions=[]
 for cmd in [['go','version'],[str(redis),'--version'],['git','--version'],['python3','--version'],['gcc','--version']]:
  versions.append(subprocess.run(cmd,capture_output=True,text=True).stdout.splitlines()[0])
 for group in ['N1','N3']:
  (out/group/'refresh/environment-active.txt').write_text('\n'.join(versions)+'\nRedis 8.2.1 from git tag, built with make -j4 MALLOC=libc BUILD_TLS=no OPTIMIZATION=-O2 redis-server redis-cli.\nFresh go caches under scratch. Original project source is unchanged.\nThe earlier probes are reused with native Redis observations added. The comparable store is redis v1 at the pinned head, not a different vendor.\n2 MiB padding forces eviction. The memory limit and eviction policy remain enabled throughout the operations after eviction.\nNo full filer, HTTP calls, volume deletion, Cluster or Sentinel run.\n')
finally:
 server.terminate();server.wait(timeout=10);log.close()
