import pathlib, subprocess
repo = pathlib.Path.cwd()
output = repo / 'docs/research/cohort-rebuild-2026-10-05/second-pass/candidates/p-hono-5067/probes/N1'
scratch = pathlib.Path('<scratch>')
source = scratch / 'source'
entry = scratch / 'entry.ts'
entry.write_text("export { Hono } from './source/src/index'\nexport { HonoRequest } from './source/src/request'\n")
versions = []
for command in [['node', '--version'], ['bun', '--version'], ['git', '--version'], ['gh', '--version'], ['python3', '--version']]:
    versions.append(' '.join(command) + '\n' + subprocess.check_output(command, text=True))
output.joinpath('environment.txt').write_text(''.join(versions) + 'No dependencies installed. Bun build bundles actual source for Node and Bun. No parser or cache mocks.\n')
for label, revision in [('base', '9728702911073aec5a63a3ba2840b7240e5d3205'), ('head', '5226d4165d48643586152614cbd07422a0ab7a22'), ('release-v4.12.28', 'v4.12.28'), ('release-v4.12.31', 'v4.12.31')]:
    subprocess.run(['git', '-C', str(source), 'checkout', '--detach', revision], check=True, capture_output=True)
    build = subprocess.run(['bun', 'build', str(entry), '--target=node', '--outfile=' + str(scratch / 'bundle.mjs')], check=True, capture_output=True, text=True)
    text = 'Revision: ' + subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True) + 'Build: ' + build.stdout + build.stderr
    for runtime in ['node', 'bun']:
        result = subprocess.run([runtime, str(output / 'probe.mjs'), str(scratch / 'bundle.mjs')], capture_output=True, text=True)
        text += '\n' + runtime + ' exit=' + str(result.returncode) + '\n' + result.stdout + result.stderr
        if result.returncode: raise RuntimeError(text)
    output.joinpath('result-' + label + '.txt').write_text(text)
    print(label + ': completed on Node and Bun')
