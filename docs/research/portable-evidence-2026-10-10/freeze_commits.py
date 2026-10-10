#!/usr/bin/env python3
"""List the refs that keep each run's freeze commit retrievable from the two forge repositories.

Usage: freeze_commits.py --git-dir DIR [--bundle FILE] [--check]. DIR is a scratch bare repository
outside any checkout; a missing one is created and filled with every branch, tag and pull request
head of both repositories. FILE is the member of the history-recovery manifest, already fetched.
Exit 0: the record was written, or --check reproduced it; 1: the saved record differs.
"""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
RECORD = Path(__file__).with_name('freeze-commits.v1.json')
BUNDLE = 'artifacts/recovery/shared-before-rewrite-2026-10-08.bundle'
KINDS = {'heads': 'branches', 'tags': 'tags', 'pull': 'pull_requests'}


def read(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def repositories():
    canonical = read(ROOT / 'bench/evidence/manifests/last-push-transcripts-2026-10-09-v1.json')['repository']
    return [canonical, read(ROOT / 'bench/import-manifest.json')['repository'].removeprefix('https://github.com/')]


def git(directory, *args, check=True):
    done = subprocess.run(['git', '--git-dir', str(directory), *args], capture_output=True, text=True, encoding='utf-8')
    if check and done.returncode:
        raise SystemExit(done.stderr.strip())
    return done


def fetch(directory, bundle):
    if not directory.exists():
        subprocess.run(['git', 'init', '-q', '--bare', str(directory)], check=True)
        for name in repositories():
            git(directory, 'fetch', '-q', f'https://github.com/{name}.git', *(
                f'+refs/{source}:refs/forge/{name}/{kind}/*' for source, kind in
                (('heads/*', 'heads'), ('tags/*', 'tags'), ('pull/*/head', 'pull'))))
    if bundle:
        git(directory, 'fetch', '-q', str(bundle), '+refs/*:refs/bundle/*')


def retained(directory, bundle):
    commits = {}
    for path in sorted((ROOT / 'bench/runs').glob('*/manifest.json')):
        commit = read(path).get('freeze_commit')
        if commit:
            commits.setdefault(commit, []).append(path.parent.name)
    rows = []
    for commit, runs in commits.items():
        present = not git(directory, 'cat-file', '-e', commit + '^{commit}', check=False).returncode
        refs = git(directory, 'for-each-ref', '--contains', commit, '--format=%(refname)').stdout.split() if present else []
        row = {'commit': commit, 'runs': runs, 'refs': {}, 'bundle': None}
        for name in repositories():
            found = {label: [] for label in KINDS.values()}
            for ref in refs:
                if ref.startswith(f'refs/forge/{name}/'):
                    kind, value = ref.removeprefix(f'refs/forge/{name}/').split('/', 1)
                    found[KINDS[kind]].append(int(value) if kind == 'pull' else value)
            found['pull_requests'] = min(found['pull_requests'], default=None)
            if any(found.values()):
                row['refs'][name] = {'branches': sorted(found['branches']), 'tags': sorted(found['tags']),
                                     'lowest_pull_request': found['pull_requests']}
        if bundle:
            row['bundle'] = sorted(ref.removeprefix('refs/bundle/') for ref in refs if ref.startswith('refs/bundle/heads/'))[:1] or None
        row['manifests_at_commit'] = sum(
            not git(directory, 'cat-file', '-e', f'{commit}:bench/runs/{run}/manifest.json', check=False).returncode
            for run in runs) if present else 0
        rows.append(row)
    return {'schema_version': 1, 'repositories': repositories(), 'bundle': BUNDLE if bundle else None,
            'runs_without_freeze_commit': sorted(path.parent.name for path in (ROOT / 'bench/runs').glob('*/manifest.json')
                                                 if not read(path).get('freeze_commit')),
            'commits': rows,
            'without_forge_ref': [row['commit'] for row in rows if not row['refs']]}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--git-dir', type=Path, required=True)
    parser.add_argument('--bundle', type=Path)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    if args.git_dir.resolve().is_relative_to(ROOT):
        raise SystemExit('--git-dir must stay outside this checkout')
    fetch(args.git_dir, args.bundle)
    encoded = json.dumps(retained(args.git_dir, args.bundle), indent=2) + '\n'
    if args.check:
        if RECORD.read_text(encoding='utf-8') != encoded:
            raise SystemExit('freeze commit refs differ from the saved record')
        print('freeze commits: reproduced')
    else:
        RECORD.write_text(encoded, encoding='utf-8')
        print(f'wrote {RECORD.relative_to(ROOT)}')
