import json
import os
from pathlib import Path
import subprocess

ROOT = Path('/home/jack/.cache/regrade-v4/docs/research/rule-v4-candidates-2026-10-27/s-seaweedfs-10735')
SCRATCH = Path('/home/jack/.cache/rule-v4-candidates-scratch/s-seaweedfs-10735')
CLONE = SCRATCH / 'clone'
env = dict(os.environ, GOMODCACHE=str(SCRATCH / 's-seaweedfs-10735/gomodcache'), GOCACHE=str(SCRATCH / 'gocache'), GOTMPDIR=str(SCRATCH / 'tmp'), TMPDIR=str(SCRATCH / 'tmp'), PROBE_REDIS_ADDR='127.0.0.1:56379')
revisions = {'base': 'db5a086d048c5c2d6e51e82bb070d20df04d688d', 'head': '6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1'}
for side, revision in revisions.items():
    subprocess.run(['git', 'checkout', '--detach', revision], cwd=CLONE, check=True)
    subprocess.run(['go', 'build', '-o', str(SCRATCH / ('probe-' + side)), str(ROOT / 'probes/probe.go')], cwd=CLONE, env=env, check=True)

def run(group, side, mode, count=3, delay=0, logical=False, append=False):
    directory = ROOT / 'probes' / group
    directory.mkdir(exist_ok=True, parents=True)
    command = [str(SCRATCH / ('probe-' + side)), '-logtostderr=true']
    settings = dict(env, PROBE_MODE=mode, PROBE_COUNT=str(count), PROBE_DELAY_US=str(delay), PROBE_LOGICAL=str(int(logical)))
    result = subprocess.run(command, env=settings, capture_output=True, text=True)
    with (directory / ('result-' + side + '.txt')).open('a' if append else 'w') as f:
        f.write(f'command={command!r}; mode={mode}; count={count}; artificial delay_us={delay}; logical={logical}; revision={revisions[side]}\n')
        f.write(result.stdout + '\nSTDERR:\n' + result.stderr + f'\nexit={result.returncode}\n')
    if result.returncode:
        raise RuntimeError(result.stderr)
    return [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]

for group, mode in [('P1', 'P1'), ('P2', 'P2'), ('M1b', 'M1b')]:
    for side in revisions:
        run(group, side, mode)
        if group == 'P2':
            run(group, side, mode, logical=True, append=True)

samples = []
for count, delay in [(64, 0), (64, 1000), (1024, 1000)]:
    for repetition in range(5):
        for side in revisions:
            rows = run('M1a', side, 'M1a', count, delay, append=True)
            samples.extend([dict(row, side=side, repetition=repetition + 1) for row in rows])
(ROOT / 'probes/M1a/samples.json').write_text(json.dumps(samples, indent=2) + '\n')
print('Completed base/head probes and five alternating repetitions per latency workload.')
