"""Select candidate paths: select-local.py INVENTORY_DIRECTORY NEW_OUTPUT_DIRECTORY."""
import gzip,json,collections,sys
from pathlib import Path
INVENTORIES=Path(sys.argv[1]);BASE=Path(sys.argv[2]);BASE.mkdir(exist_ok=False)
rows=[];summary=[];identities={};exclusions=collections.Counter()
for ip in sorted(INVENTORIES.glob('*.json.gz')):
 with gzip.open(ip,'rt') as f: inv=json.load(f)
 root=Path(inv['root']);counts=collections.Counter();sizes=collections.Counter()
 for e in inv['entries']:
  parts=Path(e['path']).parts;reason=None
  if e['type']!='file': reason='link-or-special'
  elif e['class']=='account-state' or Path(e['path']).name in ('auth.json','.credentials.json','.netrc','.npmrc','.pypirc','credentials','id_rsa','id_ed25519'): reason='credentials-or-account-state'
  elif e['class']=='tracked-identical': reason='tracked-copy'
  elif any(p.endswith('.git') for p in parts): reason='source-mirror-held-locally'
  elif parts[:2] == ('.local','finish-it-7'): reason='separately-inventoried-worktree'
  elif Path(e['path']).name=='claude' and e['bytes']>10000000: reason='installed-client-binary' 
  elif e['class']=='rebuildable': reason='generated-or-git'
  elif any(p in ('clone','clone-cache','mirrors','node_modules','.venv','venv') for p in parts):reason='source-or-dependencies-held-locally'
  elif len(parts)>=2 and parts[0]=='public' and parts[1] in ('evidence','data'):reason='generated-explorer-copy'
  elif e['path']=='src/routeTree.gen.ts':reason='generated-route-tree'
  elif 'home' in parts:
   tail=parts[parts.index('home')+1:]
   if not (len(tail)>=2 and ((tail[0]=='.codex' and (tail[1]=='sessions' or tail[1]=='config.toml')) or (tail[0]=='.claude' and (tail[1]=='projects' or tail[1] in ('settings.json','settings.local.json'))))):reason='unselected-home-state'
  if reason:
   counts[reason]+=1;sizes[reason]+=e['bytes'];continue
  if not e['sha256']:raise ValueError(e['path'])
  row={'root':inv['root'],'path':e['path'],'sha256':e['sha256'],'bytes':e['bytes'],'mode':e['mode'],'inventory_class':e['class']}
  rows.append(row);identities.setdefault((e['sha256'],e['mode']),row)
  counts['selected']+=1;sizes['selected']+=e['bytes']
 summary.append({'root':str(root),'counts':counts,'bytes':sizes})
(BASE/'selected-candidates.json').write_text(json.dumps(rows,separators=(',',':'))+'\n')
(BASE/'selection-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({'files':len(rows),'unique':len(identities),'bytes':sum(r['bytes'] for r in rows),'unique_bytes':sum(r['bytes'] for r in identities.values()),'largest':sorted(identities.values(),key=lambda r:r['bytes'],reverse=True)[:15]},indent=2))
for r in summary:
 if r['counts']['selected']:print(r['root'],r['counts']['selected'],round(r['bytes']['selected']/2**20,1))
