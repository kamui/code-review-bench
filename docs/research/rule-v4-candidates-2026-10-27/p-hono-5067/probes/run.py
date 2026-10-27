import json
import pathlib
import shutil
import subprocess

OUT = pathlib.Path(__file__).resolve().parent
SCRATCH = pathlib.Path('/home/jack/.cache/rule-v4-candidates-scratch/p-hono-5067')
CLONE = SCRATCH / 'clone'
BASE = '9728702911073aec5a63a3ba2840b7240e5d3205'
HEAD = '5226d4165d48643586152614cbd07422a0ab7a22'
GROUPS = ['M1', 'M2', 'M3', 'M4', 'M5', 'M6', 'P1', 'P2', 'known']


def command(args, **kwargs):
    return subprocess.run(args, text=True, capture_output=True, check=True, **kwargs).stdout


def bundle():
    entry = SCRATCH / 'entry.mjs'
    entry.write_text('\n'.join('export { ' + symbol + ' } from ' + json.dumps(str(CLONE / path)) for symbol, path in [
        ('Hono', 'src/index.ts'), ('HonoRequest', 'src/request.ts'), ('parseBody', 'src/utils/body.ts'),
        ('validator', 'src/validator/validator.ts'), ('prettyJSON', 'src/middleware/pretty-json/index.ts'),
        ('methodOverride', 'src/middleware/method-override/index.ts'),
    ]))
    command([str(CLONE / 'node_modules/.bin/esbuild'), str(entry), '--bundle', '--format=esm', '--platform=node', '--outfile=' + str(SCRATCH / 'api.mjs')])


def run(label, groups=GROUPS):
    bundle()
    results = {}
    for runtime in ['node', 'bun']:
        result = command([runtime, str(OUT / 'probe.mjs'), str(SCRATCH / 'api.mjs')])
        results[runtime] = result
        for group in groups:
            folder = OUT / group
            folder.mkdir(exist_ok=True)
            lines = [line for line in result.splitlines() if json.loads(line)['group'] == group]
            (folder / ('result-' + label + '-' + runtime + '.txt')).write_text('\n'.join(lines) + '\n')
    for group in groups:
        (OUT / group / ('result-' + label + '.txt')).write_text(''.join('\n'.join(line for line in result.splitlines() if json.loads(line)['group'] == group) + '\n' for result in results.values()))


if __name__ == '__main__':
    environment = '\n'.join(command(args).strip() for args in [['node', '--version'], ['bun', '--version'], ['git', '--version'], [str(CLONE / 'node_modules/.bin/esbuild'), '--version']])
    for group in GROUPS:
        (OUT / group).mkdir(exist_ok=True)
        shutil.copyfile(OUT / 'probe.mjs', OUT / group / 'probe.mjs')
        (OUT / group / 'environment.txt').write_text(environment + '\nBase: ' + BASE + '\nHead: ' + HEAD + '\nBuild: pinned source bundled with cached esbuild; no dependency installation.\nDriver: ../run.py; probe.mjs takes path to generated api.mjs.\n')
    for label, commit in [('base', BASE), ('head', HEAD)]:
        command(['git', '-C', str(CLONE), 'checkout', '--detach', commit])
        run(label)
    diff = command(['git', '-C', str(CLONE), 'diff', BASE, HEAD, '--', 'src/utils/body.ts', 'src/utils/buffer.ts', 'src/validator/validator.ts', 'src/utils/body.test.ts'])
    (OUT / 'change.diff').write_text(diff)
