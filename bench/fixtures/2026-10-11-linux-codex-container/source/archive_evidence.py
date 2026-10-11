import hashlib
import json
import os
from pathlib import Path
import shutil
import tarfile

REPO = Path('/home/jack/.t3/worktrees/code-review-bench/t3code-42306bc1')
OUT = REPO / 'bench/fixtures/2026-10-11-linux-codex-container'
OUT.mkdir(parents=True, exist_ok=True)

def digest(data):
    return hashlib.sha256(data).hexdigest()

def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')

def archive(label, root):
    paths = []
    for p in sorted(root.rglob('*')):
        rel = p.relative_to(root)
        if rel.parts[0] == 'rootfs':
            if str(rel) not in ('rootfs/codex-probe.py', 'rootfs/clean_context.py',
                                'rootfs/claude_container_probe.py', 'rootfs/claude_container_resources.py',
                                'rootfs/usr/bin/git'):
                continue
            if p.is_file() and p.read_bytes()[:2] != b'#!' and str(rel) == 'rootfs/usr/bin/git':
                continue
        if rel.name == 'rootfs.tar' or rel.name in ('auth.json', '.credentials.json'):
            continue
        if p.is_file() and not p.is_symlink() or p.is_symlink():
            paths.append((p, str(rel)))
    for suffix, name in (('.log', 'controller-stdout.txt'), ('.stderr', 'controller-stderr.txt')):
        p = Path(str(root) + suffix)
        if p.exists():
            paths.append((p, name))
    target = OUT / (label + '.tar.xz')
    artifacts = []
    with tarfile.open(target, 'w:xz', preset=6) as tar:
        for source, name in paths:
            if source.is_symlink():
                artifacts.append({'path': name, 'kind': 'symlink', 'target': os.readlink(source)})
            else:
                data = source.read_bytes()
                artifacts.append({'path': name, 'sha256': digest(data), 'bytes': len(data),
                                  'mode': source.stat().st_mode & 0o777})
            tar.add(source, arcname=label + '/' + name, recursive=False)
    with tarfile.open(target, 'r:xz') as tar:
        for item in artifacts:
            member = tar.getmember(label + '/' + item['path'])
            if item.get('kind') == 'symlink':
                assert member.issym() and member.linkname == item['target']
            else:
                assert digest(tar.extractfile(member).read()) == item['sha256']
                assert member.mode & 0o777 == item['mode']
    record = {'label': label, 'original': str(root), 'archive': target.name,
              'sha256': digest(target.read_bytes()), 'verified': True, 'artifacts': artifacts}
    save(OUT / (label + '-archive.json'), record)
    if (root / 'receipt.json').exists():
        shutil.copyfile(root / 'receipt.json', OUT / (label + '-receipt.json'))
    return {'label': label, 'original': str(root), 'archive': target.name,
            'sha256': record['sha256'], 'verified': True,
            'artifact_manifest': label + '-archive.json',
            'artifact_manifest_sha256': digest((OUT / (label + '-archive.json')).read_bytes()),
            'artifact_count': len(artifacts)}

entries = []
for i in range(1,21):
    root = Path(f'/tmp/issue37-native-{i:03}')
    entries.append(archive(f'attempt-{i:03}', root))
    print(entries[-1]['label'], entries[-1]['artifact_count'], flush=True)
for label, path in [('resource-probe-failed', '/tmp/issue37-resource-probe'),
                    ('resource-probe-passing', '/tmp/issue37-resource-probe-2'),
                    ('prerequisite-investigation', '/tmp/issue37-prerequisite-307ercul')]:
    entries.append(archive(label, Path(path)))
source = OUT / 'source'
source.mkdir(exist_ok=True)
runtime = {}
for name in ('codex_container_probe.py', 'codex_fixture_git.py', 'clean_context.py',
             'claude_container_probe.py', 'claude_container_resources.py'):
    shutil.copyfile(REPO / 'bench/tools' / name, source / name)
    runtime[name] = digest((source / name).read_bytes())
shutil.copyfile(REPO / 'bench/policies/empty-harness-v1.md', source / 'empty-harness-v1.md')
runtime['empty-harness-v1.md'] = digest((source / 'empty-harness-v1.md').read_bytes())
save(OUT / 'manifest.json', {'policy': 'codex-container-fixture-v1', 'paid_calls': 0,
    'accepted_attempt': 'attempt-020', 'runtime_sha256': runtime, 'entries': entries,
    'publication_plan': {'repository': 'kamui/code-review-bench', 'tag': 'evidence-codex-container-2026-10-11-v1',
       'visibility': 'public', 'categories': ['inputs', 'execution', 'accounting and lineage', 'unique artifacts', 'integrity'],
       'authorization': 'Issue #37 implementation and publication authorized by the repository owner on 2026-10-11'},
    'limitations': ['Dummy provider only; no production admission or live subscription claim.',
       'Rebuildable native binaries/rootfs are excluded and pinned by receipts; original workspaces remain retained.',
       'Prototype worker rows before attempt-013 are invalid as worker evidence; retained as diagnostics.']})
