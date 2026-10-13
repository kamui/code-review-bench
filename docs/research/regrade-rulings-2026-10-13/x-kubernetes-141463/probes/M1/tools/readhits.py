import json,sys,glob,subprocess,re,base64,os
S='<scratch>/codehits'
CUT='2026-09-09T13:25:49Z'
seen={}
queries={}
for f in sorted(glob.glob('M1-code-q*.json'), key=lambda x:int(re.search(r'q(\d+)',x).group(1))):
    d=json.load(open(f)); queries[f]=d['total_count']
    for it in d['items']:
        key=(it['repository']['full_name'], it['path'])
        seen.setdefault(key, {'sha':it['sha'],'queries':[]})['queries'].append(f)
nonascii=re.compile(r'[^\x00-\x7F]')
def gh(*a):
    r=subprocess.run(['gh','api',*a],capture_output=True,text=True)
    return json.loads(r.stdout) if r.returncode==0 else None
out=[]
for (repo,path),v in sorted(seen.items()):
    cache=os.path.join(S, (repo+'/'+path).replace('/','__'))
    if not os.path.exists(cache):
        b=gh(f'repos/{repo}/git/blobs/{v["sha"]}')
        if b is None: out.append({'repo':repo,'path':path,'error':'blob not fetched'}); continue
        open(cache,'wb').write(base64.b64decode(b['content']))
    lines=open(cache,'rb').read().decode('utf-8','replace').split('\n')
    hits=[]
    for i,l in enumerate(lines):
        lits=[x for x in re.findall(r'"(?:[^"\\]|\\.)*"', l) if nonascii.search(x)]
        if not lits: continue
        same=re.search(r'NewStatus\(|AsStatus\(', l) is not None
        cont=i>0 and re.search(r'NewStatus\([^)]*,\s*$', lines[i-1]) is not None
        if same or cont:
            code=re.search(r'framework\.(\w+)|fwk\.(\w+)', l if same else lines[i-1])
            hits.append({'line':i+1,'text':(lines[i-1].strip()+' ' if cont else '')+l.strip()})
    e={'repo':repo,'path':path,'blob_sha':v['sha'],'queries':v['queries'],'non_ascii_status_messages':hits}
    if hits:
        cpath=cache+'.commits.json'
        if not os.path.exists(cpath):
            c=gh('-X','GET',f'repos/{repo}/commits','-f',f'path={path}','-f',f'until={CUT}','-f','per_page=1')
            json.dump(c,open(cpath,'w'))
        c=json.load(open(cpath))
        if c:
            e['newest_commit_before_cutoff']={'sha':c[0]['sha'],'date':c[0]['commit']['committer']['date']}
            fpath=cache+'.atcommit'
            if not os.path.exists(fpath):
                r=subprocess.run(['gh','api','-X','GET',f'repos/{repo}/contents/{path}','-f',f'ref={c[0]["sha"]}'],capture_output=True,text=True)
                open(fpath,'wb').write(base64.b64decode(json.loads(r.stdout)['content']) if r.returncode==0 else b'')
            then=open(fpath,'rb').read().decode('utf-8','replace')
            e['messages_present_at_that_commit']=sum(1 for h in hits if h['text'].split(' ',1)[-1][-40:] in then or lines[h['line']-1].strip() in then)
        else:
            e['newest_commit_before_cutoff']=None
        r=gh(f'repos/{repo}')
        if r: e['repository']={'stars':r['stargazers_count'],'fork':r['fork'],'created_at':r['created_at'],'description':r['description']}
    out.append(e)
json.dump({'cutoff':CUT,'queries':queries,'files':out},open('M1-code-hits.json','w'),ensure_ascii=False,indent=1)
with_=[o for o in out if o.get('non_ascii_status_messages')]
print('queries',len(queries),'sum of totals',sum(queries.values()),'unique files',len(out),'files with a non-ASCII message passed to NewStatus/AsStatus',len(with_),'repositories',len({o['repo'] for o in with_}))
for o in with_:
    print(o['repo'],o['path'],'|',(o.get('newest_commit_before_cutoff') or {}).get('date'),'| present then:',o.get('messages_present_at_that_commit'),'of',len(o['non_ascii_status_messages']),'| stars',o.get('repository',{}).get('stars'),'fork',o.get('repository',{}).get('fork'))
    for h in o['non_ascii_status_messages'][:3]: print('     ',h['line'],h['text'][:170])
for o in out:
    if o.get('error'): print('ERR',o)
