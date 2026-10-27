import difflib
import json
from pathlib import Path
import os
import subprocess

ROOT = Path('/home/jack/.cache/regrade-v4/docs/research/rule-v4-candidates-2026-10-27/s-seaweedfs-10735')
SCRATCH = Path('/home/jack/.cache/rule-v4-candidates-scratch/s-seaweedfs-10735')
CLONE = SCRATCH / 'clone'
env = dict(os.environ, GOMODCACHE=str(SCRATCH / 's-seaweedfs-10735/gomodcache'), GOCACHE=str(SCRATCH / 'gocache'), GOTMPDIR=str(SCRATCH / 'tmp'), TMPDIR=str(SCRATCH / 'tmp'), PROBE_REDIS_ADDR='127.0.0.1:56379')
paths = ['weed/filer/redis2/universal_redis_store.go', 'weed/filer/redis/universal_redis_store.go']
head = '6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1'
original = {p: subprocess.check_output(['git', 'show', head + ':' + p], cwd=CLONE, text=True) for p in paths}

def modify(which):
    source = dict(original)
    p = paths[0]
    if 'P1' in which:
        source[p] = source[p].replace('if err = store.Client.Set(ctx, store.getKey(string(entry.FullPath)), value, time.Duration(entry.TtlSec)*time.Second).Err(); err != nil {', 'ttl := time.Duration(entry.TtlSec)*time.Second\n\tif entry.IsExpireS3Enabled() { ttl = 0 }\n\tif err = store.Client.Set(ctx, store.getKey(string(entry.FullPath)), value, ttl).Err(); err != nil {')
        source[p] = source[p].replace('if entry.TtlSec > 0 {', 'if entry.TtlSec > 0 && !entry.IsExpireS3Enabled() {')
    if 'PE-s6' in which:
        source[p] = source[p].replace('store.Client.Del(ctx, store.getKey(string(path))).Result()\n\t\t\t\t\tstore.Client.ZRem', 'if entry.IsDirectory() { store.Client.Del(ctx, store.getKey(genDirectoryListKey(string(path)))) }\n\t\t\t\t\tstore.Client.Del(ctx, store.getKey(string(path))).Result()\n\t\t\t\t\tstore.Client.ZRem')
    if 'P2' in which:
        q = paths[1]
        source[q] = source[q].replace('if err == filer_pb.ErrNotFound {\n\t\t\t\terr = nil', 'if err == filer_pb.ErrNotFound {\n\t\t\t\tstore.Client.SRem(ctx, dirListKey, fileName)\n\t\t\t\terr = nil')
    if 'PE-s1' in which:
        source[p] = source[p].replace('Count:  limit,', 'Count:  0,').replace('var entry *filer.Entry', 'var entry *filer.Entry\n\tvar delivered int64').replace('if !resEachEntryFunc {', 'delivered++\n\t\t\tif !resEachEntryFunc || delivered >= limit {')
    if 'GT-s1' in which:
        source[p] = source[p].replace('store.Client.Exists(ctx, store.getKey(string(path))).Result()', 'store.Client.Eval(ctx, "return redis.call(\'EXISTS\', KEYS[1])", []string{store.getKey(string(path))}).Int64()')
    if 'M1a' in which:
        start = source[p].index('\tif err := store.Client.ZRem(ctx, dirListKey, fileName).Err(); err != nil {', source[p].index('func (store *UniversalRedis2Store) removeOrphanedDirectoryListMember'))
        end = source[p].index('\n\tif err == nil && exists == 0 {', start)
        exists = 'pipe.Eval(ctx, "return redis.call(\'EXISTS\', KEYS[1])", []string{store.getKey(string(path))})' if 'GT-s1' in which else 'pipe.Exists(ctx, store.getKey(string(path)))'
        result = 'Int64()' if 'GT-s1' in which else 'Result()'
        source[p] = source[p][:start] + '\tpipe := store.Client.Pipeline()\n\tremoved := pipe.ZRem(ctx, dirListKey, fileName)\n\tcheck := '+exists+'\n\t_, _ = pipe.Exec(ctx)\n\tif removed.Err() != nil { return }\n\texists, err := check.'+result+source[p][end:]
    return source

plans = {
    'P1': [('unpatched', []), ('fix-P1', ['P1']), ('fix-PE-s6', ['PE-s6']), ('both', ['P1', 'PE-s6'])],
    'P2': [('unpatched', []), ('fix-P2', ['P2']), ('fix-PE-s1', ['PE-s1']), ('both', ['P2', 'PE-s1'])],
    'M1a': [('unpatched', []), ('fix-M1a', ['M1a']), ('fix-GT-s1', ['GT-s1']), ('both', ['M1a', 'GT-s1'])],
}
for group, variants in plans.items():
    directory = ROOT / 'probes' / group
    for variant, which in variants:
        changed = modify(which)
        patch = ''.join(''.join(difflib.unified_diff(original[p].splitlines(True), changed[p].splitlines(True), fromfile='a/'+p, tofile='b/'+p)) for p in paths)
        (directory / (variant + '.patch')).write_text(patch or '# Unmodified head.\n')
        for p, s in changed.items():
            (CLONE / p).write_text(s)
        binary = SCRATCH / 'probe-grouping'
        subprocess.run(['go', 'build', '-o', str(binary), str(ROOT / 'probes/probe.go')], cwd=CLONE, env=env, check=True)
        modes = ['P1', 'PE-s6'] if group == 'P1' else ['P2', 'PE-s1'] if group == 'P2' else ['M1a', 'GT-s1-sim']
        outputs = []
        for mode in modes:
            settings = dict(env, PROBE_MODE=mode, PROBE_COUNT='64' if group == 'M1a' else '3', PROBE_DELAY_US='1000' if group == 'M1a' else '0')
            r = subprocess.run([str(binary), '-logtostderr=true'], env=settings, capture_output=True, text=True)
            outputs.append(f'mode={mode}; variant={variant}; exit={r.returncode}\n' + r.stdout + '\nSTDERR:\n' + r.stderr)
            if r.returncode:
                raise RuntimeError(r.stderr)
        (directory / ('result-' + variant + '.txt')).write_text('\n'.join(outputs))
        print(group, variant, 'completed', flush=True)
for p, s in original.items():
    (CLONE / p).write_text(s)
