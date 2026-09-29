import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import collect_run


class CollectRunTest(unittest.TestCase):
    def test_archive_remains_portable_and_tampering_is_rejected(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            run = root / 'bench/runs/test'
            attempt = run / 'attempts/att-001'
            attempt.mkdir(parents=True)
            source = root / 'external.tar.gz'
            source.write_bytes(b'transcript fixture')
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            (attempt / 'attempt.json').write_text(json.dumps({
                'transcript_archive': {'path': str(source), 'sha256': digest},
            }))
            with patch.object(collect_run, 'ROOT', root):
                collect_run.collect(run)
                source.unlink()
                collect_run.collect(run)
                entry, = json.loads((run / 'transcripts.json').read_text())
                self.assertEqual(entry['sha256'], digest)
                archive = root / entry['path']
                self.assertEqual(archive.read_bytes(), b'transcript fixture')
                archive.write_bytes(b'changed')
                with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
                    collect_run.collect(run)
