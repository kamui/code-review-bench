#!/usr/bin/env python3
"""Drive a real interactive `zsh -f` through a pseudo-terminal and press Tab twice.

Usage: tab_probe.py LABEL /path/to/built/rg [--json OUT.json] [--only ID,ID]

Each scenario starts a new interactive `zsh -f` with an empty HOME and a cleared
environment. PATH holds only a directory with `rg` (a link to the built binary),
/usr/bin and /bin. The scenario writes its files into ~/.zsh-complete, runs its
setup lines, types `rg --generate=complete-z`, presses Tab, records the command
line, presses Tab again and records it again. A key bound to a small widget
writes the command line and the function registered for rg to a log file, so the
result does not depend on parsing terminal escape codes.

The distribution's completion directory is dropped from fpath because this
machine has a packaged ripgrep 13 whose own `_rg` would otherwise answer for rg.

The method is the one of the earlier probe
docs/research/cohort-rebuild-2026-10-05/candidates/n-ripgrep-2957/probes/N2/tab_probe.py,
extended with scenarios.
"""

import hashlib
import json
import os
import pty
import re
import select
import shutil
import subprocess
import sys
import tempfile
import time

TYPED = "rg --generate=complete-z"
EXPECTED = "rg --generate=complete-zsh "
BELL = bytes([7])
GENERATED = "@GENERATED@"
FPATH = "fpath=($HOME/.zsh-complete $fpath)"
SRC = "source <(rg --generate complete-zsh)"
EVL = 'eval "$(rg --generate complete-zsh)"'

