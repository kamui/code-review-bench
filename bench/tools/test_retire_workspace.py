"""Retirement is a separate closed-investigation operation, never an archive-presence shortcut."""
import hashlib
import copy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import evidence_inventory as inventory
import evidence_store as store
import retire_workspace as retire
import regrade
from test_evidence_store import FakeGitHub


class PublishedGitHub(FakeGitHub):
    def __init__(self, root, checkout):
        super().__init__(root)
        self.checkout = checkout
        self.shared = {}
        self.reachable = True

    def freeze_git(self):
        self.shared = {path.relative_to(self.checkout).as_posix(): path.read_bytes()
                       for path in self.checkout.rglob('*') if path.is_file()}

    def api(self, route):
        if '/compare/' in route:
            return {'status': 'ahead' if self.reachable else 'diverged'}
        name = route.split('/contents/')[1].split('?')[0]
        data = self.shared[name]
        return {'type': 'file', 'sha': hashlib.sha1(f'blob {len(data)}\0'.encode() + data).hexdigest(), 'size': len(data)}


class RetirementTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.repo, self.work, self.queue = [self.base / name for name in ('repo', 'work', 'queue')]
        for path in (self.repo, self.work, self.queue, self.base / 'remote'):
            path.mkdir()
        self.remote = PublishedGitHub(self.base / 'remote', self.repo)
        self.prefix = 'artifacts/workspaces/failed-001'
        (self.work / 'dispatch.json').write_text('{"exit_code":1,"usage":{"high":2.5}}')
        (self.work / 'stderr.log').write_text('failure diagnostics')
        selection = {'repository': 'owner/canonical', 'tag': 'evidence-v1', 'subjects': ['failed-001'], 'files': [
            {'source': str(p), 'path': self.prefix + '/' + p.name, 'kind': 'evidence'} for p in self.work.iterdir()]}
        store.pack(selection, self.base / 'packed')
        self.manifest_name = 'bench/evidence/manifests/test.json'
        store.publish(self.base / 'packed/manifest.json', self.repo / self.manifest_name, self.remote)
        receipt_name = 'bench/evidence/receipts/restored.json'
        store.restoration_receipt(self.repo / self.manifest_name, self.repo / receipt_name, self.remote)
        self.inventory_name = 'bench/evidence/inventories/test.json'
        store.write_new(self.repo / self.inventory_name, inventory.inventory(self.work, hash_all=True))
        proof = self.repo / 'bench/evidence/closure-evidence.json'
        store.write_new(proof, {'disposition': 'failed', 'replacement': 'next-002', 'usage': 2.5, 'reason': 'inspected failure'})
        ref = {'path': proof.relative_to(self.repo).as_posix(), 'sha256': store.digest(proof)}
        self.plan = {'schema_version': 1, 'workspace': str(self.work), 'kind': 'review', 'manifest': self.manifest_name,
                     'inventory': self.inventory_name, 'restoration_receipt': receipt_name, 'capture_prefix': self.prefix,
                     'investigation': 'closed', 'closure_reason': 'failure diagnosed and replacement retained',
                     'closed_by': 'fixture maintainer', 'maintenance_window': True, 'accounting_roots': [str(self.queue)],
                     'evidence': {k: [ref] for k in ('inputs', 'execution', 'accounting', 'diagnostics', 'disposition', 'lineage')}}
        self.plan_name = 'bench/evidence/retirements/test.json'
        self.save_plan()

    def save_plan(self):
        path = self.repo / self.plan_name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.plan))
        self.remote.freeze_git()

    def run_retirement(self, apply=False):
        return retire.retire(self.repo, self.plan_name, 'a'*40, 'main', self.base / 'receipt.json', apply, self.remote)

    def test_dry_run_then_closed_failure_removal_keeps_accounting_and_receipts(self):
        before = self.remote.shared.copy()
        with patch.object(retire, 'active_users', return_value=[]):
            result = self.run_retirement()
            self.assertFalse(result['applied'])
            self.assertTrue(self.work.exists())
            (self.base / 'receipt.json').unlink()
            result = self.run_retirement(apply=True)
        self.assertTrue(result['applied'])
        self.assertFalse(self.work.exists())
        self.assertEqual(before, self.remote.shared)
        self.assertTrue((self.base / 'receipt.json.completed.json').exists())
        self.assertIn('free_bytes_delta', result)

    def test_active_open_investigation_missing_diagnostics_and_unpushed_plan_refuse(self):
        with patch.object(retire, 'active_users', return_value=[123]):
            with self.assertRaisesRegex(store.EvidenceError, 'active'):
                self.run_retirement(apply=True)
        self.plan['investigation'] = 'open'
        self.save_plan()
        with self.assertRaisesRegex(store.EvidenceError, 'closure'):
            self.run_retirement(apply=True)
        self.plan['investigation'] = 'closed'
        del self.plan['evidence']['diagnostics']
        self.save_plan()
        with self.assertRaisesRegex(store.EvidenceError, 'categories'):
            self.run_retirement(apply=True)
        self.assertTrue(self.work.exists())

    def test_snapshot_changes_and_uncaptured_file_refuse(self):
        (self.work / 'stderr.log').write_text('additional diagnostics')
        with self.assertRaisesRegex(store.EvidenceError, 'changed since'):
            self.run_retirement(apply=True)
        snapshot_path = self.repo / self.inventory_name
        snapshot_path.write_text(json.dumps(inventory.inventory(self.work, hash_all=True)))
        self.remote.freeze_git()
        with self.assertRaisesRegex(store.EvidenceError, 'uncaptured'):
            self.run_retirement(apply=True)
        self.assertTrue(self.work.exists())

    def test_published_content_and_asset_replacement_refuse(self):
        self.remote.reachable = False
        with self.assertRaisesRegex(store.EvidenceError, 'reachable'):
            self.run_retirement(apply=True)
        self.remote.reachable = True
        (self.repo / self.plan_name).write_text(json.dumps({**self.plan, 'closure_reason': 'changed locally'}))
        with self.assertRaisesRegex(store.EvidenceError, 'not published'):
            self.run_retirement(apply=True)
        self.save_plan()
        self.remote.assets[0]['id'] += 1
        with patch.object(retire, 'active_users', return_value=[]):
            with self.assertRaisesRegex(store.EvidenceError, 'identity changed'):
                self.run_retirement(apply=True)
        self.assertTrue(self.work.exists())

    def test_resolved_zero_charge_still_depends_on_workspace(self):
        attempt = self.queue / 'batches/run/target/attempt-1'
        attempt.mkdir(parents=True)
        (attempt / 'work').symlink_to(self.work, target_is_directory=True)
        store.write_new(attempt / 'reservation.json', {'maxBudgetUsd': 6})
        store.write_new(attempt / 'budget-resolution.json', {'chargeUpperUsd': 0, 'evidence': []})
        (self.work / 'dispatch.json').unlink()
        self.assertEqual(retire.accounting_dependencies(self.work, [self.queue]), [str(attempt / 'reservation.json')])
        store.write_new(self.work / 'dispatch.json', {'usage': {'high': 2}})
        self.assertEqual(retire.accounting_dependencies(self.work, [self.queue]), [str(attempt / 'reservation.json')])

    def test_priced_ledger_dependency_blocks_removal_and_preserves_settled_charge(self):
        attempt = self.queue / 'batches/run/target/attempt-1'
        attempt.mkdir(parents=True)
        (attempt / 'work').symlink_to(self.work, target_is_directory=True)
        store.write_new(attempt / 'reservation.json', {'maxBudgetUsd': 6})
        before = regrade.ledger(self.queue)
        self.assertEqual(before, (regrade.Decimal('2.5'), []))
        with self.assertRaisesRegex(store.EvidenceError, 'workspace-dependent accounting'):
            self.run_retirement(apply=True)
        self.assertTrue((self.work / 'dispatch.json').exists())
        self.assertEqual(regrade.ledger(self.queue), before)
        self.assertFalse((self.base / 'receipt.json').exists())

    def test_symlinks_at_each_ledger_path_component_cannot_bypass_accounting(self):
        for component in ('queue', 'batches', 'run', 'target', 'attempt-1'):
            with self.subTest(component=component):
                queue = self.base / ('queue-' + component)
                attempt = queue / 'batches/run/target/attempt-1'
                attempt.mkdir(parents=True)
                (attempt / 'work').symlink_to(self.work, target_is_directory=True)
                store.write_new(attempt / 'reservation.json', {'maxBudgetUsd': 6})
                parts = ('batches', 'run', 'target', 'attempt-1')
                linked = queue if component == 'queue' else queue.joinpath(*parts[:parts.index(component)+1])
                moved = self.base / ('linked-' + component)
                linked.rename(moved)
                linked.symlink_to(moved, target_is_directory=True)
                self.plan['accounting_roots'] = [str(queue)]
                self.save_plan()
                before = regrade.ledger(queue)
                self.assertEqual(before, (regrade.Decimal('2.5'), []))
                with self.assertRaisesRegex(store.EvidenceError, 'workspace-dependent accounting'):
                    self.run_retirement(apply=True)
                self.assertTrue((self.work / 'dispatch.json').exists())
                self.assertEqual(regrade.ledger(queue), before)

    def test_external_dispatch_links_preserve_the_priced_ledger(self):
        persistent = self.base / 'persistent-dispatch.json'
        persistent.write_bytes((self.work / 'dispatch.json').read_bytes())
        intermediate = self.base / 'receipt-link'
        intermediate.symlink_to(self.work / '..' / persistent.name)
        parent_alias = self.base / 'parent-alias'
        parent_alias.symlink_to(self.base, target_is_directory=True)
        targets = [self.work / 'dispatch.json', self.work / '..' / persistent.name, intermediate,
                   parent_alias / self.work.name / 'dispatch.json']
        for index, target in enumerate(targets):
            with self.subTest(target=target):
                queue = self.base / f'receipt-queue-{index}'
                attempt = queue / 'batches/run/target/attempt-1'
                (attempt / 'work').mkdir(parents=True)
                (attempt / 'work/dispatch.json').symlink_to(target)
                store.write_new(attempt / 'reservation.json', {'maxBudgetUsd': 6})
                self.plan['accounting_roots'] = [str(queue)]
                self.save_plan()
                before = regrade.ledger(queue)
                self.assertEqual(before, (regrade.Decimal('2.5'), []))
                with self.assertRaisesRegex(store.EvidenceError, 'workspace-dependent accounting'):
                    self.run_retirement(apply=True)
                self.assertTrue(self.work.exists())
                self.assertEqual(regrade.ledger(queue), before)

    def test_zero_charge_evidence_file_is_a_dependency_even_with_an_external_work_directory(self):
        attempt = self.queue / 'batches/run/target/attempt-1'
        (attempt / 'work').mkdir(parents=True)
        store.write_new(attempt / 'reservation.json', {'maxBudgetUsd': 6})
        evidence = self.work / 'stderr.log'
        resolution = {'chargeUpperUsd': 0, 'evidence': [{'path': str(evidence), 'sha256': store.digest(evidence)}]}
        store.write_new(attempt / 'budget-resolution.json', resolution)
        before = regrade.ledger(self.queue)
        self.assertEqual(before, (regrade.Decimal(0), []))
        with self.assertRaisesRegex(store.EvidenceError, 'workspace-dependent accounting'):
            self.run_retirement(apply=True)
        self.assertEqual(regrade.ledger(self.queue), before)
        resolution['evidence'][0]['path'] = 'relative-to-another-controller-checkout.log'
        (attempt / 'budget-resolution.json').write_text(json.dumps(resolution))
        with self.assertRaisesRegex(store.EvidenceError, 'original controller checkout'):
            self.run_retirement(apply=True)

    def test_accounting_path_checks_do_not_confuse_unrelated_paths(self):
        self.assertFalse(retire.path_depends_on(self.base / 'work-other/file', self.work))
        self.assertFalse(retire.path_depends_on(self.queue / 'dispatch.json', self.work))
        self.assertTrue(retire.path_depends_on(self.work / 'dispatch.json', self.work))

    def test_unknown_symlink_and_modified_worktree_are_not_capture_shortcuts(self):
        current = inventory.inventory(self.work, hash_all=True)
        manifest = store.read(self.repo / self.manifest_name)
        unknown = copy.deepcopy(current)
        unknown['entries'][0]['class'] = 'unknown'
        with self.assertRaisesRegex(store.EvidenceError, 'unknown'):
            retire.capture_check(unknown, unknown, manifest, self.prefix)
        (self.work / 'linked').symlink_to(self.repo)
        current = inventory.inventory(self.work, hash_all=True)
        with self.assertRaisesRegex(store.EvidenceError, 'non-file'):
            retire.capture_check(current, current, manifest, self.prefix)

    def test_cli_refuses_an_open_plan(self):
        self.plan['investigation'] = 'open'
        self.save_plan()
        result = subprocess.run([sys.executable, retire.__file__, '--root', str(self.repo), '--plan', self.plan_name,
                                 '--shared-commit', 'a'*40, '--shared-branch', 'main', '--receipt', str(self.base/'receipt')],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('closure', result.stdout)

    def test_linked_worktree_requires_pushed_head_clean_tree_and_captured_ignored_files(self):
        def git(*args):
            subprocess.run(['git', '-C', str(self.repo), *args], check=True, capture_output=True)
        git('init', '-q')
        (self.repo / '.gitignore').write_text('ignored/\n')
        git('add', '.')
        git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.com', 'commit', '-qm', 'fixture')
        shutil.rmtree(self.work)
        git('worktree', 'add', '-b', 'retire-fixture', str(self.work))
        self.plan['kind'] = 'worktree'

        def snapshot():
            (self.repo / self.inventory_name).write_text(json.dumps(inventory.inventory(self.work, hash_all=True)))
            self.save_plan()

        snapshot()
        with patch.object(retire, 'active_users', return_value=[]):
            self.assertFalse(self.run_retirement()['applied'])
        (self.base / 'receipt.json').unlink()
        original = self.remote.api
        head = inventory.git(self.work, 'rev-parse', 'HEAD')
        with patch.object(self.remote, 'api', side_effect=lambda route: {'status': 'diverged'} if '/compare/'+head in route else original(route)):
            with self.assertRaisesRegex(store.EvidenceError, 'unpushed'):
                self.run_retirement(apply=True)
        (self.work / 'ignored').mkdir()
        (self.work / 'ignored/diagnostics.log').write_text('uncaptured diagnostics')
        snapshot()
        with self.assertRaisesRegex(store.EvidenceError, 'uncaptured'):
            self.run_retirement(apply=True)
        (self.work / '.gitignore').write_text('modified')
        snapshot()
        with self.assertRaisesRegex(store.EvidenceError, 'modified development'):
            self.run_retirement(apply=True)
        self.assertTrue(self.work.exists())


if __name__ == '__main__':
    unittest.main()
