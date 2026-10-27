import json, os, subprocess
from pathlib import Path
ROOT = Path(__file__).resolve().parent
SCRATCH = Path('/home/jack/.cache/rule-v4-candidates-scratch/n-ripgrep-2957')
REPO = SCRATCH / 'repo'
source = REPO / 'crates/core/flags/complete/rg.zsh'
original = source.read_text()
rows = []
try:
    for variant in ['head', 'candidate-only', 'known-only', 'both']:
        patched = SCRATCH / 'variants/M1/GT-n5' / variant / '_rg'
        source.write_text(patched.read_text())
        for context in ['stub', 'initialized']:
            env = os.environ.copy()
            env['ZDOTDIR'] = str(SCRATCH / 'test-homes' / context)
            result = subprocess.run(['zsh', str(REPO / 'ci/test-complete')], cwd=REPO, env=env, capture_output=True, text=True)
            rows.append(dict(variant=variant, context=context, exit=result.returncode, stdout=result.stdout, stderr=result.stderr))
finally:
    source.write_text(original)
(ROOT / 'M1/grouping/GT-n5-full/full-test-results.json').write_text(json.dumps(rows, indent=2) + '\n')
assert all(row['exit'] == (0 if row['variant'] in ['candidate-only', 'both'] else 1) for row in rows)
print('Full-context patch matrix: all eight expected full-check outcomes confirmed.')
