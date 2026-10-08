#!/usr/bin/env python3
"""Pack, publish and retrieve immutable evidence through Git-tracked manifests.

Usage: evidence_store.py {pack,publish,fetch,verify} --help
Selections contain repository, tag, subjects and files (source, path, kind).
Exit 0: verified; 1: evidence violation; 2: I/O or subprocess failure.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import tempfile
from urllib.parse import quote

import check_manifest

SCHEMA = Path(__file__).resolve().parents[1] / 'schema/evidence-storage.schema.json'
ASSET_LIMIT = 2 * 1024 ** 3
PART_LIMIT = 1024 ** 3


class EvidenceError(ValueError):
    pass


def read(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def digest(path):
    result = hashlib.sha256()
    with Path(path).open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest()


def write_new(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(value, indent=2) + '\n'
    if path.exists():
        if path.read_text(encoding='utf-8') != encoded:
            raise EvidenceError(f'refusing to replace {path}')
        return
    with path.open('x', encoding='utf-8') as handle:
        handle.write(encoded)


def logical_path(value):
    if not isinstance(value, str):
        raise EvidenceError('evidence paths must be strings')
    path = PurePosixPath(value)
    if (not value or path.is_absolute() or path.as_posix() != value or '\\' in value
            or any(part in {'..', '.git'} for part in path.parts)
            or path.parts[0] not in {'bench', 'artifacts', 'docs'}):
        raise EvidenceError(f'unsafe evidence path: {value}')
    return path


def confined(root, name):
    path = Path(root) / logical_path(name)
    for current in (path, *path.parents):
        if current.is_symlink():
            raise EvidenceError(f'symlink in evidence path: {current}')
    return path


def validate(manifest, published=False):
    errors = check_manifest.validate(read(SCHEMA), manifest)
    if errors:
        raise EvidenceError('; '.join(errors))
    if not manifest['subjects'] or len(set(manifest['subjects'])) != len(manifest['subjects']):
        raise EvidenceError('subjects must be nonempty and distinct')
    if published and not manifest['packages']:
        raise EvidenceError('published manifest contains no packages')
    paths, packages = set(), set()
    for package in manifest['packages']:
        if package['sha256'] in packages or package['bytes'] >= ASSET_LIMIT:
            raise EvidenceError('duplicate package or package exceeds release asset limit')
        packages.add(package['sha256'])
        if package['asset'] != package['sha256'] + '.tar.gz':
            raise EvidenceError('asset name must identify the archive bytes')
        if published and package['publication'] is None:
            raise EvidenceError('package has no publication receipt')
        if package['publication'] and package['publication']['sha256'] != package['sha256']:
            raise EvidenceError('publication receipt names different bytes')
        if not package['members']:
            raise EvidenceError('empty evidence package')
        if sum(member['bytes'] for member in package['members']) >= ASSET_LIMIT:
            raise EvidenceError('unpacked package exceeds size limit')
        for member in package['members']:
            logical_path(member['path'])
            if member['path'] in paths or not 0 <= member['mode'] <= 0o777:
                raise EvidenceError('duplicate member or unsafe permission bits')
            paths.add(member['path'])
    return manifest


def checked_file(path, member):
    path = Path(path)
    if path.is_symlink() or not path.is_file() or path.stat().st_size != member['bytes'] or digest(path) != member['sha256']:
        raise EvidenceError(f'missing or changed evidence: {path}')
    if 'mode' in member and path.stat().st_mode & 0o777 != member['mode']:
        raise EvidenceError(f'changed evidence permissions: {path}')


def unpack(archive, package, destination):
    checked_file(archive, package)
    expected = {item['path']: item for item in package['members']}
    seen = set()
    with tarfile.open(archive, 'r:gz') as bundle:
        for item in bundle:
            if item.name in seen or item.name not in expected or not item.isfile():
                raise EvidenceError(f'unexpected archive member: {item.name}')
            member = expected[item.name]
            if item.size != member['bytes'] or item.mode != member['mode']:
                raise EvidenceError(f'archive metadata differs: {item.name}')
            path = confined(destination, item.name)
            path.parent.mkdir(parents=True, exist_ok=True)
            with bundle.extractfile(item) as source, path.open('xb') as target:
                shutil.copyfileobj(source, target, 1024 * 1024)
            path.chmod(member['mode'])
            checked_file(path, member)
            seen.add(item.name)
    if seen != expected.keys():
        raise EvidenceError('archive is missing indexed members')


def pack(selection, output, part_limit=PART_LIMIT):
    if not isinstance(selection, dict) or set(selection) != {'repository', 'tag', 'subjects', 'files'} or not isinstance(selection['files'], list):
        raise EvidenceError('selection requires repository, tag, subjects and files')
    output = Path(output)
    if output.exists():
        raise EvidenceError('pack output must be a new directory')
    if not 0 < part_limit < ASSET_LIMIT:
        raise EvidenceError('part size must be positive and below 2 GiB')
    files, paths = [], set()
    for item in selection['files']:
        if (not isinstance(item, dict) or set(item) != {'source', 'path', 'kind'}
                or not all(isinstance(value, str) for value in item.values())
                or item['kind'] not in {'evidence', 'reproducibility'}):
            raise EvidenceError('each selection requires source, path and evidence/reproducibility kind')
        logical_path(item['path'])
        source = Path(item['source']).absolute()
        if source.name in {'auth.json', '.credentials.json'}:
            raise EvidenceError(f'authentication must not be published: {source}')
        if any(path.is_symlink() for path in (source, *source.parents)) or not source.is_file():
            raise EvidenceError(f'input must be a regular unlinked file: {source}')
        if item['path'] in paths or source.stat().st_size > part_limit:
            raise EvidenceError('duplicate logical path or input exceeds part size')
        paths.add(item['path'])
        files.append((source, {'path': item['path'], 'sha256': digest(source), 'bytes': source.stat().st_size,
                               'mode': source.stat().st_mode & 0o777, 'kind': item['kind']}))
    if not files:
        raise EvidenceError('selection contains no evidence')
    manifest = {'schema_version': 1, 'repository': selection['repository'], 'tag': selection['tag'],
                'subjects': selection['subjects'], 'packages': []}
    validate(manifest)
    output.mkdir(parents=True)
    parts = [[]]
    size = 0
    for source, member in sorted(files, key=lambda item: item[1]['path']):
        if parts[-1] and size + member['bytes'] > part_limit:
            parts.append([])
            size = 0
        parts[-1].append((source, member))
        size += member['bytes']
    for part in parts:
        with tempfile.TemporaryDirectory(dir=output) as temp:
            archive = Path(temp) / 'package.tar.gz'
            with archive.open('wb') as raw, gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode='w') as bundle:
                    for source, member in part:
                        checked_file(source, member)
                        info = tarfile.TarInfo(member['path'])
                        info.size, info.mode = member['bytes'], member['mode']
                        with source.open('rb') as handle:
                            bundle.addfile(info, handle)
            sha = digest(archive)
            package = {'asset': sha + '.tar.gz', 'sha256': sha, 'bytes': archive.stat().st_size,
                       'members': [member for _, member in part], 'publication': None}
            unpack(archive, package, Path(temp) / 'restored')
            archive.rename(output / package['asset'])
            manifest['packages'].append(package)
    validate(manifest)
    write_new(output / 'manifest.json', manifest)
    return manifest


class GitHub:
    def api(self, route):
        return json.loads(subprocess.check_output(['gh', 'api', route], text=True, encoding='utf-8'))

    def release(self, repository, tag):
        release = self.api(f'repos/{repository}/releases/tags/{quote(tag, safe="")}')
        pages = json.loads(subprocess.check_output(
            ['gh', 'api', '--paginate', '--slurp', f'repos/{repository}/releases/{release["id"]}/assets?per_page=100'],
            text=True, encoding='utf-8'))
        release['assets'] = [asset for page in pages for asset in page]
        return release

    def upload(self, repository, tag, archive):
        subprocess.run(['gh', 'release', 'upload', tag, str(archive), '--repo', repository], check=True)

    def download(self, repository, asset_id, destination, limit):
        command = ['gh', 'api', f'repos/{repository}/releases/assets/{asset_id}', '-H', 'Accept: application/octet-stream']
        with subprocess.Popen(command, stdout=subprocess.PIPE) as process:
            try:
                with Path(destination).open('xb') as target:
                    remaining = limit
                    while True:
                        chunk = process.stdout.read(min(1024 * 1024, remaining + 1))
                        if not chunk:
                            break
                        remaining -= len(chunk)
                        if remaining < 0:
                            raise EvidenceError('download exceeds pinned archive size')
                        target.write(chunk)
                if process.wait():
                    raise subprocess.CalledProcessError(process.returncode, command)
            finally:
                if process.poll() is None:
                    process.kill()


def publish(manifest_path, destination, remote=None):
    manifest_path = Path(manifest_path)
    manifest = validate(read(manifest_path))
    remote = remote or GitHub()
    if Path(destination).exists():
        existing = validate(read(destination), published=True)
        if ({k: v for k, v in existing.items() if k != 'packages'} != {k: v for k, v in manifest.items() if k != 'packages'}
                or [{k: v for k, v in p.items() if k != 'publication'} for p in existing['packages']]
                != [{k: v for k, v in p.items() if k != 'publication'} for p in manifest['packages']]):
            raise EvidenceError('refusing to replace a published manifest')
        verify_remote(existing, remote)
        return existing
    for package in manifest['packages']:
        archive = manifest_path.parent / package['asset']
        checked_file(archive, package)
        release = remote.release(manifest['repository'], manifest['tag'])
        if release.get('draft'):
            raise EvidenceError('draft releases are not shared evidence storage')
        assets = [asset for asset in release['assets'] if asset['name'] == package['asset']]
        if not assets:
            if len(release['assets']) >= 1000:
                raise EvidenceError('release is full; select another evidence release')
            try:
                remote.upload(manifest['repository'], manifest['tag'], archive)
            except subprocess.CalledProcessError:
                # An upload response can be lost after the server committed the asset.
                release = remote.release(manifest['repository'], manifest['tag'])
                if not any(a['name'] == package['asset'] for a in release['assets']):
                    raise
            release = remote.release(manifest['repository'], manifest['tag'])
            assets = [asset for asset in release['assets'] if asset['name'] == package['asset']]
        if len(assets) != 1 or assets[0]['size'] != package['bytes']:
            raise EvidenceError('release asset identity or size differs')
        asset = assets[0]
        package['publication'] = {'release_id': release['id'], 'asset_id': asset['id'], 'sha256': package['sha256'],
                                  'verified_at': datetime.now(timezone.utc).isoformat()}
        verify_remote({**manifest, 'packages': [package]}, remote)
    validate(manifest, published=True)
    write_new(destination, manifest)
    return manifest


def download_package(manifest, package, archive, remote):
    release = remote.release(manifest['repository'], manifest['tag'])
    if release.get('draft'):
        raise EvidenceError('published release became a draft')
    publication = package['publication']
    if release['id'] != publication['release_id'] or not any(
            a['id'] == publication['asset_id'] and a['name'] == package['asset'] and a['size'] == package['bytes']
            for a in release['assets']):
        raise EvidenceError('published release or asset identity changed')
    remote.download(manifest['repository'], publication['asset_id'], archive, package['bytes'])
    checked_file(archive, package)


def verify_remote(manifest, remote=None):
    validate(manifest, published=True)
    remote = remote or GitHub()
    for package in manifest['packages']:
        with tempfile.TemporaryDirectory(prefix='evidence-verify-') as temp:
            archive = Path(temp) / package['asset']
            download_package(manifest, package, archive, remote)
            unpack(archive, package, Path(temp) / 'restored')


def restoration_receipt(manifest_path, destination, remote=None):
    manifest = validate(read(manifest_path), published=True)
    verify_remote(manifest, remote)
    receipt = {'schema_version': 1, 'status': 'verified', 'manifest_sha256': digest(manifest_path),
               'at': datetime.now(timezone.utc).isoformat(), 'subjects': manifest['subjects'],
               'packages': [p['sha256'] for p in manifest['packages']],
               'method': 'empty temporary directory; remote identity, archive hash, member bytes and permissions'}
    write_new(destination, receipt)
    return receipt


def materialize(manifest, root, paths=None, offline=False, remote=None):
    validate(manifest, published=True)
    root, remote = Path(root), remote or GitHub()
    requested = set(paths) if paths is not None else {m['path'] for p in manifest['packages'] for m in p['members']}
    indexed = {m['path'] for p in manifest['packages'] for m in p['members']}
    if not requested <= indexed:
        raise EvidenceError(f'paths absent from manifest: {sorted(requested - indexed)}')
    for package in manifest['packages']:
        members = [m for m in package['members'] if m['path'] in requested]
        if not members:
            continue
        missing = []
        for member in members:
            path = confined(root, member['path'])
            if path.exists():
                checked_file(path, member)
            else:
                missing.append(member)
        if not missing:
            continue
        cache = root / '.cache/evidence'
        if any(p.is_symlink() for p in (cache, *cache.parents)):
            raise EvidenceError('evidence cache is symlinked')
        cache.mkdir(parents=True, exist_ok=True)
        archive = cache / package['asset']
        with tempfile.TemporaryDirectory(dir=cache) as temp:
            temp = Path(temp)
            if archive.exists():
                checked_file(archive, package)
            else:
                if offline:
                    raise EvidenceError(f'archive unavailable offline: {package["asset"]}')
                downloaded = temp / package['asset']
                download_package(manifest, package, downloaded, remote)
                unpack(downloaded, package, temp / 'download-check')
                try:
                    os.link(downloaded, archive)
                except FileExistsError:
                    checked_file(archive, package)
            restored = temp / 'restored'
            unpack(archive, package, restored)
            for member in missing:
                path = confined(root, member['path'])
                path.parent.mkdir(parents=True, exist_ok=True)
                try:
                    os.link(restored / member['path'], path)
                except FileExistsError:
                    checked_file(path, member)
    return [confined(root, path) for path in sorted(requested)]


def manifests(root):
    return [(path, validate(read(path), published=True))
            for path in sorted((Path(root) / 'bench/evidence/manifests').glob('*.json'))]


def resolve(root, name, sha256=None, offline=False):
    root = Path(root)
    path = confined(root, name)
    if path.is_file():
        if sha256 is not None and digest(path) != sha256:
            raise EvidenceError(f'changed evidence: {name}')
        return path
    matches = [(manifest, member) for _, manifest in manifests(root) for package in manifest['packages']
               for member in package['members'] if member['path'] == name]
    if len({member['sha256'] for _, member in matches}) > 1:
        raise EvidenceError(f'conflicting evidence identities: {name}')
    if matches:
        manifest, member = matches[0]
        if sha256 is not None and sha256 != member['sha256']:
            raise EvidenceError(f'locator conflicts with frozen hash: {name}')
        materialize(manifest, root, [name], offline=offline)
    if not path.is_file() or (sha256 is not None and digest(path) != sha256):
        raise EvidenceError(f'missing or changed evidence: {name}')
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    packing = commands.add_parser('pack')
    packing.add_argument('--selection', type=Path, required=True)
    packing.add_argument('--out', type=Path, required=True)
    packing.add_argument('--part-bytes', type=int, default=PART_LIMIT)
    publishing = commands.add_parser('publish')
    publishing.add_argument('--manifest', type=Path, required=True)
    publishing.add_argument('--out', type=Path, required=True)
    fetching = commands.add_parser('fetch')
    fetching.add_argument('--manifest', type=Path, required=True)
    fetching.add_argument('--root', type=Path, default=Path.cwd())
    fetching.add_argument('--path', action='append')
    fetching.add_argument('--offline', action='store_true')
    verifying = commands.add_parser('verify')
    verifying.add_argument('--manifest', type=Path, required=True)
    verifying.add_argument('--receipt', type=Path, help='write a new cold restoration receipt')
    args = parser.parse_args()
    try:
        if args.command == 'pack':
            pack(read(args.selection), args.out, args.part_bytes)
        elif args.command == 'publish':
            publish(args.manifest, args.out)
        elif args.command == 'fetch':
            materialize(read(args.manifest), args.root, args.path, args.offline)
        elif args.receipt:
            restoration_receipt(args.manifest, args.receipt)
        else:
            verify_remote(read(args.manifest))
        print(f'{args.command}: verified')
        return 0
    except (ValueError, tarfile.TarError) as error:
        print(f'evidence: {error}')
        return 1
    except (OSError, subprocess.SubprocessError) as error:
        print(f'evidence: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
