"""Read every first-page hit of a saved GitHub code search: fetch the file at the indexed commit,
look for the exact text the search could not match, and record the commit's date.

python3 -I read_code_hits.py <search name> <exact regex>
reads upstream/<search name>.json, writes upstream/code-hits/<search name>/<n>.txt (raw file)
and upstream/<search name>-read.json (one record per hit).
"""
import json
import pathlib
import re
import subprocess
import sys
import urllib.parse

OUT = pathlib.Path(__file__).resolve().parent
CUTOFF = '2026-07-01T09:39:48Z'
name, pattern = sys.argv[1], re.compile(sys.argv[2])
search = json.loads((OUT / (name + '.json')).read_text())
folder = OUT / 'code-hits' / name
folder.mkdir(parents=True, exist_ok=True)


def gh(*args):
    done = subprocess.run(['gh', 'api', *args], text=True, capture_output=True)
    return done.returncode, done.stdout


records = []
for number, hit in enumerate(search['items'], 1):
    repo = hit['repository']['full_name']
    commit = urllib.parse.parse_qs(urllib.parse.urlparse(hit['url']).query)['ref'][0]
    code, text = gh('-H', 'Accept: application/vnd.github.raw', 'repos/' + repo + '/contents/' + urllib.parse.quote(hit['path']) + '?ref=' + commit)
    (folder / (str(number) + '.txt')).write_text(text)
    code2, meta = gh('repos/' + repo + '/commits/' + commit, '--jq', '.commit.committer.date')
    # the newest commit touching the file at or before the cut-off, to date the code
    code3, before = gh('-X', 'GET', 'repos/' + repo + '/commits', '-f', 'path=' + hit['path'], '-f', 'until=' + CUTOFF, '-f', 'per_page=1', '--jq', '.[0].commit.committer.date // empty')
    lines = [str(i) + ': ' + line.strip()[:200] for i, line in enumerate(text.splitlines(), 1) if pattern.search(line)]
    records.append({'n': number, 'repo': repo, 'path': hit['path'], 'indexed_commit': commit,
                    'indexed_commit_date': meta.strip() or None, 'file_last_changed_by_cutoff': before.strip() or None,
                    'fetched': code == 0, 'exact_matches': lines})
    print(number, repo, hit['path'], meta.strip(), before.strip() or '-', len(lines), lines[:2])
(OUT / (name + '-read.json')).write_text(json.dumps({'search': name, 'exact_regex': pattern.pattern, 'cutoff': CUTOFF, 'hits': records}, indent=1) + '\n')
