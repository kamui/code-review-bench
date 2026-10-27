import concurrent.futures
import json
from pathlib import Path
import subprocess
import base64
import importlib.util

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('fetch', HERE / 'fetch.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
requests = {}
for number in [926, 263, 222, 363, 53, 522, 207, 3455, 3967, 4886]:
    requests['issue-' + str(number)] = ['repos/graphql/graphql-js/issues/' + str(number)]
    requests['issue-' + str(number) + '-comments'] = ['repos/graphql/graphql-js/issues/' + str(number) + '/comments?per_page=100']
for number in [926,3455,3967,4886]:
    requests['pr-' + str(number) + '-files'] = ['repos/graphql/graphql-js/pulls/' + str(number) + '/files?per_page=100']
    requests['pr-' + str(number) + '-review-comments'] = ['repos/graphql/graphql-js/pulls/' + str(number) + '/comments?per_page=100']
requests['public-code-validator'] = ['--method','GET','search/code','-f','q=repo:graphql/express-graphql validateFn','-f','per_page=100']
requests['public-code-history'] = ['repos/graphql/express-graphql/commits?path=src/index.ts&until=2022-01-17T11:22:02Z&per_page=1']
requests['release-v16.3.0'] = ['repos/graphql/graphql-js/releases/tags/v16.3.0']
with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
    records = list(pool.map(module.fetch, requests.items()))
history = json.loads((module.OUT / 'public-code-history.json').read_text())
sha = history[0]['sha']
records.append(module.fetch(('public-code-source', ['repos/graphql/express-graphql/contents/src/index.ts?ref=' + sha])))
raw = json.loads((module.OUT / 'public-code-source.json').read_text())
(module.OUT / 'public-code-source.ts').write_bytes(base64.b64decode(raw['content']))
(module.OUT / 'fetch-more-log.json').write_text(json.dumps(records, indent=2) + '\n')
print(json.dumps(records, indent=2))
