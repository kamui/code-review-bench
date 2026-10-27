import concurrent.futures, json, subprocess, base64
from pathlib import Path
from urllib.parse import quote
out=Path(__file__).resolve().parent/'upstream'
tasks=[]
for name,path in [('vercel-guide','src/content/docs/en/guides/integrations-guide/vercel.mdx'),('ondemand-guide','src/content/docs/en/guides/on-demand-rendering.mdx'),('middleware-guide','src/content/docs/en/guides/middleware.mdx')]:
 sha=json.loads((out/(name+'-history.json')).read_text())[0]['sha']
 tasks.append((name,'repos/withastro/docs/contents/'+path+'?ref='+sha))
nums=set()
for name in ['maintainers-isr-exclude','maintainers-cookies']:
 nums.update(x['number'] for x in json.loads((out/(name+'.json')).read_text())['items'])
for num in nums:tasks.append(('issue-'+str(num)+'-comments','repos/withastro/astro/issues/'+str(num)+'/comments?per_page=100'))
items=[x for x in json.loads((out/'public-code.json').read_text())['items'] if x['repository']['full_name']!='kamui/code-review-bench'][:8]
for i,x in enumerate(items):
 repo=x['repository']['full_name'];path=x['path'];ref=x['html_url'].split('/blob/')[1].split('/')[0]
 tasks.append(('code-'+str(i),'repos/'+repo+'/contents/'+quote(path)+'?ref='+ref))
 tasks.append(('code-'+str(i)+'-date','repos/'+repo+'/commits?path='+quote(path)+'&sha='+ref+'&per_page=1'))
 tasks.append(('code-'+str(i)+'-cutoff','repos/'+repo+'/commits?path='+quote(path)+'&until=2026-03-25T12:02:24Z&per_page=1'))
(out/'public-code-selected.json').write_text(json.dumps(items,indent=2))
def run(x):
 name,ep=x;r=subprocess.run(['gh','api',ep],capture_output=True,text=True)
 (out/(name+'.json')).write_text(r.stdout)
 (out/(name+'.request.txt')).write_text('gh api '+ep+'\nexit='+str(r.returncode)+'\n'+r.stderr)
 if r.returncode==0:
  d=json.loads(r.stdout)
  if isinstance(d,dict) and d.get('encoding')=='base64': (out/(name+'.txt')).write_bytes(base64.b64decode(d['content']))
 return name,r.returncode
with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
 for x in pool.map(run,tasks): print(*x)
