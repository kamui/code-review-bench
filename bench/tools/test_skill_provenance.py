import hashlib
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
import unittest

import skill_provenance as provenance


class ProvenanceTest(unittest.TestCase):
    def setUp(self):
        self.temp = TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git('init', '-q')
        self.git('config', 'user.name', 'Fixture')
        self.git('config', 'user.email', 'fixture@example.test')
        self.git('remote', 'add', 'origin', 'https://github.com/example/skills.git')
        self.write('SKILL.md', '---\nname: review\n---\nRead references/rubric.md.\n')
        self.write('references/rubric.md', 'Review criteria\n')
        self.commit('2026-01-01T12:00:00Z')

    def git(self, *args, env=None):
        return subprocess.check_output(['git', '-C', str(self.root), *args], env=env).decode().strip()

    def write(self, path, contents):
        file = self.root / 'skill' / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(contents)

    def commit(self, timestamp):
        import os
        self.git('add', '.')
        self.git('commit', '-qm', 'Fixture change', env={**os.environ, 'GIT_AUTHOR_DATE': timestamp,
                                                     'GIT_COMMITTER_DATE': timestamp})

    def collect(self, **kwargs):
        return provenance.collect(self.root, 'HEAD', 'skill', **kwargs)

    def test_design_history_and_tests_do_not_change_the_fallback_date(self):
        for name in ('DESIGN.md', 'CHANGELOG.md', 'HISTORY.md', 'scripts/test_review.py'):
            self.write(name, 'Documentation or skill tests\n')
        self.commit('2026-02-01T12:00:00Z')
        record = self.collect()
        self.assertEqual(record['date'], '2026-01-01')
        self.assertIsNone(record['version'])
        self.write('references/rubric.md', 'New review criteria\n')
        self.commit('2026-03-01T12:00:00Z')
        self.assertEqual(self.collect()['date'], '2026-03-01')

    def test_deleted_runtime_file_changes_the_date(self):
        (self.root / 'skill/references/rubric.md').unlink()
        self.commit('2026-04-01T12:00:00Z')
        self.assertEqual(self.collect()['date'], '2026-04-01')

    def test_explicit_runtime_dependencies_override_documentation_exclusions(self):
        self.write('references/README.md', 'Instructions the skill reads\n')
        self.commit('2026-02-01T12:00:00Z')
        record = self.collect(inclusions=['references/*.md'])
        self.assertEqual(record['date'], '2026-02-01')
        self.assertIn('references/README.md', [file['path'] for file in record['runtime_files']])

    def test_runtime_snapshot_must_match_the_recorded_revision(self):
        self.collect(frozen=self.root / 'skill')
        self.write('references/rubric.md', 'Uncommitted change\n')
        with self.assertRaisesRegex(ValueError, 'differs from upstream'):
            self.collect(frozen=self.root / 'skill')

    def test_release_date_and_version_require_matching_runtime(self):
        self.git('tag', 'v1.2.3')
        release = {'tag_name': 'v1.2.3', 'published_at': '2026-01-10T00:00:00Z',
                   'html_url': 'https://github.com/example/skills/releases/tag/v1.2.3'}
        record = self.collect(release=release)
        self.assertEqual((record['version'], record['date'], record['date_source']), ('1.2.3', '2026-01-10', 'release'))
        self.write('references/rubric.md', 'New review criteria\n')
        self.commit('2026-02-01T12:00:00Z')
        with self.assertRaisesRegex(ValueError, 'release runtime differs'):
            self.collect(release=release)

    def test_declared_version_is_kept_without_a_release_date(self):
        self.write('SKILL.md', '---\nname: review\nversion: "2.3.4"\n---\nReview\n')
        self.commit('2026-02-01T12:00:00Z')
        record = self.collect()
        self.assertEqual(provenance.label(record), '2.3.4 · 2026-02-01')
        self.assertEqual(record['date_source'], 'commit')

    def test_empty_and_null_versions_are_omitted(self):
        for version in ('', 'null', '""', '# undeclared'):
            with self.subTest(version=version):
                self.write('SKILL.md', f'---\nname: review\nversion: {version}\ndescription: Review\n---\nReview\n')
                self.commit('2026-02-01T12:00:00Z')
                self.assertIsNone(self.collect()['version'])

    def test_frozen_pin_rejects_changed_metadata_and_wrong_tree(self):
        record = self.collect()
        path = self.root / 'release.json'
        path.write_text(json.dumps(record))
        arm = {'resolved_skill_tree': record['benchmark_tree'], 'skill_provenance': {
            'path': 'release.json', 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}}
        self.assertEqual(provenance.verify_pin(self.root, arm), record)
        arm['resolved_skill_tree'] = 'another-tree'
        with self.assertRaisesRegex(ValueError, 'different benchmark tree'):
            provenance.verify_pin(self.root, arm)
        path.write_text('{}')
        with self.assertRaisesRegex(ValueError, 'frozen hash'):
            provenance.verify_pin(self.root, arm)


if __name__ == '__main__':
    unittest.main()
