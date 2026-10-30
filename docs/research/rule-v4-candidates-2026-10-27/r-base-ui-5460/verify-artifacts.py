import hashlib,json,re
from pathlib import Path
root=Path(__file__).resolve().parent
summary=json.loads((root/'summary.json').read_text())
checks=[]
def check(name,condition):
 if not condition:raise AssertionError(name)
 checks.append(name)
for row in summary:
 for folder in ['dossiers','cases','cards']:
  check(folder+' exists',(root/folder/(row['group']+'.md')).stat().st_size>0)
for quote in json.loads((root/'quotations.json').read_text()):check('exact quote '+quote['id'],quote['quote'] in (root/quote['source']).read_text())
def observations(name):
 text=(root/'probes/M1'/name).read_text()
 check('14 passed '+name,'Tests  14 passed (14)' in text)
 return {(x['scenario'],x.get('phase')):x for line in text.splitlines() if 'OBSERVATION ' in line for x in [json.loads(line.split('OBSERVATION ',1)[1])]}
for version in ['base','head']:
 for suffix in ['', '-chromium-final']:
  rows=observations('result-'+version+suffix+'.txt')
  for scenario in ['direct-store','render-store']:
   first=rows[(scenario,'first')]
   check(version+suffix+scenario+' accepted',first['after']['value']=='a' and first['after']['valueCallbacks']==0)
   check(version+suffix+scenario+' validation',first['after']['validation']==(['a'] if version=='head' else []))
   check(version+suffix+scenario+' error',first['after']['error']==(None if version=='head' else 'Server error'))
   check(version+suffix+scenario+' flags',first['events']==[{'cancelable':False,'nativePrevented':False,'baseUIPrevented':True}])
  reject=rows[('direct-reject','first')]['after']
  check(version+suffix+' rejection',reject['value']=='' and reject['error']=='Server error' and reject['validation']==[])
  check(version+suffix+' toggle',rows[('documented-toggle',None)]['pressed']=='true')
for variant in json.loads((root/'probes/M1/matrix.json').read_text()):
 name=variant['variant']; rows=observations(name+'.txt');check('matrix exit '+name,variant['exit_code']==0)
 patched=name in ['m1-only','m1-r6','m1-r8','all']
 for scenario in ['direct-store','render-store']:
  check('matrix suppression '+name+scenario,rows[(scenario,'first')]['after']['validation']==([] if patched else ['a']))
  check('later update '+name+scenario,rows[(scenario,'later-code')]['after']['validation']==['code'])
 for placement in ['false','true']:check('restore limit '+name+placement,rows[('known-r8-baseline-'+placement,'Restore')]['after']['dirty'] is True)
card=(root/'cards/M1.md').read_text();case=(root/'cases/M1.md').read_text()
check('blinded card',not re.search(r'serious|other-material|minor-defect|recommendation|/home/|NC-|\bcomment-[a-f0-9]{8}\b',card,re.I))
check('neutral case',not re.search(r'minor-defect|recommendation|Delivery rule|Promised:|Delivered:|NC-|\bcomment-[a-f0-9]{8}\b',case,re.I))
check('last case heading',case.rsplit('\n## ',1)[1].startswith('Known only later'))
(root/'verification.json').write_text(json.dumps({'passed':True,'checks':checks},indent=2)+'\n')
print(str(len(checks))+' artifact checks passed')
