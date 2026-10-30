import subprocess, json, difflib, shlex, itertools
from pathlib import Path
from interactive import run,SCRATCH
ROOT=Path(__file__).resolve().parent
text=(ROOT/'head-generated.zsh').read_text()
guard='if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then'
fixes={
'M1': lambda s:s.replace(guard, 'if [[ $_RG_COMPLETE_LIST_ARGS == (1|t*|y*) ]] || [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then'),
'GT-n4':lambda s:s.replace('[[ $funcstack[1] == _rg ]]','[[ $funcstack[1] == _rg && $zsh_eval_context[-1] != file ]]'),
'GT-n5':lambda s:s.replace('[[ $funcstack[1] == _rg ]]','[[ $funcstack[1] == _rg || $zsh_eval_context[-1] == loadautofunc ]]'),
'M2':lambda s:s.replace('return ret','return $ret'),
}
for candidate,against in itertools.product(['M1','M2'],['GT-n4','GT-n5']):
 directory=ROOT/candidate/'grouping'/against;directory.mkdir(parents=True,exist_ok=True)
 rows=[]
 for variant,changes in [('head',[]),('candidate-only',[candidate]),('known-only',[against]),('both',[candidate,against])]:
  patched=text
  for change in changes:patched=fixes[change](patched)
  script=SCRATCH/'variants'/candidate/against/variant/'_rg';script.parent.mkdir(parents=True,exist_ok=True);script.write_text(patched)
  if changes:(directory/f'{variant}.patch').write_text(''.join(difflib.unified_diff(text.splitlines(True),patched.splitlines(True),fromfile='a/crates/core/flags/complete/rg.zsh',tofile='b/crates/core/flags/complete/rg.zsh')))
  if candidate=='M1':
   code='autoload -Uz compinit; compinit -D\n_RG_COMPLETE_LIST_ARGS=1 source '+str(script)+'\n'
   r=subprocess.run(['zsh','-dfc',code],capture_output=True,text=True)
   candidate_result=dict(spec_lines=len(r.stdout.splitlines()),exit=r.returncode,stderr=r.stderr)
  else:
   setup=['alias -g ret=XX','source '+shlex.quote(str(script)),\
    "zstyle ':completion:*' completer _complete _approximate",\
    "zstyle ':completion:*:approximate:*' max-errors 2 numeric"]
   candidate_result=run(setup,'rg --genrate',candidate+'-'+against+'-'+variant)
  if against=='GT-n4':
   code='autoload -Uz compinit; compinit -D\nunset \'_comps[rg]\'\ncd '+str(script.parent)+'\nsource _rg\nprint -r -- "source_status=$? registered=${_comps[rg]-NONE}"\n'
   r=subprocess.run(['zsh','-dfc',code],capture_output=True,text=True)
   known_result=dict(exit=r.returncode,stdout=r.stdout,stderr=r.stderr)
  else:
   (script.parent/'_ripgrep').write_text(patched)
   setup=['fpath=('+shlex.quote(str(script.parent))+' $fpath); autoload -Uz _ripgrep; compdef _ripgrep rg']
   known_result=run(setup,'rg --generate=complete-z',candidate+'-'+against+'-'+variant+'-known')
  row=dict(variant=variant,candidate=candidate_result,known=known_result);rows.append(row)
  print(candidate,against,variant,'candidate',candidate_result.get('spec_lines',candidate_result.get('buffer')),'known',known_result.get('stdout',known_result.get('buffer')),flush=True)
 (directory/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
