import base64
import concurrent.futures
import json
from collect import ROOT, api


def fetch_code(args):
    prefix, index, item = args
    repo, path = item['repository']['full_name'], item['path']
    name = prefix + '-hit-' + str(index)
    api(name + '-history', 'repos/' + repo + '/commits', {'path': path, 'until': '2026-07-01T09:39:48Z', 'per_page': 1})
    history = json.loads((ROOT / (name + '-history.json')).read_text())
    if not isinstance(history, list) or not history:
        return name + ': no code at cutoff found'
    sha = history[0]['sha']
    api(name, 'repos/' + repo + '/contents/' + path, {'ref': sha})
    data = json.loads((ROOT / (name + '.json')).read_text())
    if 'content' in data:
        (ROOT / (name + '.txt')).write_bytes(base64.b64decode(data['content']))
    return name + ': ' + history[0]['commit']['committer']['date'] + ' ' + repo + '/' + path


if __name__ == '__main__':
    sha = json.loads((ROOT / 'fetch-cutoff.json').read_text())[0]['sha']
    print(api('fetch-standard-at-cutoff', 'repos/whatwg/fetch/contents/fetch.bs', {'ref': sha}), flush=True)
    data = json.loads((ROOT / 'fetch-standard-at-cutoff.json').read_text())
    if 'content' in data:
        (ROOT / 'fetch-standard-at-cutoff.txt').write_bytes(base64.b64decode(data['content']))
    jobs = []
    for prefix in ['code-validator', 'code-parsebody-retry', 'code-override-retry', 'code-pretty-retry', 'code-specific-parsebody', 'code-specific-override']:
        data = json.loads((ROOT / (prefix + '.json')).read_text())
        print(prefix, data.get('total_count'), flush=True)
        for index, item in enumerate(data.get('items', []), 1):
            jobs.append((prefix, index, item))
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        for result in pool.map(fetch_code, jobs):
            print(result, flush=True)
    for prefix in ['maintainers-validator', 'maintainers-parsebody', 'maintainers-malformed', 'maintainers-pretty', 'maintainers-override', 'later-validator', 'later-malformed', 'later-pretty']:
        query = json.loads((ROOT / (prefix + '.request.json')).read_text())['command']
        query = next(x for x in query if x.startswith('q='))[2:]
        api(prefix + '-all', 'search/issues', {'q': query, 'per_page': 100})
        data = json.loads((ROOT / (prefix + '-all.json')).read_text())
        print(prefix + '-all', data.get('total_count'), flush=True)
        for item in data.get('items', []):
            if item.get('comments') and (item['number'] in [4109, 3707, 4062, 2675, 1441, 1333, 3300, 3952, 5226, 5377] or 'case' in (item.get('title') or '').lower()):
                print(api(prefix + '-comments-' + str(item['number']), 'repos/honojs/hono/issues/' + str(item['number']) + '/comments', {'per_page': 100}), flush=True)
