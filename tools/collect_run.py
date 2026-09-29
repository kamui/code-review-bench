"""Collect portable, checksum-verified transcript references for a completed run."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]


def collect(run):
    entries = []
    for path in sorted((run / 'attempts').glob('*/attempt.json')):
        record = json.loads(path.read_text())
        archive = record.get('transcript_archive')
        if not archive:
            continue
        destination = ROOT / 'artifacts/transcripts' / run.name / f'{path.parent.name}.tar.gz'
        source = destination if destination.is_file() else Path(archive['path']).expanduser()
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        if digest != archive['sha256']:
            raise ValueError(f'Transcript checksum mismatch: {path}')
        destination.parent.mkdir(parents=True, exist_ok=True)
        if source.resolve() != destination.resolve():
            shutil.copyfile(source, destination)
        entries.append({'attempt': path.relative_to(ROOT).as_posix(),
                        'path': destination.relative_to(ROOT).as_posix(),
                        'sha256': digest, 'status': 'verified'})
    (run / 'transcripts.json').write_text(json.dumps(entries, indent=2) + '\n')
    print(f'Collected {len(entries)} verified transcripts for {run.name}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', type=Path, required=True)
    args = parser.parse_args()
    collect(args.run.resolve())
