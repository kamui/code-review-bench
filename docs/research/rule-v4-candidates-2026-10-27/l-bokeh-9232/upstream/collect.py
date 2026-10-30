import json
import subprocess
from pathlib import Path

OUT = Path(__file__).resolve().parent


def fetch(name, endpoint, query=None):
    args = ['gh', 'api', '--method', 'GET', endpoint]
    if query is not None:
        args += ['-f', 'q=' + query, '-f', 'per_page=100']
    result = subprocess.run(args, capture_output=True, text=True)
    (OUT / name).write_text(result.stdout)
    with (OUT / 'requests.jsonl').open('a') as log:
        log.write(json.dumps({'command': args, 'exit': result.returncode,
                              'stderr': result.stderr, 'saved': name}) + '\n')
    if result.returncode:
        print(name, 'BLOCKED', result.stderr.strip())
        return None
    data = json.loads(result.stdout)
    print(name, 'items=' + str(data.get('total_count')) if isinstance(data, dict) and 'total_count' in data else 'ok')
    return data


if __name__ == '__main__':
    for number in [9232, 9509]:
        for suffix, endpoint in [('', f'pulls/{number}'), ('-comments', f'issues/{number}/comments?per_page=100'), ('-reviews', f'pulls/{number}/comments?per_page=100'), ('-files', f'pulls/{number}/files?per_page=100')]:
            fetch(f'pr-{number}{suffix}.json', 'repos/bokeh/bokeh/' + endpoint)
    for number in [9129, 9494]:
        fetch(f'issue-{number}.json', f'repos/bokeh/bokeh/issues/{number}')
        fetch(f'issue-{number}-comments.json', f'repos/bokeh/bokeh/issues/{number}/comments?per_page=100')
    searches = {
        'maintainers-timezone': 'repo:bokeh/bokeh DatePicker timezone created:<=2019-10-03T04:32:25Z',
        'maintainers-historical': 'repo:bokeh/bokeh DatePicker (historical OR 1890 OR LMT) created:<=2019-10-03T04:32:25Z',
        'later-historical': 'repo:bokeh/bokeh DatePicker (historical OR 1890 OR LMT)',
        'later-offset': 'repo:bokeh/bokeh DatePicker getTimezoneOffset',
    }
    for name, query in searches.items():
        fetch('search-' + name + '.json', 'search/issues', query)
    fetch('search-public-code.json', 'search/code', '"DatePicker" "1890" language:Python')
    fetch('pikaday-commit.json', 'repos/bokeh/pikaday/commits/6b7258e')
    for path in ['README.md', 'pikaday.js']:
        fetch('pikaday-' + path.replace('.', '-') + '.contents.json', f'repos/bokeh/pikaday/contents/{path}?ref=6b7258e')
    for path in ['sphinx/source/docs/releases/1.4.0.rst', 'sphinx/source/docs/releases/2.0.0.rst']:
        fetch(path.split('/')[-1] + '.contents.json', f'repos/bokeh/bokeh/contents/{path}?ref={path.split("/")[-1][:-4]}')
    fetch('compare-to-1.4.0.json', 'repos/bokeh/bokeh/compare/ec1e525c9a8ad128e29b3b74beb732e79a0db437...1.4.0')
