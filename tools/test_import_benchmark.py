"""An edited imported file keeps its imported bytes available to the check."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import import_benchmark


def git(root, *arguments):
    subprocess.run(['git', '-C', str(root), '-c', 'user.name=fixture', '-c', 'user.email=fixture@example.invalid',
                    *arguments], check=True, capture_output=True)


class PreserveTest(unittest.TestCase):
    def test_committed_edit_is_named_then_preserved_from_history(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            imported = {'bench/harness/client.json': b'{"version": 1}\n', 'bench/runs/example/review.md': b'frozen\n'}
            for name, data in imported.items():
                (root / name).parent.mkdir(parents=True)
                (root / name).write_bytes(data)
            manifest = root / 'bench/import-manifest.json'
            manifest.write_text(json.dumps({'files': [
                {'path': name, 'sha256': import_benchmark.digest(data), 'bytes': len(data)}
                for name, data in imported.items()], 'transcripts': []}))
            git(root, 'init', '--quiet')
            git(root, 'add', '.')
            git(root, 'commit', '--quiet', '-m', 'import')
            for name in imported:
                (root / name).write_bytes(b'edited\n')
            git(root, 'commit', '--quiet', '-am', 'edit')
            with patch.object(import_benchmark, 'ROOT', root), patch.object(import_benchmark, 'MANIFEST', manifest):
                with self.assertRaises(ValueError) as refusal:
                    import_benchmark.verify()
                self.assertTrue(str(refusal.exception).endswith(
                    'python3 tools/import_benchmark.py --preserve bench/harness/client.json'))
                with self.assertRaisesRegex(ValueError, 'evidence: bench/runs/example/review.md$'):
                    import_benchmark.preserve(['bench/harness/client.json'])
            preserved = root / 'artifacts/import-source/bench/harness/client.json'
            self.assertEqual(preserved.read_bytes(), imported['bench/harness/client.json'])
            self.assertEqual((root / 'bench/harness/client.json').read_bytes(), b'edited\n')
            self.assertEqual(json.loads(manifest.read_text())['files'][0]['preserved_path'],
                             'artifacts/import-source/bench/harness/client.json')


if __name__ == '__main__':
    unittest.main()
