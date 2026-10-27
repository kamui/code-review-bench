import base64
import concurrent.futures
import json
import pathlib
import subprocess

OUT = pathlib.Path(__file__).parent / 'upstream'
jobs = {}
for number in [443, 487, 951, 949, 1131, 1202, 743, 4774, 1601]:
    jobs[f'issue-{number}.json'] = f'repos/graphql/graphql-js/issues/{number}'
    jobs[f'issue-{number}-comments.json'] = f'repos/graphql/graphql-js/issues/{number}/comments?per_page=100'
for number in [443, 487, 951, 949, 1131, 4774, 1601]:
    jobs[f'pr-{number}-files.json'] = f'repos/graphql/graphql-js/pulls/{number}/files?per_page=100'
for i, item in enumerate(json.loads((OUT / 'public-code.json').read_text())['items']):
    jobs[f'public-file-{i}.json'] = item['url'].replace('https://api.github.com/', '')
    jobs[f'public-date-{i}.json'] = f"repos/{item['repository']['full_name']}/commits?path={item['path']}&per_page=1"

def capture(item):
    name, endpoint = item
    p = subprocess.run(['gh', 'api', endpoint], capture_output=True, text=True)
    (OUT / name).write_text(p.stdout)
    (OUT / (name + '.stderr')).write_text(p.stderr)
    if name.startswith('public-file-') and p.returncode == 0:
        data = json.loads(p.stdout)
        (OUT / name.replace('.json', '.txt')).write_bytes(base64.b64decode(data['content']))
    return {'saved': name, 'command': ['gh', 'api', endpoint], 'exit': p.returncode}

with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
    result = list(pool.map(capture, jobs.items()))
(OUT / 'details-commands.json').write_text(json.dumps(result, indent=2) + '\n')
print('Captured', len(result), 'responses; failures:', [x for x in result if x['exit']])