# id, title, files in ~/.zsh-complete, lines before compinit, run compinit?, lines after compinit
SCENARIOS = [
    ("control-fpath-_rg",
     "generated script saved as _rg in fpath (the FAQ's recommended method)",
     {"_rg": GENERATED}, [FPATH], True, []),
    ("control-source-toplevel",
     "`source <(rg --generate complete-zsh)` typed after compinit (the FAQ's second method, without its `$ `)",
     {}, [], True, [SRC]),
    ("control-eval-toplevel",
     '`eval "$(rg --generate complete-zsh)"` typed after compinit',
     {}, [], True, [EVL]),
    ("n5-fpath-_ripgrep",
     "known problem GT-n5: generated script saved as _ripgrep in fpath",
     {"_ripgrep": GENERATED}, [FPATH], True, []),
    ("n5-fpath-_rg_completion",
     "known problem GT-n5: generated script saved as _rg_completion in fpath",
     {"_rg_completion": GENERATED}, [FPATH], True, []),
    ("n4-source-bare-_rg",
     "known problem GT-n4: script saved as _rg outside fpath, then `cd` there and `source _rg` after compinit",
     {"_rg": GENERATED}, [], True, ["cd ~/.zsh-complete && source _rg"]),
    ("wrapper-_rg-source",
     "wrapper: fpath file _rg holding `#compdef rg` and `source <(rg --generate complete-zsh)`",
     {"_rg": "#compdef rg\n" + SRC + "\n"}, [FPATH], True, []),
    ("wrapper-_rg-eval",
     'wrapper: fpath file _rg holding `#compdef rg` and `eval "$(rg --generate complete-zsh)"`',
     {"_rg": "#compdef rg\n" + EVL + "\n"}, [FPATH], True, []),
    ("wrapper-_rg-source-then-call",
     "wrapper that calls the function itself, the form of zsh's own _npm: `#compdef rg`, the source line, then `_rg \"$@\"`",
     {"_rg": "#compdef rg\n" + SRC + "\n_rg \"$@\"\n"}, [FPATH], True, []),
    ("wrapper-_ripgrep-eval-then-call",
     "the same form under another name with eval: fpath file _ripgrep holding `#compdef rg`, the eval line, then `_rg \"$@\"`",
     {"_ripgrep": "#compdef rg\n" + EVL + "\n_rg \"$@\"\n"}, [FPATH], True, []),
    ("wrapper-on-demand-explicit-call",
     "wrapper function that removes itself, evals the script and calls the registered function (the form of a public configuration dated 2024-12-19)",
     {}, [], True, ["_rg() { unfunction _rg; eval \"$(eval rg --generate complete-zsh)\"; $_comps[rg] }", "compdef _rg rg"]),
    ("wrapper-_ripgrep-source",
     "wrapper under another name: fpath file _ripgrep holding `#compdef rg` and the source line",
     {"_ripgrep": "#compdef rg\n" + SRC + "\n"}, [FPATH], True, []),
    ("wrapper-_ripgrep-eval",
     "wrapper under another name: fpath file _ripgrep holding `#compdef rg` and the eval line",
     {"_ripgrep": "#compdef rg\n" + EVL + "\n"}, [FPATH], True, []),
    ("wrapper-zshrc-function-_rg-source",
     "wrapper written as a function after compinit: `_rg() { source <(rg --generate complete-zsh) }; compdef _rg rg`",
     {}, [], True, ["_rg() { " + SRC + " }", "compdef _rg rg"]),
    ("wrapper-zshrc-function-_rg-eval",
     "wrapper written as a function after compinit: `_rg() { eval \"$(rg --generate complete-zsh)\" }; compdef _rg rg`",
     {}, [], True, ["_rg() { " + EVL + " }", "compdef _rg rg"]),
    ("wrapper-zshrc-function-other-name-source",
     "wrapper function under another name: `_rg_lazy() { source <(rg --generate complete-zsh) }; compdef _rg_lazy rg`",
     {}, [], True, ["_rg_lazy() { " + SRC + " }", "compdef _rg_lazy rg"]),
    ("wrapper-_rg-source-fpath-after-compinit",
     "wrapper file _rg, its directory added to fpath only after compinit, then `autoload -Uz _rg; compdef _rg rg`",
     {"_rg": "#compdef rg\n" + SRC + "\n"}, [], True, [FPATH, "autoload -Uz _rg", "compdef _rg rg"]),
    ("wrapper-_rg-source-no-compinit",
     "wrapper file _rg in fpath, compinit never run; the wrapper is then called by hand (`autoload -Uz _rg; _rg`)",
     {"_rg": "#compdef rg\n" + SRC + "\n"}, [FPATH], False, ["autoload -Uz _rg; _rg"]),
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


def run_scenario(rg_bin, script, ident, title, files, before, compinit, after):
    work = tempfile.mkdtemp(prefix="m1-probe.")
    try:
        home = os.path.join(work, "home")
        comp = os.path.join(home, ".zsh-complete")
        bindir = os.path.join(work, "bin")
        os.makedirs(comp)
        os.makedirs(bindir)
        os.symlink(rg_bin, os.path.join(bindir, "rg"))
        for name, content in files.items():
            with open(os.path.join(comp, name), "wb") as handle:
                handle.write(script if content == GENERATED else content.encode())

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
        setup = ["PS1='%% '", "fpath=(${fpath:#*vendor-completions*})"] + before
        if compinit:
            setup.append("autoload -Uz compinit && compinit -D")
        setup += [
            'logline() { print -r -- "command line: <$BUFFER>   rg completion registered to: ${_comps[rg]-<nothing>}" >> $HOME/log }',
            "zle -N logline",
            "bindkey '^X^B' logline",
        ]
        for line in setup:
            send(fd, line + "\n", idle=0.4)
        for line in after:
            send(fd, "{ " + line + " } >> $HOME/out 2>&1; print -r -- \"exit status: $?\" >> $HOME/out\n", idle=0.8)
        send(fd, 'print -r -- "before the first Tab, rg completion registered to: ${_comps[rg]-<nothing>}" >> $HOME/log\n', idle=0.4)

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
        out_path = os.path.join(home, "out")
        printed = ""
        if os.path.exists(out_path):
            with open(out_path) as handle:
                printed = re.sub(r"/proc/self/fd/\d+", "/proc/self/fd/N", handle.read().replace(work, "<tmp>"))

        line1 = log[1].split("<", 1)[1].split(">", 1)[0]
        line2 = log[2].split("<", 1)[1].split(">", 1)[0]
        result = {
            "id": ident,
            "first_tab": "completes" if line1 == EXPECTED else "nothing",
            "first_tab_bell": BELL in first,
            "second_tab": "completes" if line2 == EXPECTED else "nothing",
            "registered_before": log[0].rsplit(": ", 1)[1],
            "registered_after_first": log[1].rsplit(": ", 1)[1],
            "printed": printed.strip(),
        }

        print(f"=== {ident}: {title}")
        for name, content in files.items():
            shown = "the generated script" if content == GENERATED else repr(content)
            print(f"    file ~/.zsh-complete/{name}: {shown}")
        for line in before:
            print(f"    before compinit: {line}")
        print(f"    compinit run: {'yes' if compinit else 'no'}")
        for line in after:
            print(f"    after compinit: {line}")
        if after:
            for row in printed.strip().splitlines():
                print(f"        printed: {row}")
        print(f"    {log[0]}")
        print(f"    typed: <{TYPED}> then Tab")
        print(f"    after 1st Tab  {log[1]}")
        print(f"                   terminal bell on 1st Tab: {'yes' if BELL in first else 'no'}")
        print(f"    after 2nd Tab  {log[2]}")
        print(f"                   terminal bell on 2nd Tab: {'yes' if BELL in second else 'no'}")
        print(f"    first Tab: {'completed' if result['first_tab'] == 'completes' else 'NOT completed'}"
              f"; second Tab: {'line is complete' if result['second_tab'] == 'completes' else 'NOT completed'}")
        print()
        return result
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main():
    label = sys.argv[1]
    rg_bin = os.path.abspath(sys.argv[2])
    out = sys.argv[sys.argv.index("--json") + 1] if "--json" in sys.argv else None
    version = subprocess.run([rg_bin, "--version"], capture_output=True, text=True).stdout.splitlines()[0]
    script = subprocess.run([rg_bin, "--generate", "complete-zsh"], check=True, capture_output=True).stdout
    zsh = subprocess.run(["zsh", "-f", "-c", "print $ZSH_VERSION"], capture_output=True, text=True).stdout.strip()
    digest = hashlib.sha256(script).hexdigest()
    print(f"variant: {label}")
    print(f"rg: {version}")
    print(f"zsh: {zsh}")
    print(f"generated script sha256: {digest}")
    tail = script.decode().split("\n################################################################################\n# ZSH COMPLETION REFERENCE")[0]
    print("end of the generated script, before its reference section:")
    for row in tail.rstrip().splitlines()[-9:]:
        print(f"    {row}")
    print()
    only = sys.argv[sys.argv.index("--only") + 1].split(",") if "--only" in sys.argv else None
    results = [run_scenario(rg_bin, script, *scenario) for scenario in SCENARIOS if only is None or scenario[0] in only]
    if out:
        with open(out, "w") as handle:
            json.dump({"variant": label, "rg": version, "zsh": zsh, "script_sha256": digest, "scenarios": results}, handle, indent=1)
            handle.write("\n")


if __name__ == "__main__":
    main()
