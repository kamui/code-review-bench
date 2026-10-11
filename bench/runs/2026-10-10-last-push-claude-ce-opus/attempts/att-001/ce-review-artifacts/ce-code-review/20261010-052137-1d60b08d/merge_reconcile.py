import json,sys,re
rd=sys.argv[1]
def norm(t): return re.sub(r'\s+',' ',t).strip().lower()
m1=json.load(open(rd+'/mechanical-findings.json'))
smap=json.load(open(rd+'/source-detail-map.json'))
F='src/requests/adapters.py'
INJ='src/requests/adapters.py:95 -- pool_kwargs["ssl_context"] = _preloaded_ssl_context'
# cluster -> helper pass-1 stable numbers (primary) ; fast-pass suppressed candidates named by title
clusters=[
 dict(key='A',nums=[1,2,3,4,5,6,11],fp=[],detail=('security',0),severity='P0',confidence=100,line=95,autofix_class='manual',
      title='Client certificate loaded into the shared SSLContext is presented by every later verify=True request',
      first_evidence=INJ),
 dict(key='B',nums=[12,14,15,16,17,18,20],fp=['preloaded sslcontext injected into pool kwargs for every verify=true request may override adapter-level tls settings'],detail=('api-contract',1),severity='P1',confidence=100,line=95,autofix_class='manual',
      title='HTTPAdapter subclass ssl_context and TLS-version settings are replaced by the preloaded context when verify=True',
      first_evidence=INJ),
 dict(key='C',nums=[8,13,19,28],fp=[],detail=('api-contract',2),severity='P1',confidence=100,line=95,autofix_class='manual',
      title='http:// request through an https:// proxy now raises a raw ValueError under verify=True',
      first_evidence=INJ),
 dict(key='D',nums=[7,21,23,25,26,27],fp=[],detail=('api-contract',3),severity='P1',confidence=100,line=304,autofix_class='manual',
      title='verify=True falls back to the OS trust store instead of certifi on the https-proxy hop and for direct cert_verify() callers',
      first_evidence='src/requests/adapters.py:304 -- if verify is not True:'),
 dict(key='E',nums=[9,10,22,24,29],fp=['ca bundle is loaded at module import with no error handling'],detail=('api-contract',4),severity='P1',confidence=100,line=76,autofix_class='manual',
      title='`import requests` fails when the default CA bundle cannot be loaded or ssl is unavailable',
      first_evidence='src/requests/adapters.py:76 -- _preloaded_ssl_context.load_verify_locations('),
 dict(key='F',nums=[30],fp=[],detail=('testing',4),severity='P2',confidence=75,line=94,autofix_class='gated_auto',
      title='TLS trust-path rewiring ships with no test additions',
      first_evidence='src/requests/adapters.py:94 -- elif verify is True:'),
]
by_num={f['#']:f for f in m1['findings']}
assert sorted(n for c in clusters for n in c['nums'])==list(range(1,31))
arts={}
def art(rv):
    if rv not in arts: arts[rv]=json.load(open(f'{rd}/{rv}.json'))
    return arts[rv]
synthetic=[];work={}
for c in clusters:
    reviewers=[];indep=[];keys=[];ev=[];sources=[]
    for n in c['nums']:
        f=by_num[n]
        for r in f['reviewers']:
            if r not in reviewers: reviewers.append(r)
            k='|'.join((r,f['file'],str(f['line']),norm(f['title'])))
            assert k in smap,k
            keys.append(k)
            for e in smap[k]['evidence']:
                if e not in ev: ev.append(e)
        for r in f['independent_reviewers']:
            if r not in indep: indep.append(r)
        sources.append({'pass1_#':n,'reviewers':f['reviewers'],'severity':f['severity'],'confidence':f['confidence'],'line':f['line'],'autofix_class':f['autofix_class'],'title':f['title']})
    for t in c['fp']:
        s=[x for x in m1['suppressed_findings'] if norm(x['title'])==t]
        assert len(s)==1,t
        reviewers.append('fast-pass')
        sources.append({'pass1_#':None,'reviewers':['fast-pass'],'severity':s[0]['severity'],'confidence':s[0]['confidence'],'line':s[0]['line'],'autofix_class':s[0]['autofix_class'],'title':s[0]['title'],'note':'suppressed at anchor 50; semantic duplicate, never counts toward promotion'})
    d=art(c['detail'][0])['findings'][c['detail'][1]]
    sf={'title':c['title'],'severity':c['severity'],'file':F,'line':c['line'],'confidence':c['confidence'],
        'autofix_class':c['autofix_class'],'owner':'downstream-resolver','requires_verification':True,'pre_existing':False,
        'first_evidence':c['first_evidence'],'suggested_fix':d['suggested_fix'],'evidence':ev,
        'reviewers':reviewers,'independent_reviewers':indep}
    synthetic.append(sf)
    work['|'.join((F,str(c['line']),norm(c['title'])))]={'cluster':c['key'],'reviewers':reviewers,'independent_reviewers':indep,'source_keys':keys,
        'why_it_matters':d['why_it_matters'],'detail_source':'%s[%d]'%c['detail'],'evidence':ev,'merged_from':sources}
json.dump([{'reviewer':'synthesis','findings':synthetic,'residual_risks':[],'testing_gaps':[]}],open(rd+'/merge-input-2.json','w'),indent=1)
json.dump(work,open(rd+'/merge-working-map.json','w'),indent=1)
print('synthetic',len(synthetic),'evidence sizes',[ (len(s['evidence']),sum(map(len,s['evidence']))) for s in synthetic])
