import json, subprocess, sys
from pathlib import Path
root = Path(__file__).resolve().parents[4]
report = root / 'docs/research/selected-cache-rebuild-2026-10-02'
cache = root / '.local/selected-cache-rebuild-2026-10-02/cache'
for entry in json.loads((report / 'cache-replacements.v1.json').read_text())['targets']:
    name = entry['target']
    out = report / 'smoke' / f'{name}.json'
    out.parent.mkdir(exist_ok=True)
    with (report / 'logs' / f'{name}-offline-smoke.log').open('x') as log:
        done = subprocess.run(['bwrap', '--unshare-net', '--bind', '/', '/', '--dev-bind', '/dev', '/dev', '--proc', '/proc', '--', sys.executable, 'bench/tools/provision.py', 'smoke', '--target', f'bench/targets/{name}', '--cache-root', str(cache), '--cache-replacements', str(report / 'cache-replacements.v1.json'), '--out', str(out)], stdout=log, stderr=subprocess.STDOUT)
    if done.returncode or not out.exists() or any((check['exit_code'] for check in json.loads(out.read_text())['checks'])):
        print('Failed offline smoke:', name, flush=True)
        sys.exit(1)
    print('Passed offline smoke:', name, flush=True)
