#!/usr/bin/env python3
"""Drive a real interactive `zsh -f` through a pseudo-terminal and press Tab.

Usage: tab_probe.py /path/to/built/rg

Each scenario starts a new interactive `zsh -f` with an empty HOME and a
cleared environment, installs the generated completion script, runs compinit,
types `rg --generate=complete-z`, presses Tab, records the command line, presses
Tab again and records it again. A key bound to a small widget writes the
command line and the current rg completion registration to a log file, so the
result does not depend on parsing terminal escape codes.

The packaged completion directory is dropped from fpath because this machine
has a distribution ripgrep whose own `_rg` would otherwise answer for rg.
"""

import hashlib
import os
import pty
import select
import shutil
import subprocess
import sys
import tempfile
import time

TYPED = "rg --generate=complete-z"
EXPECTED = "rg --generate=complete-zsh "
BELL = bytes([7])

SCENARIOS = [
    (
        "1. script installed in fpath as _rg (the name the FAQ and man page give)",
        "_rg",
    ),
    (
        "2. script installed in fpath as _ripgrep (bound to rg by its `#compdef rg` first line)",
        "_ripgrep",
    ),
    (
        "3. script installed in fpath under another name: _rg_completion",
        "_rg_completion",
    ),
]


def read_until_idle(fd, idle=0.6, limit=8.0):
    out = b""
    start = time.time()
    last = time.time()
    while time.time() - start < limit:
        ready, _, _ = select.select([fd], [], [], 0.1)
        if ready:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
            last = time.time()
        elif time.time() - last > idle:
            break
    return out


def send(fd, data, idle=0.6):
    os.write(fd, data.encode())
    return read_until_idle(fd, idle=idle)


def bell(output):
    return "yes" if BELL in output else "no"


def run_scenario(rg_bin, title, filename):
    work = tempfile.mkdtemp(prefix="n2-probe.")
    try:
        home = os.path.join(work, "home")
        comp = os.path.join(home, ".zsh-complete")
        bindir = os.path.join(work, "bin")
        os.makedirs(comp)
        os.makedirs(bindir)
        os.symlink(rg_bin, os.path.join(bindir, "rg"))
        script = subprocess.run(
            [rg_bin, "--generate", "complete-zsh"], check=True, capture_output=True
        ).stdout
        with open(os.path.join(comp, filename), "wb") as handle:
            handle.write(script)

        env = {
            "HOME": home,
            "ZDOTDIR": home,
            "PATH": f"{bindir}:/usr/bin:/bin",
            "TERM": "xterm",
            "COLUMNS": "200",
            "LINES": "50",
        }
        pid, fd = pty.fork()
        if pid == 0:
            os.execve("/usr/bin/zsh", ["zsh", "-f", "-i"], env)
        read_until_idle(fd)
        setup = [
            "PS1='%% '",
            "fpath=(${fpath:#*vendor-completions*})",
            "fpath=($HOME/.zsh-complete $fpath)",
            "autoload -Uz compinit && compinit -D",
            'logline() { print -r -- "command line: <$BUFFER>   rg completion registered to: ${_comps[rg]-<nothing>}" >> $HOME/log }',
            "zle -N logline",
            "bindkey '^X^B' logline",
            'print -r -- "after compinit, rg completion registered to: ${_comps[rg]-<nothing>}" >> $HOME/log',
        ]
        for line in setup:
            send(fd, line + "\n", idle=0.4)

        send(fd, TYPED, idle=0.4)
        first = send(fd, "\t", idle=1.5)
        send(fd, "\x18\x02", idle=0.4)
        second = send(fd, "\t", idle=1.5)
        send(fd, "\x18\x02", idle=0.4)
        send(fd, "\x15", idle=0.3)
        send(fd, "exit\n", idle=0.3)
        try:
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass
        os.close(fd)

        with open(os.path.join(home, "log")) as handle:
            log = handle.read().splitlines()

        print(f"=== {title}")
        print(f"    file in fpath: ~/.zsh-complete/{filename}")
        print(f"    {log[0]}")
        print(f"    typed: <{TYPED}> then Tab")
        print(f"    after 1st Tab  {log[1]}")
        print(f"                   terminal bell on 1st Tab: {bell(first)}")
        print(f"    after 2nd Tab  {log[2]}")
        print(f"                   terminal bell on 2nd Tab: {bell(second)}")
        line_after_first = log[1].split("<", 1)[1].split(">", 1)[0]
        verdict = "completed" if line_after_first == EXPECTED else "NOT completed"
        print(f"    first Tab: {verdict}")
        print()
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main():
    rg_bin = os.path.abspath(sys.argv[1])
    version = subprocess.run([rg_bin, "--version"], capture_output=True, text=True).stdout.splitlines()[0]
    script = subprocess.run([rg_bin, "--generate", "complete-zsh"], capture_output=True).stdout
    zsh = subprocess.run(["zsh", "-f", "-c", "print $ZSH_VERSION"], capture_output=True, text=True).stdout.strip()
    print(f"rg: {version}")
    print(f"zsh: {zsh}")
    print(f"generated script sha256: {hashlib.sha256(script).hexdigest()}")
    print()
    for title, filename in SCENARIOS:
        run_scenario(rg_bin, title, filename)


if __name__ == "__main__":
    main()
