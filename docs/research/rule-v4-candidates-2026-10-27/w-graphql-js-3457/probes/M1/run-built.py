import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
CLONE = Path('/home/jack/.cache/rule-v4-candidates-scratch/w-graphql-js-3457/clone')
env = {**os.environ,'BABEL_DISABLE_CACHE':'1','NODE_ENV':'production'}
for name,sha in [('base','730d5af8e933235fd5aa312a00be465db0b8acf5'),('head','efdbbcfacd92adb78e002f2b8a61f2a6a1504c19')]:
    subprocess.run(['git','checkout','--detach',sha],cwd=CLONE,check=True,capture_output=True)
    build = subprocess.run(['node','resources/build-npm.js'],cwd=CLONE,env=env,capture_output=True,text=True)
    (HERE / ('build-'+name+'.txt')).write_text(build.stdout+build.stderr)
    if build.returncode:
        raise RuntimeError('Package build failed: '+name)
    probe = subprocess.run(['node',str(HERE/'probe.cjs'),str(CLONE),'built'],cwd=CLONE,env={**env,'PROBE_REVISION':'built-'+name},capture_output=True,text=True)
    (HERE / ('result-built-'+name+'.txt')).write_text(probe.stdout)
    (HERE / ('stderr-built-'+name+'.txt')).write_text(probe.stderr)
    if probe.returncode:
        raise RuntimeError(probe.stderr)
    print('Built and probed',name)
