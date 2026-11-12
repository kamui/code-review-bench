"""Drive an interactive zsh on a pseudo-terminal under KSH_ARRAYS or nounset/no_eval_lineno.

Usage: python3 -I drive.py <zsh-root> <scripts-dir> <scratch-dir>
<zsh-root>: the extracted Debian zsh 5.9-8 packages (usr/bin/zsh, modules, functions).
<scripts-dir>: base.zsh and head.zsh, made by round 2's gen.sh
(docs/research/audit-v2-r2-reconciliation-2026-11-04/probes/ripgrep-2957-dispatch/gen.sh).
Each scenario starts `zsh -i` with ZDOTDIR set to a fresh directory, records what start-up
prints and the registration for rg, optionally types a command at the prompt, then types
`rg --generate=complete-z`, presses Tab, prints the buffer, presses Tab again and prints it.
"""
import os, pty, select, sys, time, shutil, re

ZROOT, SCRIPTS, SCRATCH = sys.argv[1:4]

ENV = """module_path=({zr}/usr/lib/x86_64-linux-gnu/zsh/5.9)
fpath=({zr}/usr/share/zsh/functions/**/*(N/))
"""

COMMON = """PS1='PROMPT> '
_show() {{ print -r -- $'\\n'"BUF<$BUFFER>" }}
zle -N _show
bindkey '^X^B' _show
"""

REG = 'print -r -- "REG<${_comps[rg]-none}> STATUS<$?>"'

# name: (.zshrc tail, command typed at the prompt or None)
SCEN = {
  "K1-ksh_arrays-source-after-compinit":
    ("autoload -Uz compinit && compinit -u -D\nsetopt ksh_arrays\nsource {script}\n" + REG + "\n", None),
  "K2-ksh_arrays-before-compinit-source-after":
    ("setopt ksh_arrays\nautoload -Uz compinit && compinit -u -D\nsource {script}\n" + REG + "\n", None),
  "K3-ksh_arrays-script-as-_rg-on-fpath":
    ("fpath=({rdir} $fpath)\nsetopt ksh_arrays\nautoload -Uz compinit && compinit -u -D\n", None),
  "K4-ksh_arrays-script-as-_rg-on-fpath-compinit-first":
    ("fpath=({rdir} $fpath)\nautoload -Uz compinit && compinit -u -D\nsetopt ksh_arrays\n", None),
  "K5-emulate-ksh-script-as-_rg-on-fpath":
    ("fpath=({rdir} $fpath)\nautoload -Uz compinit && compinit -u -D\nemulate ksh\n", None),
  "K6-emulate-sh-script-as-_rg-on-fpath":
    ("fpath=({rdir} $fpath)\nautoload -Uz compinit && compinit -u -D\nemulate sh\n", None),
  "N1-nounset-no_eval_lineno-eval-in-zshrc":
    ("autoload -Uz compinit && compinit -u -D\nsetopt nounset no_eval_lineno\neval \"$(<{script})\"\nprint -r -- \"EVAL_STATUS<$?>\"\n" + REG + "\n", None),
  "N2-nounset-no_eval_lineno-eval-typed-at-prompt":
    ("autoload -Uz compinit && compinit -u -D\nsetopt nounset no_eval_lineno\n",
     "eval \"$(<{script})\"; print -r -- \"EVAL_STATUS<$?>\"; " + REG),
}

def read_until(fd, pat, timeout=8.0):
    buf = b""; end = time.time() + timeout
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            try: chunk = os.read(fd, 65536)
            except OSError: break
            if not chunk: break
            buf += chunk
            if re.search(pat, buf): break
    return buf

clean = lambda b: re.sub(r"\x1b\[[0-9;?]*[A-Za-z]|\r|\x07", "", b.decode("utf-8", "replace"))

def run(name, commit):
    d = os.path.join(SCRATCH, f"{name}.{commit}")
    shutil.rmtree(d, ignore_errors=True); os.makedirs(d)
    script = os.path.join(SCRIPTS, f"{commit}.zsh")
    rdir = os.path.join(d, "plain"); os.makedirs(rdir)
    shutil.copy(script, os.path.join(rdir, "_rg"))
    rc, typed = SCEN[name]
    with open(os.path.join(d, ".zshenv"), "w") as f:
        f.write(ENV.format(zr=ZROOT))
    with open(os.path.join(d, ".zshrc"), "w") as f:
        f.write(COMMON.format() + rc.replace("{script}", script).replace("{rdir}", rdir))
    env = {"HOME": d, "ZDOTDIR": d, "TERM": "xterm", "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(os.path.join(ZROOT, "usr/bin/zsh"), ["zsh", "-i"], env)
    out = read_until(fd, rb"PROMPT> ", 15)
    time.sleep(1.5)
    startup = out + read_until(fd, rb"$^", 0.5)
    typed_out = b""
    if typed:
        os.write(fd, typed.replace("{script}", script).encode() + b"\r")
        typed_out = read_until(fd, rb"REG<[^>]*> STATUS<\d+>", 8) + read_until(fd, rb"$^", 0.5)
    os.write(fd, b"rg --generate=complete-z"); read_until(fd, rb"complete-z", 2)
    os.write(fd, b"\t"); time.sleep(1.0); read_until(fd, rb"$^", 0.5)
    os.write(fd, b"\x18\x02"); b1 = read_until(fd, rb"BUF<[^>]*>", 8)
    os.write(fd, b"\t"); time.sleep(1.0); read_until(fd, rb"$^", 0.5)
    os.write(fd, b"\x18\x02"); b2 = read_until(fd, rb"BUF<[^>]*>", 8)
    os.write(fd, b"\x15exit\r"); read_until(fd, rb"$^", 1.5)
    try: os.kill(pid, 9)
    except OSError: pass
    os.waitpid(pid, 0)
    m1 = re.search(r"BUF<([^>]*)>", clean(b1)); m2 = re.search(r"BUF<([^>]*)>", clean(b2))
    sub = lambda s: s.replace(SCRATCH, "<scratch>").replace(SCRIPTS, "<scratch>")
    st = [sub(l) for l in clean(startup).split("\n") if l.strip() and "PROMPT>" not in l]
    print(f"== {name} @ {commit}")
    print("   startup output:", st if st else "(none)")
    if typed:
        tl = [sub(l) for l in clean(typed_out).split("\n") if l.strip() and "PROMPT>" not in l and not l.startswith("eval ")]
        print("   typed:", sub(typed.replace("{script}", "<scratch>/" + commit + ".zsh")))
        print("   typed output:", tl if tl else "(none)")
    print("   buffer after Tab 1:", repr(m1.group(1) if m1 else None))
    print("   buffer after Tab 2:", repr(m2.group(1) if m2 else None))

for trial in (1, 2, 3):
    print(f"# trial {trial}")
    for name in SCEN:
        for commit in ("base", "head"):
            run(name, commit)
