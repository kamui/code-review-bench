import os
import pathlib
import shutil
import subprocess
import sys
import time

root = pathlib.Path.cwd()
scratch = pathlib.Path('<scratch>')
clone = scratch / 'clone'
out = root / 'docs/research/cohort-rebuild-2026-10-05/second-pass/candidates/s-seaweedfs-10735/probes'
redis = scratch / 'redis-build/redis-8.2.1/src/redis-server'
server = subprocess.Popen([str(redis), '--port', '16379', '--bind', '127.0.0.1', '--save', '', '--appendonly', 'no', '--dir', str(scratch)], stdout=open(scratch / 'redis.log', 'w'), stderr=subprocess.STDOUT)
env = dict(os.environ, GOPATH=str(scratch / 'go'), GOMODCACHE=str(scratch / 'go/pkg/mod'), GOCACHE=str(scratch / 'go-build-cache'), PROBE_REDIS_ADDR='127.0.0.1:16379')
try:
    time.sleep(0.3)
    later = sys.argv[1:] == ['later']
    groups = ['N1', 'N3'] if later else (sys.argv[1:] or ['N1', 'N3'])
    for group in groups:
        revisions = [('base','db5a086d048c5c2d6e51e82bb070d20df04d688d'), ('head','6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1')]
        if later:
            revisions = [('release-4.42','f04da8e9ad8db7ae06bd2caba1b6560b36e28e31')]
            if group == 'N1': revisions.insert(0, ('followup-10743','9d076beada7ed0b75842919d2d03ad46aa829169'))
        for label, sha in revisions:
            target = clone / 'weed/filer/redis2/zz_ruling_probe_test.go'
            target.unlink(missing_ok=True)
            subprocess.run(['git','checkout','--detach',sha],cwd=clone,check=True,capture_output=True)
            shutil.copy(out / group / 'probe_test.go',target)
            subprocess.run(['gofmt','-w',str(target)],check=True)
            shutil.copy(target,out / group / 'probe_test.go')
            result = subprocess.run(['go','test','./weed/filer/redis2','-run','Probe$','-v','-count=1'],cwd=clone,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
            (out / group / f'result-{label}.txt').write_text(f'commit={sha}\ncommand=go test ./weed/filer/redis2 -run Probe$ -v -count=1\nexit={result.returncode}\n{result.stdout}')
            print(group,label,result.returncode,result.stdout,flush=True)
        versions = [subprocess.run(cmd,env=env,capture_output=True,text=True).stdout.strip() for cmd in [['go','version'],[str(redis),'--version'],['git','--version'],['python3','--version'],['gcc','--version']]]
        (out / group / ('environment-later.txt' if later else 'environment.txt')).write_text('\n'.join(versions)+'\nRedis built from https://download.redis.io/releases/redis-8.2.1.tar.gz\nmake -j4 MALLOC=libc BUILD_TLS=no OPTIMIZATION=-O2 redis-server redis-cli\nStandalone Redis on 127.0.0.1:16379; persistence disabled.\nDependencies and build cache isolated under scratch.\nOriginal store source compiled without edits. Store calls forced into the stated order.\nFull filer, volume data, cluster and Sentinel not exercised.\n')
finally:
    server.terminate()
    server.wait(timeout=10)
