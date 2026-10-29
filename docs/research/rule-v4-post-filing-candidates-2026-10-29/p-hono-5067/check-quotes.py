"""Check that every double-quoted passage of 20 characters or more in the dossiers, cases and cards
appears in a saved source. python3 -I check-quotes.py <clone> > quote-check.txt

Sources: everything under upstream/ and probes/, the source and tests of the clone at both commits,
the rule file, and the ruling files of this pull request. Whitespace is collapsed and markup
(<tags>, {{ }}, ** and backticks) is removed on both sides before comparing.
"""
import json
import pathlib
import re
import subprocess
import sys

OUT = pathlib.Path(__file__).resolve().parent
REPO = OUT.parents[3]
clone = pathlib.Path(sys.argv[1])


def norm(text):
    text = re.sub(r'</?[a-zA-Z][^<>\n]{0,200}>', '', text)
    text = text.replace('{{', '').replace('}}', '').replace('**', '').replace('`', '')
    text = text.replace('\\"', '"').replace('\\n', ' ').replace('\\r', ' ')
    return re.sub(r'\s+', ' ', text).strip()


corpus = []
for path in list((OUT / 'upstream').rglob('*')) + list((OUT / 'probes').rglob('*')):
    if path.is_file():
        corpus.append(path.read_text(errors='replace'))
for commit in ['9728702911073aec5a63a3ba2840b7240e5d3205', '5226d4165d48643586152614cbd07422a0ab7a22']:
    for name in ['src/utils/body.ts', 'src/utils/body.test.ts', 'src/utils/buffer.ts', 'src/request.ts', 'docs/MIGRATION.md']:
        corpus.append(subprocess.run(['git', '-C', str(clone), 'show', commit + ':' + name], text=True, capture_output=True).stdout)
corpus.append((REPO / 'bench/rubric/rules.pre-existing.v3.md').read_text())
corpus.append((REPO / 'docs/research/impact-boundary-2026-10-04/impact-boundary.v4.md').read_text())
for path in (REPO / 'docs/research/rule-v4-candidates-2026-10-27/rulings').glob('1[3-9]-hono*'):
    corpus.append(path.read_text())
packet = json.loads((OUT / 'packet.json').read_text())
corpus.append(json.dumps(packet, ensure_ascii=False))
for family in packet['reference_families']:
    corpus.append(family['title'])
haystack = '\n'.join(norm(doc) for doc in corpus)

missing = 0
for folder in ['dossiers', 'cases', 'cards']:
    for path in sorted((OUT / folder).glob('*.md')):
        text = path.read_text()
        for quote in re.findall(r'"([^"\n]{20,})"', text):
            found = norm(quote) in haystack
            # a quotation that itself holds quoted words is split at them and each piece checked
            if not found:
                pieces = [p for p in re.split(r'\s*\.\.\.\s*', norm(quote)) if p]
                found = all(p in haystack for p in pieces)
            if not found:
                missing += 1
            print(('ok      ' if found else 'MISSING ') + folder + '/' + path.name + ': ' + quote[:150])
print('missing:', missing)
