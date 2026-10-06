from pathlib import Path
import os
import shutil
import subprocess
import sys

scratch = Path('<scratch>/source')
output = Path(__file__).resolve().parent
groups = sys.argv[2:] or ['N1', 'Q1', 'Q2', 'Q3', 'Q4']
mode = sys.argv[1] if len(sys.argv) > 1 else 'jsdom'
suffix = '' if mode == 'jsdom' else '-chromium'
commits = {'base': '30b8ea2004fa999bed151204208676c6c0a9d261', 'head': '14d39e5d1ad6b7aca2fb067415dba09c6bea219b'}
versions = subprocess.run(['node', '--version'], capture_output=True, text=True).stdout
versions += 'pnpm ' + subprocess.run(['corepack', 'pnpm', '--version'], cwd=scratch, capture_output=True, text=True).stdout
versions += subprocess.run(['./node_modules/.bin/vitest', '--version'], cwd=scratch, capture_output=True, text=True).stdout
versions += subprocess.run(['node', '-e', "for (const p of ['react','react-dom','jsdom','vite']) console.log(p,require(p+'/package.json').version)"], cwd=scratch, capture_output=True, text=True).stdout
versions += 'Installed using corepack pnpm install --frozen-lockfile --ignore-scripts --store-dir ../pnpm-store.\n'
versions += 'The pnpm shim had no configured version; corepack selected the project-pinned pnpm 11.17.0.\n'
versions += 'Linux. jsdom simulated browser; Chromium browser when run with chromium argument. Q1 simulates text arriving before server hydration. Actual browser autofill is not run.\n'
versions += '\n'.join(f'{k} {v}' for k,v in commits.items()) + '\n'
(scratch.parent / 'tmp').mkdir(exist_ok=True)
for g in groups:
    (output / g / 'environment.txt').write_text(versions)
for label, sha in commits.items():
    subprocess.run(['git','checkout','--detach',sha], cwd=scratch, check=True)
    for g in groups:
        relative = Path('packages/react/src/field/control/zzruling-' + g + '.test.jsx')
        shutil.copyfile(output / g / 'probe.test.jsx', scratch / relative)
    for g in groups:
        command = ['./node_modules/.bin/vitest', 'run', '--project', '@base-ui/react', 'packages/react/src/field/control/zzruling-' + g + '.test.jsx']
        env = {**os.environ, 'VITEST_ENV': mode, 'TMPDIR': str(scratch.parent / 'tmp'), 'PLAYWRIGHT_BROWSERS_PATH': '<home>/.cache/ms-playwright', 'TZ': 'UTC', 'NO_COLOR': '1'}
        result = subprocess.run(command, cwd=scratch, env=env, capture_output=True, text=True)
        (output / g / ('result-' + label + suffix + '.txt')).write_text('Commit: ' + sha + '\nCommand: VITEST_ENV=' + mode + ' TZ=UTC ' + ' '.join(command) + '\nExit code: ' + str(result.returncode) + '\n' + result.stdout + result.stderr)
        print(label, g, 'exit', result.returncode, flush=True)
