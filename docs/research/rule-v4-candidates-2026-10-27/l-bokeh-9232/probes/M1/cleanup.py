import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232')
TARGETS = [
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/source'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/deps'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/vendor'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/venv'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/py310'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/cache'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/cargo-home'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/rust-target'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/runtime'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/t'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/browser-temp'),
    Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232/browser-config'),
]

for target in TARGETS:
    assert target.parent == ROOT and not target.is_symlink(),target
    assert target.resolve().parent == ROOT.resolve(),target
source = ROOT/'source'
if source.exists():
    subprocess.run(['git','diff','--exit-code','HEAD'],cwd=source,check=True,capture_output=True)
for target in TARGETS:
    if target.exists():
        shutil.rmtree(target)
receipt = {
    'removed':{str(target):not target.exists() for target in TARGETS},
    'remaining':[target.name for target in ROOT.iterdir()],
    'retained':'All probes, patches, outputs, environment, API responses and validation receipts remain in the dossier directory.',
    'method':'Fixed allowlist, direct-child and symlink checks, clean tracked clone check, then shutil.rmtree. Shell rm -f-style deletion was refused.'
}
assert all(receipt['removed'].values())
Path(__file__).with_suffix('.json').write_text(json.dumps(receipt,indent=2)+'\n')
print('Removed all allowlisted scratch build, clone, browser and dependency directories.')
