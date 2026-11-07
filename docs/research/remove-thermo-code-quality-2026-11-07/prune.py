"""Prune bench/grading/current records for the runs that left bench/scoreboard.current.json (#444).

Usage: python3 -I prune.py <repo> <git-rev-with-old-registry> [--write]
"""
import json, subprocess, sys
from pathlib import Path

repo, old_rev, write = Path(sys.argv[1]), sys.argv[2], '--write' in sys.argv
current = repo / 'bench/grading/current'

def runs(registry):
    return {source['run'] for source in registry['sources']}

old = json.loads(subprocess.check_output(['git', '-C', str(repo), 'show', f'{old_rev}:bench/scoreboard.current.json']))
new = json.loads((repo / 'bench/scoreboard.current.json').read_text())
removed = sorted(runs(old) - runs(new))
paths = tuple(f'bench/{run}/' for run in removed)
in_removed = lambda review: review['path'].startswith(paths)

def load(name): return json.loads((current / f'{name}.json').read_text())
grades, claims, adjudications, credits, candidates = (load(n) for n in ('grades', 'claims', 'adjudications', 'credits', 'candidates'))
dropped = {'removed_runs': removed}

kept = [b for b in grades['batches'] if b['run'] not in removed]
dropped['grades.batches'] = [f"{b['run']}/{b['target']}" for b in grades['batches'] if b['run'] in removed]
grades['batches'] = kept

orphan_claims, stripped_links = [], 0
kept_claims = []
for claim in claims['claims']:
    links = [link for link in claim['links'] if not in_removed(link['review'])]
    stripped_links += len(claim['links']) - len(links)
    if not links:
        orphan_claims.append(claim['id'])
        continue
    claim['links'] = links
    kept_claims.append(claim)
claims['claims'] = kept_claims
dropped['claims.links_stripped'] = stripped_links
dropped['claims'] = orphan_claims

dropped_credits = [r['id'] for r in credits['rulings'] if in_removed(r['review'])]
credits['rulings'] = [r for r in credits['rulings'] if not in_removed(r['review'])]
dropped['credits.rulings'] = dropped_credits

dropped_candidates, stripped_anchors = [], 0
kept_candidates = []
for candidate in candidates['candidates']:
    if candidate['source']['run'] in removed:
        dropped_candidates.append(candidate['id'])
        continue
    anchors = [a for a in candidate['anchors'] if not in_removed(a['review'])]
    stripped_anchors += len(candidate['anchors']) - len(anchors)
    candidate['anchors'] = anchors
    kept_candidates.append(candidate)
candidates['candidates'] = kept_candidates
dropped['candidates'] = dropped_candidates
dropped['candidates.anchors_stripped'] = stripped_anchors

gone = set(orphan_claims) | set(dropped_credits) | set(dropped_candidates)
dropped['adjudications.decisions'] = [d['id'] for d in adjudications['decisions'] if d['subject'] in gone]
adjudications['decisions'] = [d for d in adjudications['decisions'] if d['subject'] not in gone]

print(json.dumps({k: (len(v) if isinstance(v, list) else v) for k, v in dropped.items()}, indent=1))
if write:
    for name, value in (('grades', grades), ('claims', claims), ('adjudications', adjudications), ('credits', credits), ('candidates', candidates)):
        path = current / f'{name}.json'
        ascii_only = path.read_text().isascii()
        path.write_text(json.dumps(value, indent=2, ensure_ascii=ascii_only) + '\n')
    (Path(__file__).parent / 'dropped.v1.json').write_text(json.dumps(dropped, indent=2) + '\n')
