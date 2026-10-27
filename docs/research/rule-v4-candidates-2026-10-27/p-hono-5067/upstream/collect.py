import concurrent.futures
import json
import pathlib
import subprocess
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent
REQUESTS = {
    'pr-5067-comments': 'repos/honojs/hono/issues/5067/comments?per_page=100',
    'pr-5067-review-comments': 'repos/honojs/hono/pulls/5067/comments?per_page=100',
    'pr-5067-commits': 'repos/honojs/hono/pulls/5067/commits?per_page=100',
    'issue-5060': 'repos/honojs/hono/issues/5060',
    'issue-5060-comments': 'repos/honojs/hono/issues/5060/comments?per_page=100',
    'pr-5131': 'repos/honojs/hono/pulls/5131',
    'pr-5131-files': 'repos/honojs/hono/pulls/5131/files',
    'release-v4.12.28': 'repos/honojs/hono/releases/tags/v4.12.28',
    'docs-cutoff': 'repos/honojs/website/commits?until=2026-07-01T09:39:48Z&per_page=1',
}
QUERIES = {
    'maintainers-validator': 'repo:honojs/hono "Content-Type" validator created:<=2026-07-01T09:39:48Z',
    'maintainers-parsebody': 'repo:honojs/hono "formData" "parseBody" created:<=2026-07-01T09:39:48Z',
    'maintainers-malformed': 'repo:honojs/hono "Malformed FormData" created:<=2026-07-01T09:39:48Z',
    'maintainers-pretty': 'repo:honojs/hono "prettyJSON" "Content-Type" created:<=2026-07-01T09:39:48Z',
    'maintainers-override': 'repo:honojs/hono "formData" "override" created:<=2026-07-01T09:39:48Z',
    'later-validator': 'repo:honojs/hono "validator" "boundary" created:>2026-07-01T09:39:48Z',
    'later-malformed': 'repo:honojs/hono "Malformed FormData" created:>2026-07-01T09:39:48Z',
    'later-pretty': 'repo:honojs/hono "prettyJSON" "Content-Type" created:>2026-07-01T09:39:48Z',
    'code-validator': '"validator" "multipart/form-data" language:TypeScript',
    'code-parsebody': '"parseBody" "formData" language:TypeScript',
    'code-override': '"spyOn" "formData" repo:honojs/hono',
    'code-pretty': '"prettyJSON" "Application/JSON" language:TypeScript',
}


def api(name, endpoint, fields=None):
    cmd = ['gh', 'api', '--method', 'GET', endpoint]
    for key, value in (fields or {}).items():
        cmd += ['-f', key + '=' + str(value)]
    result = subprocess.run(cmd, capture_output=True, text=True)
    (ROOT / (name + '.json')).write_text(result.stdout)
    (ROOT / (name + '.request.json')).write_text(json.dumps({
        'command': cmd, 'returncode': result.returncode, 'stderr': result.stderr,
    }, indent=2))
    return name, result.returncode


if __name__ == '__main__':
    jobs = [(name, endpoint, None) for name, endpoint in REQUESTS.items()]
    for name, query in QUERIES.items():
        jobs.append((name, 'search/code' if name.startswith('code-') else 'search/issues', {'q': query, 'per_page': 5}))
    with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
        for result in pool.map(lambda args: api(*args), jobs):
            print(*result, flush=True)
    query = 'query { repository(owner:"honojs",name:"hono") { pullRequest(number:5067) { updatedAt userContentEdits(first:100) { nodes { createdAt editedAt diff deletedAt } pageInfo { hasNextPage } } } } }'
    result = subprocess.run(['gh', 'api', 'graphql', '-f', 'query=' + query], capture_output=True, text=True)
    (ROOT / 'pr-5067-edits.graphql.json').write_text(result.stdout)
    (ROOT / 'pr-5067-edits.request.json').write_text(json.dumps({'query': query, 'returncode': result.returncode, 'stderr': result.stderr}, indent=2))
    print('edits', result.returncode, flush=True)
    for name in ['rfc9110', 'rfc7578']:
        url = 'https://www.rfc-editor.org/rfc/' + name + '.txt'
        try:
            (ROOT / (name + '.txt')).write_bytes(urllib.request.urlopen(url).read())
            (ROOT / (name + '.request.json')).write_text(json.dumps({'url': url}))
            print(name, 'ok', flush=True)
        except Exception as error:
            print(name, str(error), flush=True)
