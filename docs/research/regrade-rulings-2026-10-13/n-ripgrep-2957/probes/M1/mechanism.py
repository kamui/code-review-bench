#!/usr/bin/env python3
"""Show what zsh reports to code that runs inside a completion call.

Usage: mechanism.py

No ripgrep code is involved. One interactive `zsh -f` on a pseudo-terminal, an
empty HOME, three completion files in fpath and compinit run. Each place prints
`$funcstack`, `$zsh_eval_context`, whether `compstate` is set and whether
`compdef` exists, into a log file.
"""
import os, pty, select, sys, time, tempfile, shutil
def read_until_idle(fd, idle=0.5, limit=6.0):
    out=b""; start=time.time(); last=time.time()
    while time.time()-start<limit:
        r,_,_=select.select([fd],[],[],0.1)
        if r:
            try: c=os.read(fd,65536)
            except OSError: break
            if not c: break
            out+=c; last=time.time()
        elif time.time()-last>idle: break
    return out
def send(fd,d,idle=0.5):
    os.write(fd,d.encode()); return read_until_idle(fd,idle=idle)
work=tempfile.mkdtemp(prefix="m1-mech.")
home=os.path.join(work,"home"); comp=os.path.join(home,".zsh-complete"); os.makedirs(comp)
show='print -r -- "  [$1] funcstack=<$funcstack> zsh_eval_context=<$zsh_eval_context> compstate_set=$+compstate compdef=$+functions[compdef] args=<$*>" >> $HOME/log\n'
open(os.path.join(home,"show.zsh"),"w").write(show.replace("$1","LABEL"))
files={
 "_wsrc":"#compdef wsrc\nLABEL=wrapper-source; source <(cat $HOME/show.zsh)\n",
 "_wevl":"#compdef wevl\nLABEL=wrapper-eval; eval \"$(cat $HOME/show.zsh)\"\n",
 "_plain":"#compdef plain\nLABEL=autoloaded-file-itself\n"+show.replace("[$1]","[$LABEL]"),
}
open(os.path.join(home,"show.zsh"),"w").write(show.replace("[$1]","[$LABEL]"))
for n,c in files.items(): open(os.path.join(comp,n),"w").write(c)
env={"HOME":home,"ZDOTDIR":home,"PATH":"/usr/bin:/bin","TERM":"xterm","COLUMNS":"200","LINES":"50"}
pid,fd=pty.fork()
if pid==0: os.execve("/usr/bin/zsh",["zsh","-f","-i"],env)
read_until_idle(fd)
for l in ["PS1='%% '","fpath=(${fpath:#*vendor-completions*})","fpath=($HOME/.zsh-complete $fpath)","autoload -Uz compinit && compinit -D",
          "LABEL=toplevel-source; source $HOME/show.zsh", "LABEL=toplevel-eval; eval \"$(cat $HOME/show.zsh)\"",
          "f() { LABEL=source-inside-plain-function; source $HOME/show.zsh }; f"]:
    send(fd,l+"\n",0.4)
for cmd in ["wsrc ","wevl ","plain "]:
    send(fd,cmd,0.3); send(fd,"\t",1.0); send(fd,"\x15",0.3)
send(fd,"exit\n",0.3)
try: os.waitpid(pid,0)
except ChildProcessError: pass
print(open(os.path.join(home,"log")).read())
shutil.rmtree(work,ignore_errors=True)
