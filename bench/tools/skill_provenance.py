#!/usr/bin/env python3
"""Record a skill's release metadata and verified runtime source before freezing a run."""

import argparse
from datetime import datetime
import fnmatch
import hashlib
import json
from pathlib import Path
import re
import subprocess


def git(repository, *arguments):
    return subprocess.check_output(['git', '-C', str(repository), *arguments])


def runtime_file(path, exclusions=(), inclusions=()):
    if any(fnmatch.fnmatchcase(path, pattern) for pattern in inclusions):
        return True
    parts = Path(path).parts
    name = parts[-1].lower()
    return not (any(part.lower() in {'tests', 'test', 'history', 'licenses', '__pycache__'} for part in parts)
                or name in {'design.md', 'changelog.md', 'history.md', 'readme.md', 'license', 'license.md',
                            'third_party_notices.md'}
                or name.startswith('test_') or '.test.' in name or '.spec.' in name
                or any(fnmatch.fnmatchcase(path, pattern) for pattern in exclusions))


def source_files(repository, revision, skill_path, exclusions=(), inclusions=()):
    prefix = skill_path.rstrip('/') + '/'
    paths = git(repository, 'ls-tree', '-r', '--name-only', revision, '--', skill_path).decode().splitlines()
    return {path.removeprefix(prefix): git(repository, 'show', f'{revision}:{path}')
            for path in paths if runtime_file(path.removeprefix(prefix), exclusions, inclusions)}


def collect(repository, revision, skill_path, *, frozen=None, benchmark_tree=None, release=None, exclusions=(), inclusions=()):
    if git(repository, 'rev-parse', '--is-shallow-repository').strip() == b'true':
        raise ValueError('fetch complete history before collecting skill provenance')
    commit = git(repository, 'rev-parse', f'{revision}^{{commit}}').decode().strip()
    tree = git(repository, 'rev-parse', f'{commit}:{skill_path}').decode().strip()
    files = source_files(repository, commit, skill_path, exclusions, inclusions)
    if 'SKILL.md' not in files:
        raise ValueError('skill source must contain SKILL.md')
    if frozen:
        frozen_files = {path.relative_to(frozen).as_posix(): path.read_bytes() for path in frozen.rglob('*')
                        if path.is_file() and runtime_file(path.relative_to(frozen).as_posix(), exclusions, inclusions)}
        mismatches = sorted(path for path in files.keys() | frozen_files.keys() if files.get(path) != frozen_files.get(path))
        if mismatches:
            raise ValueError('frozen runtime differs from upstream: ' + ', '.join(mismatches))
    prefix = skill_path.rstrip('/') + '/'
    history = git(repository, 'log', '--diff-merges=first-parent', '--format=commit %H %cI', '--name-only', commit,
                  '--', skill_path).decode().splitlines()
    latest = None
    for line in history:
        if line.startswith('commit '):
            _, changed_commit, timestamp = line.split()
        elif line.startswith(prefix) and runtime_file(line.removeprefix(prefix), exclusions, inclusions):
            candidate = (datetime.fromisoformat(timestamp), changed_commit, timestamp)
            if latest is None or candidate[0] > latest[0]:
                latest = candidate
    if latest is None:
        raise ValueError('no runtime-file commit found; fetch the complete skill history')
    frontmatter = files['SKILL.md'].decode().split('---', 2)
    declared = re.search(r'^version:[ \t]*[\'"]?([^ \t\r\n\'"#]+)', frontmatter[1], re.MULTILINE) if len(frontmatter) == 3 and not frontmatter[0].strip() else None
    version = declared.group(1) if declared and declared.group(1).lower() not in {'null', '~'} else None
    release_metadata = None
    if release:
        timestamp = release.get('published_at')
        tag = release['tag_name']
        if release.get('draft') or not timestamp or datetime.fromisoformat(timestamp.replace('Z', '+00:00')).tzinfo is None:
            raise ValueError('release must have a published timestamp with a timezone')
        if source_files(repository, tag, skill_path, exclusions, inclusions) != files:
            raise ValueError('published release runtime differs from the pinned skill')
        released_version = re.search(r'(?:^|[-/])v?(\d+\.\d+\.\d+(?:-[\w.]+)?)$', tag)
        if version and released_version and version != released_version.group(1):
            raise ValueError('declared skill version differs from release version')
        version = version or (released_version.group(1) if released_version else None)
        release_metadata = {'tag': tag, 'timestamp': timestamp, 'url': release['html_url']}
    repository_url = git(repository, 'remote', 'get-url', 'origin').decode().strip().removesuffix('.git')
    return {'skill_path': skill_path, 'source_repository': repository_url, 'source_commit': commit,
            'source_tree': tree, 'benchmark_tree': benchmark_tree or tree, 'version': version,
            'release': release_metadata,
            'last_runtime_change': {'commit': latest[1], 'timestamp': latest[2]},
            'date': (release_metadata['timestamp'] if release_metadata else latest[2])[:10],
            'date_source': 'release' if release_metadata else 'commit',
            'runtime_files': [{'path': path, 'sha256': hashlib.sha256(content).hexdigest()}
                              for path, content in sorted(files.items())], 'excluded_patterns': list(exclusions),
            'included_patterns': list(inclusions)}


def label(record):
    return ' · '.join(part for part in (record['version'], record['date']) if part)


def verify_pin(run, arm):
    pin = arm.get('skill_provenance')
    if pin is None:
        return None
    path = (run / pin['path']).resolve()
    if not path.is_relative_to(run.resolve()):
        raise ValueError('skill provenance must be inside the run directory')
    if hashlib.sha256(path.read_bytes()).hexdigest() != pin['sha256']:
        raise ValueError('skill provenance no longer matches its frozen hash')
    record = json.loads(path.read_text())
    if record['benchmark_tree'] != arm['resolved_skill_tree']:
        raise ValueError('skill provenance identifies a different benchmark tree')
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--skill-path', required=True)
    parser.add_argument('--frozen', type=Path)
    parser.add_argument('--benchmark-tree')
    parser.add_argument('--release-json', type=Path, help='Saved GitHub release object for the version being used')
    parser.add_argument('--exclude', action='append', default=[], help='Additional non-runtime file glob')
    parser.add_argument('--include', action='append', default=[], help='Runtime dependency glob overriding exclusions')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    record = collect(args.repository, args.revision, args.skill_path, frozen=args.frozen,
                     benchmark_tree=args.benchmark_tree,
                     release=json.loads(args.release_json.read_text()) if args.release_json else None,
                     exclusions=args.exclude, inclusions=args.include)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open('x') as output:
        output.write(json.dumps(record, indent=2) + '\n')
    print(json.dumps({'path': str(args.out), 'sha256': hashlib.sha256(args.out.read_bytes()).hexdigest(),
                      'label': label(record)}))


if __name__ == '__main__':
    main()
