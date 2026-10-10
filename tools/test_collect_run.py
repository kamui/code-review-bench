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

    def test_archives_keep_the_repository_path_their_record_names_and_probes_are_collected(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            run = root / 'bench/runs/test'
            cache = root / 'cache/test-probe/att-002.tar.gz'
            filed = {'attempts/att-001': ('~/gone/checkout/artifacts/transcripts/test/att-001.tar.gz', 'artifacts/transcripts/test/att-001.tar.gz'),
                     'attempts/att-002': ('artifacts/transcripts/reviews/test/att-002.tar.gz', 'artifacts/transcripts/reviews/test/att-002.tar.gz'),
                     'attempts/att-003': ('artifacts/custom-transcripts/test/att-003.tar.gz', 'artifacts/custom-transcripts/test/att-003.tar.gz'),
                     'attempts/att-004': ('local-archive/test/att-004.tar.gz', 'artifacts/transcripts/test/att-004.tar.gz'),
                     'probes/att-001': ('/gone/checkout/artifacts/transcripts/test-probe/att-001.tar.gz', 'artifacts/transcripts/test-probe/att-001.tar.gz'),
                     'probes/att-002': (str(cache), 'artifacts/transcripts/test/probes/att-002.tar.gz')}
            copied = {'attempts/att-004': root / 'local-archive/test/att-004.tar.gz', 'probes/att-002': cache}
            for name, (recorded, logical) in filed.items():
                archive = copied.get(name, root / logical)
                archive.parent.mkdir(parents=True, exist_ok=True)
                archive.write_bytes(name.encode())
                (run / name).mkdir(parents=True)
                (run / name / 'attempt.json').write_text(json.dumps({'transcript_archive': {
                    'path': recorded, 'sha256': hashlib.sha256(name.encode()).hexdigest()}}))
            self.assertNotEqual(Path.cwd(), root)
            with patch.object(collect_run, 'ROOT', root):
                collect_run.collect(run)
                entries = json.loads((run / 'transcripts.json').read_text())
                self.assertEqual({entry['attempt']: entry['path'] for entry in entries},
                                 {f'bench/runs/test/{name}/attempt.json': logical for name, (_, logical) in filed.items()})
                for name, source in copied.items():
                    self.assertEqual((root / filed[name][1]).read_bytes(), source.read_bytes())
                (root / 'artifacts/transcripts/test/att-001.tar.gz').unlink()
                with self.assertRaisesRegex(ValueError, 'Transcript archive unavailable: artifacts/transcripts/test/att-001.tar.gz'):
                    collect_run.collect(run)
            record = run / 'attempts/att-001/attempt.json'
            record.write_text(json.dumps({'transcript_archive': {'path': '/gone/artifacts/transcripts/../../escape.tar.gz', 'sha256': ''}}))
            with patch.object(collect_run, 'ROOT', root), self.assertRaisesRegex(ValueError, 'unsafe evidence path'):
                collect_run.collect(run)


if __name__ == '__main__':
    unittest.main()
