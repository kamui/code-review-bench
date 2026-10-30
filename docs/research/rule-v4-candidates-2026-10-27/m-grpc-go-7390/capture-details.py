import json,subprocess,base64,concurrent.futures
from pathlib import Path
D=Path(__file__).parent/'upstream'
receipts=[]
def get(name,endpoint):
 r=subprocess.run(['gh','api',endpoint],capture_output=True,text=True)
 (D/(name+'.json')).write_text(r.stdout)
 receipts.append(dict(name=name,command=['gh','api',endpoint],exit=r.returncode,stderr=r.stderr))
 if r.returncode: return None
 return json.loads(r.stdout)
for n in [2669,2739,2740,1648,2960,1376,1444]:
 get(f'issue-{n}',f'repos/grpc/grpc-go/issues/{n}')
 get(f'issue-{n}-comments',f'repos/grpc/grpc-go/issues/{n}/comments?per_page=100')
 if n in [2669,2740,1648,2960,1376]: get(f'pr-{n}-files',f'repos/grpc/grpc-go/pulls/{n}/files?per_page=100')
for i,x in enumerate(json.loads((D/'public-code.json').read_text())['items'],1):
 repo=x['repository']['full_name']; path=x['path']; name=f'code-{i}'
 h=get(name+'-history',f'repos/{repo}/commits?path={path}&until=2024-07-09T06:40:48Z&per_page=1')
 if not h:continue
 obj=get(name,f'repos/{repo}/contents/{path}?ref={h[0]["sha"]}')
 if obj and 'content' in obj: (D/(name+'.go')).write_bytes(base64.b64decode(obj['content']))
(D/'detail-receipts.json').write_text(json.dumps(receipts,indent=2)+'\n')
print('Captured',len(receipts),'responses; failures:',[x for x in receipts if x['exit']])
