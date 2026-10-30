import os, pty, select, signal, time, json, shlex
from pathlib import Path
SCRATCH=Path('/home/jack/.cache/rule-v4-candidates-scratch/n-ripgrep-2957')
def run(setup, line, name, tabs=1):
 home=SCRATCH/'homes'/name;home.mkdir(parents=True,exist_ok=True)
 capture=home/'buffer.txt'
 capture.unlink(missing_ok=True)
 pid,fd=pty.fork()
 if pid==0:
  os.environ.update(ZDOTDIR=str(home),TERM='xterm',LC_ALL='C',PROMPT='READY> ',PS1='READY> ')
  os.chdir(home)
  os.execv('/usr/bin/zsh',['zsh','-d','-f','-i'])
 transcript=b''
 def drain(seconds=0.25):
  nonlocal transcript
  end=time.monotonic()+seconds;chunk=b''
  while time.monotonic()<end:
   r,_,_=select.select([fd],[],[],max(0,end-time.monotonic()))
   if r:
    try:data=os.read(fd,65536)
    except OSError:break
    chunk+=data
  transcript+=chunk
  return chunk
 try:
  drain()
  common=["PROMPT='READY> '","autoload -Uz compinit; compinit -D", "unset '_comps[rg]'", "capture_line() { print -r -- \"$BUFFER\" > "+shlex.quote(str(capture))+"; zle send-break; }", "zle -N capture_line; bindkey '^X^B' capture_line"]
  for command in common+setup:
   os.write(fd,(command+'\n').encode());drain()
  os.write(fd,line.encode())
  for _ in range(tabs):os.write(fd,b'\t');drain(0.4)
  os.write(fd,b'\x18\x02');drain()
  deadline=time.monotonic()+5
  while not capture.exists() and time.monotonic()<deadline:drain(0.2)
  result=capture.read_text().strip() if capture.exists() else None
  os.write(fd,b'exit\n');drain()
 finally:
  try:os.kill(pid,signal.SIGKILL)
  except ProcessLookupError:pass
  os.waitpid(pid,0);os.close(fd)
 return dict(name=name,line=line,tabs=tabs,buffer=result,transcript=transcript.decode(errors='replace'))
if __name__=='__main__':
 root=Path(__file__).resolve().parent
 for rev in ['base','head']:
  script=root/f'{rev}-generated.zsh'
  for alias in [False,True]:
   for way in ['source','fpath']:
    directory=SCRATCH/'completions'/rev;directory.mkdir(parents=True,exist_ok=True);(directory/'_rg').write_text(script.read_text())
    setup=[]
    if alias:setup+=['alias -g ret=XX']
    if way=='source':setup+=['source '+shlex.quote(str(script))]
    else:setup+=['fpath=('+shlex.quote(str(directory))+' $fpath); autoload -Uz _rg; compdef _rg rg']
    setup+=["zstyle ':completion:*' completer _complete _approximate","zstyle ':completion:*:approximate:*' max-errors 2 numeric"]
    for line in ['rg --generate=complete-z','rg --genrate']:
     name=f'{rev}-{way}-alias{int(alias)}-'+('typo' if line.endswith('genrate') else 'normal')
     d=run(setup,line,name);(root/'M2'/f'{name}.json').write_text(json.dumps(d,indent=2)+'\n');print(name,repr(d['buffer']))
