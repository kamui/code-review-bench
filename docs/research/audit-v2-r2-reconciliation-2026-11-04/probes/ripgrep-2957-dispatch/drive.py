"""Drive an interactive zsh on a pseudo-terminal and record what Tab does.

Usage: python3 -I drive.py <zsh-root> <scripts-dir> <scratch-dir>
<zsh-root>: a directory holding usr/bin/zsh, usr/lib/x86_64-linux-gnu/zsh/5.9 and
usr/share/zsh/functions (the Debian zsh 5.9-8 packages, extracted).
<scripts-dir>: holds base.zsh and head.zsh, made by gen.sh.
For each scenario and commit: start `zsh -i` with ZDOTDIR set to a fresh directory,
type `rg --generate=complete-z`, press Tab, print the edit buffer, press Tab
again, print the buffer again, then exit. Prints the startup output and both buffers.
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

SCEN = {
  "A-top-level-source-after-compinit":
    "autoload -Uz compinit && compinit -u -D\nsource {script}\n",
  "B-fpath-file-_rg-that-sources-script":
    "fpath=({wdir} $fpath)\nautoload -Uz compinit && compinit -u -D\n",
  "C-function-_rg-sourcing-script-called-at-startup":
    "autoload -Uz compinit && compinit -u -D\n_rg() {{ source {script} }}\n_rg\n",
  "D-compdef-unset-after-compinit-before-source":
    "autoload -Uz compinit && compinit -u -D\nunfunction compdef\nsource {script}\n",
  "E-compdef-unset-after-source":
    "autoload -Uz compinit && compinit -u -D\nsource {script}\nunfunction compdef\n",
  "H-compdef-replaced-by-a-stub-before-source":
    "autoload -Uz compinit && compinit -u -D\ncompdef() {{ : }}\nsource {script}\n",
  "F-script-installed-as-_ripgrep-on-fpath":
    "fpath=({cdir} $fpath)\nautoload -Uz compinit && compinit -u -D\n",
  "G-script-installed-as-_rg-on-fpath":
    "fpath=({rdir} $fpath)\nautoload -Uz compinit && compinit -u -D\n",
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

def run(name, commit):
    d = os.path.join(SCRATCH, f"{name}.{commit}")
    shutil.rmtree(d, ignore_errors=True); os.makedirs(d)
    script = os.path.join(SCRIPTS, f"{commit}.zsh")
    wdir = os.path.join(d, "wrap"); os.makedirs(wdir)
    with open(os.path.join(wdir, "_rg"), "w") as f:
        f.write(f"#compdef rg\nsource {script}\n")
    cdir = os.path.join(d, "renamed"); os.makedirs(cdir)
    shutil.copy(script, os.path.join(cdir, "_ripgrep"))
    rdir = os.path.join(d, "plain"); os.makedirs(rdir)
    shutil.copy(script, os.path.join(rdir, "_rg"))
    with open(os.path.join(d, ".zshenv"), "w") as f:
        f.write(ENV.format(zr=ZROOT))
    with open(os.path.join(d, ".zshrc"), "w") as f:
        f.write(COMMON.format() + SCEN[name].format(script=script, wdir=wdir, cdir=cdir, rdir=rdir))
    env = {"HOME": d, "ZDOTDIR": d, "TERM": "xterm", "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(os.path.join(ZROOT, "usr/bin/zsh"), ["zsh", "-i"], env)
    out = read_until(fd, rb"PROMPT> ", 15)
    time.sleep(1.5)
    startup = out + read_until(fd, rb"$^", 0.5)
    os.write(fd, b"rg --generate=complete-z"); read_until(fd, rb"complete-z", 2)
    os.write(fd, b"\t"); time.sleep(1.0); read_until(fd, rb"$^", 0.5)
    os.write(fd, b"\x18\x02"); b1 = read_until(fd, rb"BUF<[^>]*>", 8)
    os.write(fd, b"\t"); time.sleep(1.0); read_until(fd, rb"$^", 0.5)
    os.write(fd, b"\x18\x02"); b2 = read_until(fd, rb"BUF<[^>]*>", 8)
    os.write(fd, b"\x15exit\r"); read_until(fd, rb"$^", 1.5)
    try: os.kill(pid, 9)
    except OSError: pass
    os.waitpid(pid, 0)
    clean = lambda b: re.sub(r"\x1b\[[0-9;?]*[A-Za-z]|\r|\x07", "", b.decode("utf-8", "replace"))
    m1 = re.search(r"BUF<([^>]*)>", clean(b1)); m2 = re.search(r"BUF<([^>]*)>", clean(b2))
    st = [l for l in clean(startup).split("\n") if l.strip() and "PROMPT>" not in l]
    print(f"== {name} @ {commit}")
    print("   startup output:", st if st else "(none)")
    print("   buffer after Tab 1:", repr(m1.group(1) if m1 else None))
    print("   buffer after Tab 2:", repr(m2.group(1) if m2 else None))

for trial in (1, 2, 3):
    print(f"# trial {trial}")
    for name in SCEN:
        for commit in ("base", "head"):
            run(name, commit)
