import subprocess, json, base64, re
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
OUT=Path(__file__).resolve().parent
records={}
for name in ['public-code-zero','public-code-negative']:
    for item in json.loads((OUT/(name+'.json')).read_text())['items']:
        records[item['html_url']]=item

def fetch(item):
    repo=item['repository']['full_name']; sha=item['html_url'].split('/blob/')[1].split('/')[0]
    key=str(list(records).index(item['html_url'])+1)
    data={ 'url':item['html_url'], 'path':item['path'], 'ref':sha }
    for kind, endpoint in [('blob',item['git_url'].replace('https://api.github.com/','')), ('commit','repos/'+repo+'/commits/'+sha)]:
        r=subprocess.run(['gh','api',endpoint],text=True,capture_output=True)
        (OUT/('code-'+key+'-'+kind+'.json')).write_text(r.stdout)
        if r.returncode: data[kind+'_blocked']=r.stderr; continue
        d=json.loads(r.stdout)
        if kind=='blob':
            source=base64.b64decode(d['content']).decode(errors='replace')
            lines=source.splitlines(); indices=set()
            for n,line in enumerate(lines):
                if re.search('LoadReportingInterval|load_reporting_interval',line):
                    indices.update(range(max(0,n-2),min(len(lines),n+5)))
            data['excerpts']='\n'.join(str(n+1)+': '+lines[n] for n in sorted(indices))
        else: data['code_date']=d['commit']['committer']['date']
    return data
with ThreadPoolExecutor(max_workers=6) as pool:
    result=list(pool.map(fetch,records.values()))
(OUT/'public-code-read.json').write_text(json.dumps(result,indent=2)+'\n')
for r in result:
    print(r['url'],r.get('code_date'),r.get('blob_blocked',''))
    print(r.get('excerpts',''))
