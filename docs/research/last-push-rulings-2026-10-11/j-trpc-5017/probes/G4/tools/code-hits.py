"""Reads the saved code searches, and for every hit that is a source file fetches its history around the cut-off.
Writes G4-code-hits.json: per file, the newest commit before the cut-off (and the file as it was then), or the date it first appeared."""
import json, subprocess, glob, base64, urllib.parse, re
CUT = '2023-11-09T22:08:09Z'
def gh(*args):
    r = subprocess.run(['gh', 'api', *args], capture_output=True, text=True)
    return json.loads(r.stdout) if r.returncode == 0 and r.stdout.strip() else {'error': r.stderr.strip()[:300]}
hits = {}
for f in sorted(glob.glob('G4-code-q*.json'), key=lambda n: int(re.search(r'q(\d+)', n).group(1))):
    d = json.load(open(f))
    for i in d.get('items', []):
        key = (i['repository']['full_name'], i['path'])
        hits.setdefault(key, []).append(f)
out = []
for (repo, path), queries in hits.items():
    entry = {'repository': repo, 'path': path, 'queries': queries, 'source_file': bool(re.search(r'\.(m?ts|tsx)$', path))}
    if entry['source_file'] and repo != 'trpc/trpc' and '/packages/tests/server/regression/' not in '/' + path:
        p = urllib.parse.quote(path)
        before = gh(f'repos/{repo}/commits?path={p}&until={CUT}&per_page=1')
        allc = gh(f'repos/{repo}/commits?path={p}&per_page=100')
        if isinstance(allc, list) and allc:
            entry['commits_listed'] = len(allc)
            entry['oldest_listed_commit'] = {'sha': allc[-1]['sha'], 'date': allc[-1]['commit']['committer']['date']}
            entry['newest_commit'] = {'sha': allc[0]['sha'], 'date': allc[0]['commit']['committer']['date']}
        else:
            entry['history_error'] = allc if isinstance(allc, dict) else 'no commits listed'
        if isinstance(before, list) and before:
            sha = before[0]['sha']
            entry['newest_commit_before_cutoff'] = {'sha': sha, 'date': before[0]['commit']['committer']['date']}
            c = gh(f'repos/{repo}/contents/{p}?ref={sha}')
            entry['content_before_cutoff'] = base64.b64decode(c['content']).decode('utf-8', 'replace') if 'content' in c else c
        else:
            entry['newest_commit_before_cutoff'] = None
        cur = gh(f'repos/{repo}/contents/{p}')
        entry['content_now'] = base64.b64decode(cur['content']).decode('utf-8', 'replace') if 'content' in cur else cur
    out.append(entry)
json.dump(out, open('G4-code-hits.json', 'w'), indent=1)
for e in out:
    print(('SRC ' if e['source_file'] else 'doc ') + e['repository'], e['path'], '| before cut-off:', (e.get('newest_commit_before_cutoff') or {}).get('date'), '| first listed:', (e.get('oldest_listed_commit') or {}).get('date'))
