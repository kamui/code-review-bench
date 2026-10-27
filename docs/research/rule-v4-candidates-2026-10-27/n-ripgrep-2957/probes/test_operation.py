import subprocess, os, json, shutil, hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
SCRATCH=Path('/home/jack/.cache/rule-v4-candidates-scratch/n-ripgrep-2957')
REPO=SCRATCH/'repo'
for rev,sha in [('base','79cbe89deb1151e703f4d91b19af9cdcc128b765'),('head','855bfa6cdae4f4fe8762f892fc4957635397083e')]:
 subprocess.run(['git','checkout','--detach',sha],cwd=REPO,check=True,capture_output=True)
 (REPO/'target/debug').mkdir(parents=True,exist_ok=True)
 shutil.copy2(SCRATCH/f'rg-{rev}',REPO/'target/debug/rg')
 results=[]
 for context,text in [('empty',''),('stub','compdef() { :; }\n'),('initialized','autoload -Uz compinit\ncompinit -D\n')]:
  home=SCRATCH/'test-homes'/context;home.mkdir(parents=True,exist_ok=True);(home/'.zshenv').write_text(text)
  env=os.environ.copy();env.update(ZDOTDIR=str(home))
  cmd=['zsh',str(REPO/'ci/test-complete')]
  r=subprocess.run(cmd,cwd=REPO,env=env,capture_output=True,text=True)
  results.append(dict(context=context,startup=text,command=cmd,exit=r.returncode,stdout=r.stdout,stderr=r.stderr))
  extractor=(REPO/'ci/test-complete').read_text().split('\nmain() {')[0]
  code=extractor+'\nget_comp_args '+str(REPO/'crates/core/flags/complete/rg.zsh')
  x=subprocess.run(['zsh','-c',code],cwd=REPO,env=env,capture_output=True,text=True)
  results[-1].update(spec_lines=len(x.stdout.splitlines()),extract_exit=x.returncode,extract_stderr=x.stderr)
 (ROOT/'probes/M1'/f'result-{rev}.txt').write_text(json.dumps(results,indent=2)+'\n')
 print(rev,[(x['context'],x['exit'],x['spec_lines']) for x in results])
