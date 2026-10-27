import hashlib
import json
from pathlib import Path
import re

OUT = Path(__file__).resolve().parents[2]
ROOT = OUT.parents[3]
sources = []


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for child in value.values():
            yield from strings(child)
    elif isinstance(value, list):
        for child in value:
            yield from strings(child)


for path in (OUT / 'upstream').iterdir():
    if path.suffix not in ['.json', '.txt', '.md', '.mdx', '.diff']:
        continue
    content = path.read_text().replace('\r\n', '\n')
    if path.suffix == '.json':
        try:
            sources.extend((str(path.relative_to(OUT)), text.replace('\r\n', '\n'))
                           for text in strings(json.loads(content)))
        except ValueError:
            pass
    sources.append((str(path.relative_to(OUT)), content))
packet = (ROOT / 'bench/targets/j-trpc-5017/packet.md').read_text()
sources.append(('task packet', packet))
body = packet.split('## 3. Pull-request body, verbatim\n\n```\n')[1].split('\n```')[0]
checks = []
for relative in ['dossiers/P1.md', 'cases/P1.md', 'cards/P1.md']:
    content = (OUT / relative).read_text()
    for quote in re.findall(r'^> (.+)$', content, re.M):
        if quote.startswith('**What happens:'):
            continue
        matches = [name for name, text in sources if quote in text]
        checks.append({'document': relative, 'quote': quote, 'exact_match': bool(matches),
                       'source': matches[0] if matches else None})
    for diff in re.findall(r'```diff\n(.*?)\n```', content, re.S):
        checks.append({'document': relative, 'quote': 'Diff excerpt',
                       'exact_match': diff in (OUT / 'upstream/change.diff').read_text()})
    if relative != 'cards/P1.md':
        checks.append({'document': relative, 'quote': 'Full task-packet body',
                       'exact_match': body in content, 'occurrences': content.count(body)})
manifest = json.loads((OUT / 'summary.json').read_text())
for entry in manifest:
    for relative in ['dossiers/' + entry['group'] + '.md', entry['case'], entry['card'],
                     'probes/' + entry['group'] + '/result-base.txt',
                     'probes/' + entry['group'] + '/result-head.txt',
                     'probes/' + entry['group'] + '/environment.txt', 'REPORT.md']:
        checks.append({'artifact': relative, 'exists': (OUT / relative).is_file()})
(OUT / 'probes/P1/quotation-check.json').write_text(json.dumps(checks, indent=2, ensure_ascii=False) + '\n')
assert all(check.get('exact_match', check.get('exists', False)) for check in checks), checks
records = []
for path in sorted(OUT.rglob('*')):
    if path.is_file() and path.name != 'evidence-sha256.json':
        records.append({'path': str(path.relative_to(OUT)),
                        'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
(OUT / 'probes/P1/evidence-sha256.json').write_text(json.dumps(records, indent=2) + '\n')
print('Exact block quotations, diff excerpts, packet bodies and required artifacts verified.')
