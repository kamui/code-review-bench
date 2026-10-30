import subprocess,os,json,shlex
from pathlib import Path
from interactive import run,SCRATCH
ROOT=Path(__file__).resolve().parent
REPO=SCRATCH/'repo'
source=REPO/'crates/core/flags/complete/rg.zsh'
original=source.read_text()
try:
 for against in ['GT-n4','GT-n5']:
  directory=ROOT/'M1/grouping'/against
  rows=[]
  for variant in ['head','candidate-only','known-only','both']:
   path=SCRATCH/'variants/M1'/against/variant/'_rg'
   source.write_text(path.read_text())
   for context in ['stub','initialized']:
    env=os.environ.copy();env['ZDOTDIR']=str(SCRATCH/'test-homes'/context)
    r=subprocess.run(['zsh',str(REPO/'ci/test-complete')],cwd=REPO,env=env,capture_output=True,text=True)
    rows.append(dict(variant=variant,context=context,exit=r.returncode,stdout=r.stdout,stderr=r.stderr))
  (directory/'full-test-results.json').write_text(json.dumps(rows,indent=2)+'\n')
finally:source.write_text(original)
rows=[]
for rev in ['base','head']:
 script=ROOT/f'{rev}-generated.zsh'
 setup=['alias -g ret=XX','source '+shlex.quote(str(script)), 'compdef _rg rg',\
  "zstyle ':completion:*' completer _complete _approximate",\
  "zstyle ':completion:*:approximate:*' max-errors 2 numeric"]
 d=run(setup,'rg --genrate',rev+'-source-explicit-registration');rows.append(d)
 print(d['name'],repr(d['buffer']),flush=True)
(ROOT/'M2/base-head-explicit-registration.json').write_text(json.dumps(rows,indent=2)+'\n')
for alias in [False,True]:
 directory=SCRATCH/'documented-way';directory.mkdir(exist_ok=True)
 (directory/'_rg').write_text((ROOT/'head-generated.zsh').read_text())
 setup=[]
 if alias:setup+=['alias -g ret=XX']
 setup+=['fpath=('+shlex.quote(str(directory))+' $fpath); compinit -D',\
  "zstyle ':completion:*' completer _complete _approximate",\
  "zstyle ':completion:*:approximate:*' max-errors 2 numeric"]
 d=run(setup,'rg --genrate','head-documented-fpath-alias'+str(alias))
 (ROOT/'M2'/('documented-fpath-alias'+str(alias)+'.json')).write_text(json.dumps(d,indent=2)+'\n')
 print(d['name'],repr(d['buffer']),flush=True)
