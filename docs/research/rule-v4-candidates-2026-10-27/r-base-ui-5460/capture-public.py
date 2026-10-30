import base64, concurrent.futures, json, subprocess, urllib.parse
from pathlib import Path
out=Path(__file__).parent / 'upstream'
items=json.loads((out/'public-code-focused.json').read_text())['items']
items=[x for x in items if x['path'].endswith(('.tsx','.ts')) and not x['path'].endswith('.d.ts') and not x['path'].startswith('submissions/')]
def capture(pair):
 i,x=pair; repo=x['repository']['full_name']; path=x['path']; stem='public-file-'+str(i)
 cmd=['gh','api','repos/'+repo+'/contents/'+urllib.parse.quote(path,safe='/')]
 r=subprocess.run(cmd,capture_output=True,text=True); (out/(stem+'.json')).write_text(r.stdout)
 if r.returncode:return {'repo':repo,'path':path,'error':r.stderr}
 j=json.loads(r.stdout); data=base64.b64decode(j['content']).decode(); (out/(stem+'.txt')).write_text(data)
 dates=subprocess.run(['gh','api','-X','GET','repos/'+repo+'/commits','-f','path='+path,'-f','per_page=1'],capture_output=True,text=True)
 (out/(stem+'-date.json')).write_text(dates.stdout)
 dj=json.loads(dates.stdout); date=dj[0]['commit']['committer']['date'] if isinstance(dj,list) and dj else None
 lines=data.splitlines(); excerpts=[]
 for n,line in enumerate(lines):
  if 'preventBaseUIHandler' in line: excerpts.append({'line':n+1,'text':'\n'.join(lines[max(0,n-8):n+9])})
 return {'repo':repo,'path':path,'sha':j['sha'],'date':date,'saved':stem+'.txt','excerpts':excerpts}
with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool: result=list(pool.map(capture,enumerate(items,1)))
(out/'public-code-read.json').write_text(json.dumps(result,indent=2)+'\n')
print('Read',len(result),'files')
