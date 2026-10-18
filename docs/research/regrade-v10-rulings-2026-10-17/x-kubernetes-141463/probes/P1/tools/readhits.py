# usage: python3 -I readhits.py <out.json> <search result json>...
# Fetches each hit's file at its latest commit before the cut-off and lists lines that put a literal '%' (written "%%"
# in a format string) into a scheduler status or extender message, with the commit date.
import json, subprocess, sys, base64, re
out, files = sys.argv[1], sys.argv[2:]
hits = {}
for f in files:
    for it in json.load(open(f)).get('items', []):
        hits[(it['repository']['full_name'], it['path'])] = it['html_url']
res = []
for (repo, path), url in sorted(hits.items()):
    cm = json.loads(subprocess.run(['gh', 'api', '-X', 'GET', f'repos/{repo}/commits', '-f', f'path={path}', '-f', 'until=2026-09-09T13:25:49Z', '-f', 'per_page=1'], capture_output=True, text=True).stdout or '[]')
    if not cm:
        res.append({'repo': repo, 'path': path, 'url': url, 'commit': None, 'note': 'no commit before the cut-off'}); continue
    sha, date = cm[0]['sha'], cm[0]['commit']['committer']['date']
    first = json.loads(subprocess.run(['gh', 'api', '-X', 'GET', f'repos/{repo}/commits', '-f', f'path={path}', '-f', 'per_page=100'], capture_output=True, text=True).stdout or '[]')
    oldest = first[-1]['commit']['committer']['date'] if first else None
    c = json.loads(subprocess.run(['gh', 'api', f'repos/{repo}/contents/{path}?ref={sha}'], capture_output=True, text=True).stdout)
    text = base64.b64decode(c['content']).decode('utf-8', 'replace').splitlines()
    lines = []
    for i, l in enumerate(text):
        if '%%' in l:
            window = '\n'.join(text[max(0, i-4):i+5])
            if re.search(r'NewStatus|Unschedulable|FailedNodes|AsStatus', window):
                lines.append({'line': i+1, 'text': l.strip()})
    res.append({'repo': repo, 'path': path, 'url': url, 'commit': sha, 'commit_date': date, 'oldest_commit_on_page': oldest, 'status_lines_with_percent': lines})
json.dump(res, open(out, 'w'), indent=1, ensure_ascii=False)
for r in res:
    print(r['repo'], r['path'], r.get('commit_date'), len(r.get('status_lines_with_percent') or []))
    for l in r.get('status_lines_with_percent') or []: print('    ', l['line'], l['text'][:160])
