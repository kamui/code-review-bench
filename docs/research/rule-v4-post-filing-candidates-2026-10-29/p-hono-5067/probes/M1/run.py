"""Driver for group M1. Run with: python3 -I run.py

Needs the clone at SCRATCH/clone with node_modules linked from the dependency
archive (see README.md). Writes every result beside this file.
"""
import difflib
import json
import pathlib
import subprocess

OUT = pathlib.Path(__file__).resolve().parent
SCRATCH = pathlib.Path('/home/jack/.cache/rule-v4-post-filing-scratch/p-hono-5067')
CLONE = SCRATCH / 'clone'
BASE = '9728702911073aec5a63a3ba2840b7240e5d3205'
HEAD = '5226d4165d48643586152614cbd07422a0ab7a22'
BODY = 'src/utils/body.ts'
RUNTIMES = [['node'], ['bun'], ['deno', 'run', '--allow-read', '--allow-env']]

DISPATCH = "  if (mediaType === 'multipart/form-data' || mediaType === 'application/x-www-form-urlencoded') {\n"
GT_P6 = " || contentType?.toLowerCase().startsWith('multipart/form-data ')"
M1 = " || /^multipart\\/form-data[^\\s;]/.test(contentType?.toLowerCase() ?? '')"
OWN = {
    # last round's smallest fix for GT-p6 (rule-v4-candidates, probes/M3/fix-M3.patch), unchanged
    'fix-GT-p6': DISPATCH.replace(') {\n', GT_P6 + ') {\n'),
    # smallest fix for this group's headers alone: `multipart/form-data` followed by a character that is neither space nor `;`
    'fix-M1': DISPATCH.replace(') {\n', M1 + ') {\n'),
    # both clauses together
    'fix-GT-p6+M1': DISPATCH.replace(') {\n', GT_P6 + M1 + ') {\n'),
    # one expression: a multipart media type is recognised by its prefix, as before the change, after lower-casing
    'fix-one-expression': DISPATCH.replace("mediaType === 'multipart/form-data'", "mediaType?.startsWith('multipart/form-data')"),
}
# known problems' patches from rule-v4-candidates (probes/patches), copied under their family ids
OTHERS = ['fix-GT-p1', 'fix-GT-p2', 'fix-GT-p3', 'fix-GT-p4', 'fix-GT-p5', 'fix-GT-p7', 'fix-PE-p1']
TESTS = ['src/utils/body.test.ts', 'src/utils/buffer.test.ts', 'src/validator/validator.test.ts']


def command(args, check=True, **kwargs):
    done = subprocess.run(args, text=True, capture_output=True, **kwargs)
    if check and done.returncode != 0:
        raise SystemExit(' '.join(args) + '\n' + done.stdout + done.stderr)
    return done


def git(*args):
    return command(['git', '-C', str(CLONE), *args]).stdout


def bundle():
    entry = SCRATCH / 'entry.mjs'
    entry.write_text('\n'.join('export { ' + symbol + ' } from ' + json.dumps(str(CLONE / path)) for symbol, path in [
        ('Hono', 'src/index.ts'), ('HonoRequest', 'src/request.ts'), ('parseBody', 'src/utils/body.ts'),
        ('validator', 'src/validator/validator.ts'), ('prettyJSON', 'src/middleware/pretty-json/index.ts'),
        ('methodOverride', 'src/middleware/method-override/index.ts'),
    ]) + '\n')
    command([str(CLONE / 'node_modules/.bin/esbuild'), str(entry), '--bundle', '--format=esm', '--platform=node',
             '--log-level=warning', '--outfile=' + str(SCRATCH / 'api.mjs')])


def probe(label, script='probe.mjs', prefix='result-', runtimes=RUNTIMES):
    bundle()
    combined = []
    for runtime in runtimes:
        done = command([*runtime, str(OUT / script), str(SCRATCH / 'api.mjs')], check=False)
        text = done.stdout + (('\nSTDERR exit ' + str(done.returncode) + '\n' + done.stderr) if done.returncode else '')
        (OUT / (prefix + label + '-' + runtime[0] + '.txt')).write_text(text)
        combined.append(text)
    (OUT / (prefix + label + '.txt')).write_text(''.join(combined))


def tests(label):
    done = command([str(CLONE / 'node_modules/.bin/vitest'), 'run', '--project', 'main', '--coverage.enabled=false', *TESTS], check=False, cwd=CLONE)
    tail = '\n'.join(line for line in (done.stdout + done.stderr).splitlines() if line.strip().startswith(('Test Files', 'Tests', '✓', '×', 'FAIL', '❯')))
    (OUT / ('tests-' + label + '.txt')).write_text('exit ' + str(done.returncode) + '\n' + tail + '\n')


def checkout(commit):
    git('checkout', '--quiet', '--force', '--detach', commit)


def own_patch(name):
    original = (CLONE / BODY).read_text()
    assert original.count(DISPATCH) == 1
    changed = original.replace(DISPATCH, OWN[name])
    (CLONE / BODY).write_text(changed)
    patch = ''.join(difflib.unified_diff(original.splitlines(True), changed.splitlines(True), fromfile='a/' + BODY, tofile='b/' + BODY))
    (OUT / 'patches' / (name + '.patch')).write_text(patch)


def other_patch(name):
    git('apply', str(OUT / 'patches' / (name + '.patch')))


if __name__ == '__main__':
    versions = [command(args).stdout.strip() for args in [['node', '--version'], ['bun', '--version'], ['deno', '--version'], ['git', '--version'], [str(CLONE / 'node_modules/.bin/esbuild'), '--version'], [str(CLONE / 'node_modules/.bin/vitest'), '--version']]]
    (OUT / 'environment.txt').write_text(
        'node ' + versions[0] + '\nbun ' + versions[1] + '\n' + versions[2] + '\n' + versions[3] + '\nesbuild ' + versions[4] + '\nvitest ' + versions[5]
        + '\nplatform ' + command(['uname', '-srm']).stdout.strip()
        + '\nBase: ' + BASE + '\nHead: ' + HEAD
        + '\nDependencies: /home/jack/.t3/bench-cache/archives/p-hono-5067.tar.gz (sha256 0e0e2ff5dbe7c7756072e964487b946d8eea118a69c6bd056cdac5c2b450ccf2), node_modules linked into the clone; nothing installed.'
        + '\nBuild: src/ of the checkout bundled with the cached esbuild into api.mjs; probe.mjs imports it.\n')
    (OUT / 'change.diff').write_text(git('diff', BASE, HEAD, '--', BODY, 'src/utils/buffer.ts'))
    for label, commit in [('base', BASE), ('head', HEAD)]:
        checkout(commit)
        probe(label)
        probe(label, 'known-probe.mjs', 'known-', [['node'], ['bun']])
        tests(label)
    for name in list(OWN) + OTHERS:
        checkout(HEAD)
        (own_patch if name in OWN else other_patch)(name)
        probe(name)
        if name in OWN:
            probe(name, 'known-probe.mjs', 'known-', [['node'], ['bun']])
            tests(name)
    checkout(HEAD)
