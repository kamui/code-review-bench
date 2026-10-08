import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import evidence_inventory as inventory


class InventoryTest(unittest.TestCase):
    def test_ignored_and_unknown_files_are_inventoried_without_following_links(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / 'workspace'
            root.mkdir()
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            (root / '.gitignore').write_text('.local/\n')
            (root / 'source.txt').write_text('tracked')
            subprocess.run(['git', '-C', str(root), 'add', '.'], check=True)
            subprocess.run(['git', '-C', str(root), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.com',
                            'commit', '-qm', 'fixture'], check=True)
            (root / '.local').mkdir()
            (root / '.local/diagnostics.log').write_text('unique')
            (root / 'unknown.bin').write_bytes(b'unknown')
            (root / 'link').symlink_to(base)
            result = inventory.inventory(root)
            entries = {item['path']: item for item in result['entries']}
            self.assertEqual(entries['source.txt']['class'], 'tracked-identical')
            self.assertEqual(entries['.local/diagnostics.log']['class'], 'local-evidence')
            self.assertEqual(entries['unknown.bin']['class'], 'unknown')
            self.assertEqual(entries['link']['class'], 'unknown')
            self.assertFalse(result['retirement_authorized'])
            (root / 'source.txt').write_text('changed')
            changed = inventory.inventory(root)
            self.assertEqual(next(e for e in changed['entries'] if e['path']=='source.txt')['class'], 'development')
            output = base / 'inventory.json'
            done = subprocess.run([sys.executable, inventory.__file__, '--root', str(root), '--out', str(output)],
                                  capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertEqual(json.loads(output.read_text())['roots'][0]['root'], str(root))
            refused = subprocess.run([sys.executable, inventory.__file__, '--root', str(root), '--out', str(root/'inventory.json')],
                                     capture_output=True, text=True)
            self.assertEqual(refused.returncode, 1)
            self.assertFalse((root / 'inventory.json').exists())


if __name__ == '__main__':
    unittest.main()
