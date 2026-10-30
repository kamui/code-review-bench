import subprocess,json
from pathlib import Path
ROOT=Path(__file__).resolve().parent
for rev in ['base','head']:
 rows=[]
 for alias in [False,True]:
  script=ROOT/f'{rev}-generated.zsh'
  code='autoload -Uz compinit; compinit -D\n'
  if alias:code+='alias -g ret=XX\n'
  code+='source '+str(script)+'\n'
  code+='print -r -- \"definition: ${${(f)functions[_rg]}[-1]}\"\n'
  code+='_arguments() { return 1; }\n_rg\nprint -r -- \"failing-arguments status=$?\"\n'
  r=subprocess.run(['zsh','-dfc',code],capture_output=True,text=True)
  rows.append(dict(alias=alias,command=['zsh','-dfc',code],exit=r.returncode,stdout=r.stdout,stderr=r.stderr))
 (ROOT/'M2'/f'result-{rev}.txt').write_text(json.dumps(rows,indent=2)+'\n')
 print(rev,[(x['alias'],x['stdout']) for x in rows])
