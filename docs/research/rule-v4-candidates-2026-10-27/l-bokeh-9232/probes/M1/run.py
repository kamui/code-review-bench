import difflib
import json
import os
import subprocess
from pathlib import Path

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[1]
SCRATCH = Path('/home/jack/.cache/rule-v4-candidates-scratch/l-bokeh-9232')
SOURCE = SCRATCH / 'source'
DEPS = SCRATCH / 'deps'
VENDOR = SCRATCH / 'vendor'
FILE = 'bokehjs/src/lib/models/widgets/date_picker.ts'
BASE = 'ccb4bcb4c2b841d89b0e88303a97bf4604a5795f'
HEAD = '36549bca3a63d581f7b68d08054a7813c1e6a499'
ZONES = ['America/New_York','America/Los_Angeles','UTC','Europe/London','Europe/Paris',
         'Asia/Kolkata','Asia/Tokyo','Pacific/Auckland','Pacific/Kiritimati']


def run(args, **kwargs):
    result = subprocess.run([str(arg) for arg in args], capture_output=True, text=True, timeout=60, **kwargs)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout + result.stderr


def probes(label, wire=False):
    text = f'variant={label}\ncommit=' + run(['git','rev-parse','HEAD'], cwd=SOURCE)
    if wire:
        text += '\nPYTHON MODEL AND SERIALIZATION\n'
        text += run([SCRATCH/'py310/bin/python',OUT/'wire.py',SOURCE], env={**os.environ, 'PYTHONDONTWRITEBYTECODE':'1'})
    text += '\nSOURCE METHODS IN NODE\n'
    for zone in ZONES:
        text += run(['node',OUT/'probe.cjs',SOURCE,DEPS,'node'], env={**os.environ,'TZ':zone})
    text += '\nSOURCE VIEW METHODS AND PINNED PIKADAY IN CHROME\n'
    text += run(['node',OUT/'probe.cjs',SOURCE,DEPS,'browser',VENDOR])
    (OUT/f'result-{label}.txt').write_text(text)
    print(label, 'saved', flush=True)


for label, commit in [('base',BASE),('head',HEAD)]:
    run(['git','checkout','--detach',commit], cwd=SOURCE)
    (ROOT/'upstream'/f'date_picker-{label}.txt').write_text((SOURCE/FILE).read_text())
    probes(label, wire=True)

original = (SOURCE/FILE).read_text()
numeric = original
for field in ['value','min_date','max_date']:
    numeric = numeric.replace(f'this._unlocal_date(new Date(this.model.{field}))',
                              f'this._unlocal_date(new Date(this.model.{field}), typeof this.model.{field} === "number")')
numeric = numeric.replace('_unlocal_date(date: Date): Date {',
    '_unlocal_date(date: Date, from_python = false): Date {\n'
    '    if (from_python)\n'
    '      return new Date(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate())')
start = original.index('    //Get the UTC offset')
end = original.index('\n  }', start)
local = original[:start] + '    return new Date(date.getFullYear(), date.getMonth(), date.getDate())' + original[end:]
start2 = numeric.index('    //Get the UTC offset')
end2 = numeric.index('\n  }',start2)
both = numeric[:start2] + '    return new Date(date.getFullYear(), date.getMonth(), date.getDate())' + numeric[end2:]
for label, patched in [('known-only',numeric),('historical-only',local),('both',both)]:
    patch = ''.join(difflib.unified_diff(original.splitlines(True),patched.splitlines(True),fromfile='a/'+FILE,tofile='b/'+FILE))
    (OUT/f'{label}.patch').write_text(patch)
    (SOURCE/FILE).write_text(patched)
    try:
        probes(label)
    finally:
        (SOURCE/FILE).write_text(original)

environment = {
    'node':run(['node','-p','JSON.stringify(process.versions)']),
    'chrome':run(['/usr/bin/google-chrome','--version']),
    'python':run([SCRATCH/'py310/bin/python','--version']),
    'python_dependencies':run(['uv','pip','freeze','--python',SCRATCH/'py310/bin/python']),
    'js_dependencies':json.loads((DEPS/'package.json').read_text()),
    'base':BASE,'head':HEAD,
    'pikaday_commit':'6b7258e',
    'limits':[
        'Full BokehJS dependency graph and Selenium server suite were not built.',
        'The entire DatePicker TypeScript module is transpiled with TypeScript 3.6.2; its render(), _on_select() and _unlocal_date() execute unchanged.',
        'The host widget layout and model-change notification are minimal adapters; the pinned Pikaday and the browser Date implementation are real.',
        'The original cache metadata and archive paths are absent; dependencies were installed under scratch.',
        'Chrome 155 and Node 24 use current timezone data, not a 2019 browser or timezone build.',
        'Exploratory browser attempt before showing the calendar failed to locate its day button; the saved results use show() before clicking.',
        'A launch using browser-temp under scratch exceeded the Unix socket path limit; the saved runs use the shorter scratch/t path.'
    ]
}
(OUT/'environment.txt').write_text(json.dumps(environment, indent=2)+'\n')
(OUT/'dependencies.package.json').write_bytes((DEPS/'package.json').read_bytes())
(OUT/'dependencies.bun.lock').write_bytes((DEPS/'bun.lock').read_bytes())
(ROOT/'upstream'/'change.diff').write_text(run(['git','diff',BASE,HEAD],cwd=SOURCE))
