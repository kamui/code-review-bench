import json,shlex
from pathlib import Path
from interactive import run,SCRATCH
ROOT=Path(__file__).resolve().parent
for candidate in ['M1','M2']:
 for known in ['GT-n4','GT-n5']:
  rows=[]
  for variant in ['head','candidate-only','known-only','both']:
   script=SCRATCH/'variants'/candidate/known/variant/'_rg'
   for name,way in ([('_rg','bare-source')] if known=='GT-n4' else [('_rg','source'),('_rg','eval'),('_other','source'),('_other','eval')]):
    if way=='bare-source':setup=['cd '+shlex.quote(str(script.parent)),'source _rg']
    else:
     command=('source '+shlex.quote(str(script))) if way=='source' else ('eval \"$(<'+shlex.quote(str(script))+')\"')
     setup=[name+'() { '+command+'; }','compdef '+name+' rg']
    d=run(setup,'rg --generate=complete-z',candidate+'-'+known+'-'+variant+'-'+name+'-'+way)
    rows.append(dict(variant=variant,way=way,function=name,result=d))
    print(candidate,known,variant,name,way,repr(d['buffer']),flush=True)
  suffix='-full' if known=='GT-n5' else ''
  (ROOT/candidate/'grouping'/(known+suffix)/'interactive-known-variants.json').write_text(json.dumps(rows,indent=2)+'\n')
