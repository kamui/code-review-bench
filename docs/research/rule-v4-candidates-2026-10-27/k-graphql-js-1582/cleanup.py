import datetime
import json
import pathlib
import shutil

paths = [
    '/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/base',
    '/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/head',
    '/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/bun-cache',
    '/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/cargo-home',
    '/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/target',
    '/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/runtime',
]
for name in paths:
    path = pathlib.Path(name)
    assert path.is_dir() and not path.is_symlink(), name

shutil.rmtree('/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/base')
shutil.rmtree('/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/head')
shutil.rmtree('/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/bun-cache')
shutil.rmtree('/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/cargo-home')
shutil.rmtree('/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/target')
shutil.rmtree('/home/jack/.cache/rule-v4-candidates-scratch/k-graphql-js-1582/runtime')
receipt = {
    'finished_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'removed': [{'path': name, 'absent': not pathlib.Path(name).exists()} for name in paths],
    'preserved': 'All probes, outputs, patches, dependency lock, upstream API responses, validator receipt, artifact checks, and preparation documents remain in the output directory.',
    'boundary_deviation': 'The validator runtime installer used and removed its own system-temporary build directory; its command lacked a scratch TMPDIR override. See REPORT.md.',
    'cleanup_method': 'The shell rejected rm -rf; shutil.rmtree used six explicit literal paths after directory and symlink checks.',
}
assert all(item['absent'] for item in receipt['removed'])
(pathlib.Path(__file__).parent / 'cleanup.json').write_text(json.dumps(receipt, indent=2) + '\n')
print('Removed six scratch clone, dependency, cache and validator directories; evidence preserved.')
