#!/usr/bin/env python3
"""Start a real interactive zsh from a given .zshrc on a pseudo-terminal, record what
it prints at startup, then type a command line and press Tab twice.

Usage: zshrc_probe.py LABEL /path/to/rg --set SET [--json OUT.json]

Each scenario gets a new empty HOME, which is also ZDOTDIR, and a cleared
environment. PATH holds a directory with `rg` (a link to the given binary) and,
for a comparable tool, a link to that tool as installed here, and /usr/bin and /bin. The scenario's lines are written to $ZDOTDIR/.zshrc. zsh is
started as `zsh -d -i`: -d skips the system-wide startup files /etc/zsh/zprofile,
/etc/zsh/zshrc and /etc/zsh/zlogin, so only the scenario's .zshrc decides whether
compinit runs. Scenarios marked "global" start `zsh -i` instead, so that this
Ubuntu machine's /etc/zsh/zshrc runs before the scenario's .zshrc.

$ZDOTDIR/.zshenv holds one harness line, `fpath=(${fpath:#*vendor-completions*})`:
this machine has a packaged ripgrep 13 whose own `_rg` in that directory would
otherwise answer for rg once compinit runs. .zshenv is read before every .zshrc,
the system-wide one included.

After startup the probe binds a key to a widget that appends the command line,
the function registered for the typed command (`$_comps[...]`) and whether
`compdef` exists to a log file. It types the scenario's text, presses Tab, logs,
presses Tab again and logs again. Startup output is the raw terminal output up to
the first prompt with escape sequences, the PROMPT_SP mark and the prompt removed.
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

BELL = bytes([7])
GENERATED = "@GENERATED@"
FPATH = "fpath=($HOME/.zsh-complete $fpath)"
SRC = "source <(rg --generate complete-zsh)"
SRC_FAQ = "$ source <(rg --generate complete-zsh)"
EVL = 'eval "$(rg --generate complete-zsh)"'
CI = "autoload -Uz compinit && compinit"
RG_TYPED, RG_EXPECTED = "rg --generate=complete-z", "rg --generate=complete-zsh "
SAVED = {"_rg": GENERATED}


def s(ident, title, lines, files=None, typed=RG_TYPED, expected=RG_EXPECTED, cmd="rg", glob=False):
    return dict(id=ident, title=title, lines=lines, files=files or {}, typed=typed, expected=expected, cmd=cmd, glob=glob)


# The FAQ's two ways, read at the head (FAQ.md:116-135): the script saved as ~/.zsh-complete/_rg plus the fpath
# line in .zshrc; or the source line in .zshrc. "with compinit" scenarios put the usual line `autoload -Uz
# compinit && compinit` where the title says.
CORE_FPATH = [
    s("m1a-fpath-no-compinit", "FAQ way 1: script saved as _rg, .zshrc holds only the fpath line; compinit never runs",
      [FPATH], SAVED),
    s("n2-fpath-after-compinit", "GT-n2's case: .zshrc runs compinit, then the fpath line", [CI, FPATH], SAVED),
    s("ctl-fpath-before-compinit", "control: .zshrc has the fpath line, then compinit", [FPATH, CI], SAVED),
    s("m1a-fpath-no-compinit-global-rc", "the fpath line only, started with the system-wide startup files of this "
      "Ubuntu machine, whose /etc/zsh/zshrc runs compinit before ~/.zshrc", [FPATH], SAVED, glob=True),
]
CORE_SOURCE = [
    s("m1b-source-no-compinit", "FAQ way 2 without its `$ `: .zshrc holds only the source line; compinit never runs",
      [SRC]),
    s("m1b-faq-verbatim-no-compinit", "FAQ way 2 as printed, with its `$ ` (GT-n1): .zshrc holds only that line",
      [SRC_FAQ]),
    s("m1b-eval-no-compinit", "the eval form the pull request's author uses in review: .zshrc holds only that line",
      [EVL]),
    s("n3-source-before-compinit", "GT-n3's case: .zshrc has the source line, then compinit", [SRC, CI]),
    s("ctl-source-after-compinit", "control: .zshrc runs compinit, then the source line", [CI, SRC]),
    s("m1b-source-no-compinit-global-rc", "the source line only, started with the system-wide startup files of this "
      "Ubuntu machine, whose /etc/zsh/zshrc runs compinit before ~/.zshrc", [SRC], glob=True),
]
PLATFORM = [
    s("platform-zsh-own-no-compinit", "zsh's own completion for `ls`, .zshrc empty; compinit never runs", [],
      typed="ls --colo", expected="ls --color=", cmd="ls"),
    s("platform-zsh-own-with-compinit", "zsh's own completion for `ls`, .zshrc runs compinit", [CI],
      typed="ls --colo", expected="ls --color=", cmd="ls"),
]


def tool(name, gen, typed, expected):
    return [
        s(f"comparable-{name}-source-no-compinit", f"`source <({gen})` alone in .zshrc; compinit never runs",
          [f"source <({gen})"], typed=typed, expected=expected, cmd=name),
        s(f"comparable-{name}-source-after-compinit", f"compinit, then `source <({gen})`",
          [CI, f"source <({gen})"], typed=typed, expected=expected, cmd=name),
        s(f"comparable-{name}-source-before-compinit", f"`source <({gen})`, then compinit",
          [f"source <({gen})", CI], typed=typed, expected=expected, cmd=name),
    ]


COMPARABLE = (
    tool("gh", "gh completion -s zsh", "gh completion -s z", "gh completion -s zsh ")
    + tool("uv", "uv generate-shell-completion zsh", "uv generate-shell-completion z",
           "uv generate-shell-completion zsh ")
    + tool("deno", "deno completions zsh", "deno complet", "deno completions ")
)


# Reader models of documentation fixes, run at the head. Each fix is a patch to FAQ.md at the head, saved as
# M1a/patches/<name>.patch (the fpath way) and M1b/patches/<name>.patch (the source way). A fix "allows" a layout when a person who follows its words literally could end up with it; where the words
# fix no place, every place is allowed. "has-ci" is a .zshrc that already runs compinit; "no-ci" one that does not.
def layouts_fpath():
    out = []
    # head text: no place stated, no compinit mentioned
    out += [("head", "has-ci", "fpath line after compinit", [CI, FPATH]),
            ("head", "has-ci", "fpath line before compinit", [FPATH, CI]),
            ("head", "no-ci", "fpath line, no compinit", [FPATH])]
    # fix-n2: "before the line that runs `compinit`"; in a .zshrc without that line the words fix no place
    out += [("fix-n2", "has-ci", "fpath line before compinit (the only place allowed)", [FPATH, CI]),
            ("fix-n2", "no-ci", "fpath line, no compinit (no compinit line exists to put it before)", [FPATH])]
    # fix-m1a: "If your .zshrc does not run compinit, add `autoload -Uz compinit && compinit` after this line."
    out += [("fix-m1a", "has-ci", "fpath line after compinit (the sentence is for .zshrc files without compinit)",
             [CI, FPATH]),
            ("fix-m1a", "has-ci", "fpath line before compinit", [FPATH, CI]),
            ("fix-m1a", "no-ci", "fpath line, then the added compinit line", [FPATH, CI])]
    # fix-both: fix-n2's words and fix-m1a's sentence
    out += [("fix-both", "has-ci", "fpath line before compinit", [FPATH, CI]),
            ("fix-both", "no-ci", "fpath line, then the added compinit line", [FPATH, CI])]
    # fix-block: the snippet becomes two lines, the fpath line and `autoload -Uz compinit && compinit`, to add as
    # shown; a .zshrc that already runs compinit may hold its own line before or after the block
    out += [("fix-block", "has-ci", "own compinit, then the block", [CI, FPATH, CI]),
            ("fix-block", "has-ci", "the block, then own compinit", [FPATH, CI, CI]),
            ("fix-block", "no-ci", "the block alone", [FPATH, CI])]
    return out


def layouts_source():
    out = []
    out += [("head", "has-ci", "source line after compinit", [CI, SRC]),
            ("head", "has-ci", "source line before compinit", [SRC, CI]),
            ("head", "no-ci", "source line, no compinit", [SRC])]
    # fix-n3: "after the line that runs `compinit`"; in a .zshrc without that line the words fix no place
    out += [("fix-n3", "has-ci", "source line after compinit (the only place allowed)", [CI, SRC]),
            ("fix-n3", "no-ci", "source line, no compinit (no compinit line exists to put it after)", [SRC])]
    # fix-m1b: "If your .zshrc does not run compinit, add `autoload -Uz compinit && compinit` before this line."
    out += [("fix-m1b", "has-ci", "source line after compinit", [CI, SRC]),
            ("fix-m1b", "has-ci", "source line before compinit (the sentence is for .zshrc files without compinit)",
             [SRC, CI]),
            ("fix-m1b", "no-ci", "the added compinit line, then the source line", [CI, SRC])]
    # fix-both: fix-n3's words and fix-m1b's sentence
    out += [("fix-both", "has-ci", "source line after compinit", [CI, SRC]),
            ("fix-both", "no-ci", "the added compinit line, then the source line", [CI, SRC])]
    # fix-block: the snippet becomes `autoload -Uz compinit && compinit` and the source line, to add as shown
    out += [("fix-block", "has-ci", "own compinit, then the block", [CI, CI, SRC]),
            ("fix-block", "has-ci", "the block, then own compinit", [CI, SRC, CI]),
            ("fix-block", "no-ci", "the block alone", [CI, SRC])]
    return out


def fix_scenarios(kind):
    rows = layouts_fpath() if kind == "fpath" else layouts_source()
    files = SAVED if kind == "fpath" else {}
    return [s(f"{fix}|{state}|{i}", f"{fix}, .zshrc {'already runs' if state == 'has-ci' else 'does not run'} "
              f"compinit: {desc}", lines, files) for i, (fix, state, desc, lines) in enumerate(rows)]


SETS = {
    "M1a": CORE_FPATH + PLATFORM,
    "M1b": CORE_SOURCE + PLATFORM,
    "fix-fpath": fix_scenarios("fpath"),
    "fix-source": fix_scenarios("source"),
    "comparable": COMPARABLE,
}

ANSI = re.compile(rb"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[()][0-9A-Za-z]|\x1b[=>]|\r")


def read_until_idle(fd, idle=0.6, limit=10.0):
    out = b""
    start = last = time.time()
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


def run(rg_bin, script, sc):
    work = tempfile.mkdtemp(prefix="zshrc-probe.")
    try:
        home = os.path.join(work, "home")
        comp = os.path.join(home, ".zsh-complete")
        bindir = os.path.join(work, "bin")
        os.makedirs(comp)
        os.makedirs(bindir)
        os.symlink(rg_bin, os.path.join(bindir, "rg"))
        if sc["cmd"] not in ("rg", "ls"):
            os.symlink(os.path.realpath(shutil.which(sc["cmd"])), os.path.join(bindir, sc["cmd"]))
        for name, content in sc["files"].items():
            with open(os.path.join(comp, name), "wb") as handle:
                handle.write(script if content == GENERATED else content.encode())
        with open(os.path.join(home, ".zshenv"), "w") as handle:
            handle.write("fpath=(${fpath:#*vendor-completions*})\n")
        with open(os.path.join(home, ".zshrc"), "w") as handle:
            handle.write("".join(line + "\n" for line in sc["lines"]))
        env = {"HOME": home, "ZDOTDIR": home, "PATH": f"{bindir}:/usr/bin:/bin", "TERM": "xterm",
               "COLUMNS": "200", "LINES": "50"}
        argv = ["zsh", "-i"] if sc["glob"] else ["zsh", "-d", "-i"]
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(home)
            os.execve("/usr/bin/zsh", argv, env)
        startup = read_until_idle(fd, idle=1.5, limit=20.0)
        cmd = sc["cmd"]
        harness = [
            "PS1='%% '",
            'logline() { print -r -- "line: <$BUFFER>  registered for ' + cmd + ': ${_comps[' + cmd
            + ']-<nothing>}  compdef defined: $+functions[compdef]" >> $HOME/log }',
            "zle -N logline",
            "bindkey '^X^B' logline",
            'print -r -- "before Tab: registered for ' + cmd + ': ${_comps[' + cmd
            + ']-<nothing>}  compdef defined: $+functions[compdef]  _rg defined: $+functions[_rg]" >> $HOME/log',
        ]
        for line in harness:
            send(fd, line + "\n", idle=0.4)
        send(fd, sc["typed"], idle=0.4)
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
        text = ANSI.sub(b"", startup).decode(errors="replace")
        text = re.sub(r"/proc/self/fd/\d+", "/proc/self/fd/N", text.replace(work, "<tmp>"))
        # Drop zsh's PROMPT_SP mark (a lone `%` padded with spaces) and the default prompt `host% `.
        rows = [re.sub(r"[\w.-]+% ?$", "", re.sub(r"^%\s+", "", row)) for row in text.split("\n")]
        printed = [row for row in rows if row.strip()]
        line1 = log[1].split("<", 1)[1].split(">", 1)[0]
        line2 = log[2].split("<", 1)[1].split(">", 1)[0]
        return {
            "id": sc["id"], "title": sc["title"], "zshrc": sc["lines"], "files": list(sc["files"]),
            "started_as": " ".join(argv), "startup_output": printed, "before_tab": log[0],
            "typed": sc["typed"], "after_first_tab": log[1], "after_second_tab": log[2],
            "first_tab": "completes" if line1 == sc["expected"] else "nothing",
            "first_tab_bell": BELL in first,
            "second_tab": "completes" if line2 == sc["expected"] else "nothing",
        }
    finally:
        shutil.rmtree(work, ignore_errors=True)


def show(r):
    print(f"=== {r['id']}: {r['title']}")
    print(f"    started as: {r['started_as']}")
    for name in r["files"]:
        print(f"    file ~/.zsh-complete/{name}: the generated script")
    print("    .zshrc:" + ("" if r["zshrc"] else " (empty)"))
    for line in r["zshrc"]:
        print(f"        {line}")
    print("    printed at startup, up to the first prompt:")
    for row in r["startup_output"]:
        print(f"        | {row}")
    print(f"    {r['before_tab']}")
    print(f"    typed: <{r['typed']}> then Tab")
    print(f"    after 1st Tab: {r['after_first_tab']}   bell: {'yes' if r['first_tab_bell'] else 'no'}")
    print(f"    after 2nd Tab: {r['after_second_tab']}")
    print(f"    RESULT first Tab: {r['first_tab']}; second Tab: {r['second_tab']}")
    print()


def main():
    label, rg_bin = sys.argv[1], os.path.abspath(sys.argv[2])
    chosen = sys.argv[sys.argv.index("--set") + 1]
    out = sys.argv[sys.argv.index("--json") + 1] if "--json" in sys.argv else None
    version = subprocess.run([rg_bin, "--version"], capture_output=True, text=True).stdout.splitlines()[0]
    script = subprocess.run([rg_bin, "--generate", "complete-zsh"], check=True, capture_output=True).stdout
    zsh = subprocess.run(["zsh", "-f", "-c", "print $ZSH_VERSION"], capture_output=True, text=True).stdout.strip()
    print(f"commit: {label}\nrg: {version}\nzsh: {zsh}\nscenario set: {chosen}")
    print(f"generated script sha256: {hashlib.sha256(script).hexdigest()}\n")
    results = []
    for sc in SETS[chosen]:
        r = run(rg_bin, script, sc)
        show(r)
        results.append(r)
    if out:
        with open(out, "w") as handle:
            json.dump({"commit": label, "rg": version, "zsh": zsh, "set": chosen, "scenarios": results}, handle,
                      indent=1)
            handle.write("\n")


if __name__ == "__main__":
    main()
