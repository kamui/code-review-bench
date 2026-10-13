#!/usr/bin/env python3
"""Run the same wrapper with other tools' generated zsh completion scripts.

Usage: tab_tool.py

For each tool, three new interactive `zsh -f` sessions on a pseudo-terminal, each
with an empty HOME, the completion file in ~/.zsh-complete, that directory in
fpath before compinit:

  saved    the tool's generated script saved as _<tool> (the control)
  wrapper  a file _<tool> holding `#compdef <tool>` and `source <(<generate command>)`
  wrapper+call  the same wrapper with a last line that calls the function itself

The tools are the versions installed on this machine on the day of the run, not
the versions of the cut-off. cargo's wrapper is the text `rustup completions zsh
cargo` prints, unchanged.
"""

import os
import pty
import select
import shutil
import subprocess
import tempfile
import time

BELL = bytes([7])
HOME = os.path.expanduser("~")
SYSROOT = subprocess.run(["rustc", "--print", "sysroot"], capture_output=True, text=True).stdout.strip()

# tool, binary, generate command, typed, expected, how the generated script ends (read)
TOOLS = [
    ("gh", "/usr/bin/gh", "gh completion -s zsh", "gh rep", "gh repo ", "_gh"),
    ("uv", os.path.join(HOME, ".local/bin/uv"), "uv generate-shell-completion zsh", "uv generate-shell-comp", "uv generate-shell-completion ", '_uv "$@"'),
    ("deno", os.path.join(HOME, ".deno/bin/deno"), "deno completions zsh", "deno upg", "deno upgrade ", '_deno "$@"'),
]


def read_until_idle(fd, idle=0.6, limit=10.0):
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


def session(tool, extra_path, filename, content, typed, expected):
    work = tempfile.mkdtemp(prefix="m1-tool.")
    try:
        home = os.path.join(work, "home")
        comp = os.path.join(home, ".zsh-complete")
        os.makedirs(comp)
        with open(os.path.join(comp, filename), "wb") as handle:
            handle.write(content)
        env = {"HOME": home, "ZDOTDIR": home, "PATH": f"{extra_path}:/usr/bin:/bin", "TERM": "xterm", "COLUMNS": "200", "LINES": "50"}
        pid, fd = pty.fork()
        if pid == 0:
            os.execve("/usr/bin/zsh", ["zsh", "-f", "-i"], env)
        read_until_idle(fd)
        for line in [
            "PS1='%% '",
            "fpath=(${fpath:#*vendor-completions*})",
            "fpath=(${fpath:#*site-functions*})",
            "fpath=($HOME/.zsh-complete $fpath)",
            "autoload -Uz compinit && compinit -D",
            'logline() { print -r -- "<$BUFFER>|${_comps[' + tool + ']-<nothing>}" >> $HOME/log }',
            "zle -N logline",
            "bindkey '^X^B' logline",
        ]:
            send(fd, line + "\n", idle=0.4)
        send(fd, typed, idle=0.4)
        first = send(fd, "\t", idle=3.0)
        send(fd, "\x18\x02", idle=0.4)
        send(fd, "\t", idle=3.0)
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
        line1, reg1 = log[0][1:].split(">|")
        line2, _ = log[1][1:].split(">|")
        return (f"1st Tab: <{line1}> {'completed' if line1 == expected else 'NOT completed'}, bell {'yes' if BELL in first else 'no'}; "
                f"2nd Tab: <{line2}> {'line is complete' if line2 == expected else 'NOT completed'}; registered after 1st Tab: {reg1}")
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main():
    print("zsh:", subprocess.run(["zsh", "-f", "-c", "print $ZSH_VERSION"], capture_output=True, text=True).stdout.strip())
    for tool, binary, generate, typed, expected, call in TOOLS:
        bindir = os.path.dirname(binary)
        version = subprocess.run([binary, "--version"], capture_output=True, text=True).stdout.splitlines()[0]
        script = subprocess.run(generate.split(), capture_output=True, env={"PATH": f"{bindir}:/usr/bin:/bin", "HOME": HOME}).stdout
        ending = [row for row in script.decode().rstrip().splitlines() if row.strip()][-6:]
        print(f"\n=== {tool}: {version}")
        print(f"    generate command: {generate}; typed <{typed}>, expected <{expected}>")
        print("    last lines of the generated script:")
        for row in ending:
            print(f"        {row}")
        wrapper = f"#compdef {tool}\nsource <({generate})\n"
        print(f"    saved as _{tool}:        ", session(tool, bindir, f"_{tool}", script, typed, expected))
        print(f"    wrapper {wrapper!r}:")
        print("                             ", session(tool, bindir, f"_{tool}", wrapper.encode(), typed, expected))
        with_call = wrapper + call + "\n"
        print(f"    wrapper+call {with_call!r}:")
        print("                             ", session(tool, bindir, f"_{tool}", with_call.encode(), typed, expected))

    bindir = os.path.join(SYSROOT, "bin")
    version = subprocess.run([os.path.join(bindir, "cargo"), "--version"], capture_output=True, text=True).stdout.strip()
    rustup = subprocess.run(["rustup", "--version"], capture_output=True, text=True).stdout.splitlines()[0]
    wrapper = subprocess.run(["rustup", "completions", "zsh", "cargo"], capture_output=True).stdout
    with open(os.path.join(SYSROOT, "share/zsh/site-functions/_cargo")) as handle:
        ending = [row for row in handle.read().rstrip().splitlines() if row.strip()][-3:]
    print(f"\n=== cargo: {version}; wrapper printed by {rustup}")
    print("    the wrapper `rustup completions zsh cargo` prints:")
    for row in wrapper.decode().rstrip().splitlines():
        print(f"        {row}")
    print("    last lines of the file it sources, <sysroot>/share/zsh/site-functions/_cargo:")
    for row in ending:
        print(f"        {row}")
    print("    wrapper saved as _cargo: ", session("cargo", bindir, "_cargo", wrapper, "cargo bui", "cargo build "))


if __name__ == "__main__":
    main()
