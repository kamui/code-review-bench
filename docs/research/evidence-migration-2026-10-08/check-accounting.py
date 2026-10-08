"""Read controlling ledgers: check-accounting.py CANDIDATES_JSON OUTPUT_JSON."""
import json,sys,subprocess
from pathlib import Path
sys.path.insert(0,str(Path.cwd()/'bench/tools'))
import regrade
candidates=Path(sys.argv[1])
queues={}
for row in json.loads(candidates.read_text()):
 if Path(row['path']).name!='reservation.json':continue
 path=Path(row['root'])/row['path'];parts=path.parts
 if 'batches' not in parts:continue
 q=Path(*parts[:parts.index('batches')]);queues[q]=Path(row['root'])
records=[]
for q,root in sorted(queues.items()):
 regrade.ROOT=root
 try:
  used,pending=regrade.ledger(q)
  result={'settled_upper_usd':str(used),'outstanding':[None if n is None else str(n) for n in pending]}
 except Exception as error:
  result={'blocked':type(error).__name__,'reason':str(error)}
 reservations=list(q.glob('batches/*/*/attempt-*/reservation.json'))
 records.append({'queue':str(q),'controller':str(root),'reservation_count':len(reservations),'ledger':result})
Path(sys.argv[2]).write_text(json.dumps(records,indent=2)+'\n')
print(json.dumps({'queues':len(records),'reservations':sum(r['reservation_count'] for r in records),'blocked':sum('blocked' in r['ledger'] for r in records),'outstanding':sum(len(r['ledger'].get('outstanding',[])) for r in records)}))
