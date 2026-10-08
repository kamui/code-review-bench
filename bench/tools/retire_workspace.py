#!/usr/bin/env python3
"""Retire fully captured, closed execution storage during an operator maintenance window.

Usage: retire_workspace.py --plan PLAN --shared-commit SHA --shared-branch BRANCH
       --receipt NEW_PATH [--apply]
Plans and cold-restoration receipts must already be published in the canonical repository.
Default is dry-run. Existing clone/cache pruning remains the preferred first step.
Exit 0: eligible/applied; 1: refused; 2: I/O/subprocess failure. Linux /proc is required.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import fcntl
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from urllib.parse import quote

import evidence_inventory as inventory
import evidence_store as store

ROOT = Path(__file__).resolve().parents[2]


def shared_file(root, name, repository, commit, remote):
    path = store.confined(root, name)
    response = remote.api(f'repos/{repository}/contents/{quote(name, safe="/")}?ref={commit}')
    _, blob = inventory.identities(path, path.stat().st_size)
    if response.get('type') != 'file' or response.get('sha') != blob or response.get('size') != path.stat().st_size:
        raise store.EvidenceError(f'file is not published at {commit}: {name}')
    return path


def active_users(workspace):
    proc = Path('/proc')
    if not (proc / 'self/fd').is_dir():
        raise store.EvidenceError('cannot establish inactivity without Linux /proc')
    active = []
    for process in proc.iterdir():
        if not process.name.isdigit() or int(process.name) == os.getpid():
            continue
        try:
            if process.stat().st_uid != os.getuid():
                continue
            links = [process / 'cwd', *list((process / 'fd').iterdir())]
            for link in links:
                try:
                    target = Path(os.readlink(link))
                except FileNotFoundError:
                    continue
                if target.is_absolute() and target.is_relative_to(workspace):
                    active.append(int(process.name))
                    break
            command = (process / 'cmdline').read_bytes()
            if os.fsencode(workspace) in command:
                active.append(int(process.name))
        except (FileNotFoundError, ProcessLookupError):
            continue
        except PermissionError as error:
            raise store.EvidenceError(f'cannot inspect process {process.name}') from error
    return sorted(set(active))


def accounting_dependencies(workspace, roots):
    reservations = set()
    for root in roots:
        if not root.is_dir():
            raise store.EvidenceError(f'accounting root missing: {root}')
        reservations.update(root.glob('batches/*/*/attempt-*/reservation.json'))
        for directory, dirs, files in os.walk(root, followlinks=False):
            dirs[:] = [name for name in dirs if name not in {'.git', 'node_modules', 'clone', 'clone-cache'}]
            if 'reservation.json' in files:
                reservations.add(Path(directory) / 'reservation.json')
    blockers = []
    for reservation in sorted(reservations):
        work = (reservation.parent / 'work').resolve()
        if work.is_relative_to(workspace) or workspace.is_relative_to(work):
            blockers.append(str(reservation))
    return blockers


def capture_check(snapshot, current, manifest, prefix, worktree=False, classifications=None):
    if snapshot != current:
        raise store.EvidenceError('workspace changed since the complete inventory was captured')
    members = {m['path']: m for p in manifest['packages'] for m in p['members']}
    for entry in current['entries']:
        name = entry['path']
        if worktree and (entry['class'] == 'tracked-identical' or name == '.git'):
            continue
        if entry['type'] != 'file':
            raise store.EvidenceError(f'uncaptured non-file blocks retirement: {name}')
        if entry['class'] == 'unknown':
            decision = (classifications or {}).get(name, {})
            if (decision.get('class') not in {'local-evidence', 'reproducibility'} or not decision.get('reason')
                    or decision.get('sha256') != entry['sha256']):
                raise store.EvidenceError(f'unknown file blocks retirement: {name}')
        if entry['class'] == 'account-state':
            continue
        member = members.get(prefix + '/' + name)
        if not member or any(member[key] != entry[key] for key in ('sha256', 'bytes', 'mode')):
            raise store.EvidenceError(f'uncaptured file blocks retirement: {name}')


def eligibility(root, plan_name, commit, branch, remote):
    plan = store.read(store.confined(root, plan_name))
    errors = store.check_manifest.validate(store.read(ROOT / 'bench/schema/evidence-retirement.schema.json'), plan)
    if errors:
        raise store.EvidenceError('invalid closure: ' + '; '.join(errors))
    if (plan.get('schema_version') != 1 or plan.get('kind') not in {'review', 'grading', 'worktree'}
            or plan.get('investigation') != 'closed' or not plan.get('closure_reason')
            or not plan.get('closed_by') or plan.get('maintenance_window') is not True):
        raise store.EvidenceError('requires a saved investigation closure and an exclusive operator maintenance window')
    if not re.fullmatch('[0-9a-f]{40}', commit) or not branch:
        raise store.EvidenceError('pin a pushed commit and its shared branch')
    manifest = store.validate(store.read(store.confined(root, plan['manifest'])), published=True)
    repository = manifest['repository']
    comparison = remote.api(f'repos/{repository}/compare/{commit}...{quote(branch, safe="")}')
    if comparison.get('status') not in {'ahead', 'identical'}:
        raise store.EvidenceError('commit is not reachable from the shared branch')
    for name in (plan_name, plan['manifest'], plan['inventory'], plan['restoration_receipt']):
        shared_file(root, name, repository, commit, remote)
    receipt = store.read(store.confined(root, plan['restoration_receipt']))
    if receipt.get('manifest_sha256') != store.digest(store.confined(root, plan['manifest'])) or receipt.get('status') != 'verified':
        raise store.EvidenceError('cold restoration receipt does not match the published manifest')
    required = {'inputs', 'execution', 'accounting', 'diagnostics', 'disposition', 'lineage'}
    if plan['kind'] == 'grading':
        required.add('grading')
    if not required <= plan.get('evidence', {}).keys():
        raise store.EvidenceError('closure omits required evidence categories')
    for category in required:
        references = plan['evidence'][category]
        if not references:
            raise store.EvidenceError(f'closure lacks {category} evidence')
        for ref in references:
            path = shared_file(root, ref['path'], repository, commit, remote)
            if store.digest(path) != ref['sha256']:
                raise store.EvidenceError(f'closure evidence changed: {ref["path"]}')
    workspace = Path(plan['workspace']).absolute()
    if (workspace == root or root.is_relative_to(workspace) or workspace == Path.home()
            or len(workspace.parts) < 4 or any(p.is_symlink() for p in (workspace, *workspace.parents))):
        raise store.EvidenceError('unsafe workspace root')
    snapshot = store.read(store.confined(root, plan['inventory']))
    if snapshot.get('root') != str(workspace):
        raise store.EvidenceError('inventory names another workspace')
    current = inventory.inventory(workspace, hash_all=True)
    if (workspace / '.git').exists() and not current['head']:
        raise store.EvidenceError('workspace Git identity cannot be verified')
    if current['missing_tracked'] or any(e['class'] == 'development' for e in current['entries']):
        raise store.EvidenceError('modified development work blocks retirement')
    if plan['kind'] == 'worktree':
        if not current['head'] or inventory.git(workspace, 'status', '--porcelain', '--untracked-files=all'):
            raise store.EvidenceError('worktree has uncommitted work')
        common = Path(inventory.git(workspace, 'rev-parse', '--git-common-dir'))
        if not common.is_absolute():
            common = workspace / common
        if common.resolve().is_relative_to(workspace):
            raise store.EvidenceError('only linked worktrees can be retired')
        comparison = remote.api(f'repos/{repository}/compare/{current["head"]}...{quote(branch, safe="")}')
        if comparison.get('status') not in {'ahead', 'identical'}:
            raise store.EvidenceError('worktree has unpushed commits')
    elif current['head']:
        raise store.EvidenceError('Git worktrees require kind worktree')
    for clone in workspace.rglob('.git'):
        if clone.parent == workspace:
            continue
        if inventory.git(clone.parent, 'status', '--porcelain', '--untracked-files=all'):
            raise store.EvidenceError(f'modified nested clone: {clone.parent}')
    prefix = store.logical_path(plan['capture_prefix']).as_posix()
    capture_check(snapshot, current, manifest, prefix, plan['kind'] == 'worktree', plan.get('classifications'))
    roots = [root, *[Path(path).absolute() for path in plan.get('accounting_roots', [])]]
    if not plan.get('accounting_roots'):
        raise store.EvidenceError('inventory all controlling accounting queues explicitly')
    blockers = accounting_dependencies(workspace, roots)
    if blockers:
        raise store.EvidenceError(f'workspace-dependent accounting: {blockers}')
    pids = active_users(workspace)
    if pids:
        raise store.EvidenceError(f'workspace is active: {pids}')
    store.verify_remote(manifest, remote)
    if inventory.inventory(workspace, hash_all=True) != snapshot or active_users(workspace):
        raise store.EvidenceError('workspace changed or became active during verification')
    return plan, workspace, manifest, snapshot


def retire(root, plan_name, commit, branch, receipt_path, apply=False, remote=None):
    root, receipt_path = Path(root).resolve(), Path(receipt_path).absolute()
    plan, workspace, manifest, snapshot = eligibility(root, plan_name, commit, branch, remote or store.GitHub())
    if receipt_path.is_relative_to(workspace) or receipt_path.exists():
        raise store.EvidenceError('receipt must be a new path outside the workspace')
    result = {'schema_version': 1, 'workspace': str(workspace), 'shared_commit': commit,
              'plan_sha256': store.digest(store.confined(root, plan_name)), 'applied': False,
              'packages': [p['sha256'] for p in manifest['packages']], 'blocked_paths': [],
              'at': datetime.now(timezone.utc).isoformat(),
              'inventoried_allocated_bytes': sum(e['allocated_bytes'] for e in snapshot['entries']),
              'free_bytes_before': shutil.disk_usage(workspace).free}
    if apply:
        # The operator maintenance window covers clients that do not share the runner lock.
        lock_path = workspace.parent / '.evidence-retirement.lock'
        with lock_path.open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            if inventory.inventory(workspace, hash_all=True) != snapshot or active_users(workspace):
                raise store.EvidenceError('workspace changed before removal')
            store.write_new(receipt_path, {**result, 'state': 'removal-started'})
            if plan['kind'] == 'worktree':
                subprocess.run(['git', '-C', str(root), 'worktree', 'remove', '--force', str(workspace)], check=True)
            else:
                shutil.rmtree(workspace)
            result['applied'] = True
        result['free_bytes_after'] = shutil.disk_usage(workspace.parent).free
        result['free_bytes_delta'] = result['free_bytes_after'] - result['free_bytes_before']
        store.write_new(receipt_path.with_name(receipt_path.name + '.completed.json'), result)
    else:
        store.write_new(receipt_path, result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--plan', required=True, help='repository-relative published closure plan')
    parser.add_argument('--shared-commit', required=True)
    parser.add_argument('--shared-branch', required=True)
    parser.add_argument('--receipt', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    try:
        print(json.dumps(retire(args.root, args.plan, args.shared_commit, args.shared_branch, args.receipt, args.apply), indent=2))
        return 0
    except (ValueError, KeyError) as error:
        print(f'retirement refused: {error}')
        return 1
    except (OSError, subprocess.SubprocessError) as error:
        print(f'retirement: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
