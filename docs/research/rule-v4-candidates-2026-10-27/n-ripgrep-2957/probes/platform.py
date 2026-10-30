import subprocess,json,shlex
from pathlib import Path
from interactive import run,SCRATCH
ROOT=Path(__file__).resolve().parent
rows=[]
gh_script=SCRATCH/'gh.zsh'
r=subprocess.run(['gh','completion','-s','zsh'],capture_output=True,text=True);gh_script.write_text(r.stdout)
(ROOT/'comparable-gh-generated.zsh').write_text(r.stdout)
for alias in [False,True]:
 for tool,line,extra in [('ls','ls --colr',[]),('gh','gh compltion',['source '+shlex.quote(str(gh_script))])]:
  setup=[]
  if alias:setup+=['alias -g ret=XX']
  setup+=extra+["zstyle ':completion:*' completer _complete _approximate","zstyle ':completion:*:approximate:*' max-errors 2 numeric"]
  d=run(setup,line,tool+'-alias'+str(alias));rows.append(d);print(tool,alias,repr(d['buffer']),flush=True)
(ROOT/'platform-results-retry.json').write_text(json.dumps(rows,indent=2)+'\n')
for context in ['empty','stub','initialized']:
 home=SCRATCH/'test-homes'/context
 import os
 env=os.environ.copy();env['ZDOTDIR']=str(home)
 r=subprocess.run(['zsh','-c','gh completion -s zsh'],env=env,capture_output=True,text=True)
 d=dict(context=context,exit=r.returncode,lines=len(r.stdout.splitlines()),stderr=r.stderr)
 (ROOT/'M1'/f'comparable-generator-{context}.json').write_text(json.dumps(d,indent=2)+'\n')
