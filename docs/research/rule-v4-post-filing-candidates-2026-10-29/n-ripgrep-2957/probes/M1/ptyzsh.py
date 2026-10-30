"""Drive a real interactive zsh through a pseudo-terminal.

Each session gets its own fresh HOME and ZDOTDIR under the scratch directory, so only the
.zshrc written here is read (zsh -d skips the system-wide startup files). The built rg of
the chosen commit is first on PATH. The system's own _rg completion directory is removed
from fpath before compinit, and any rg registration is dropped, so only the script under
test can register completion for rg.
"""
import os, pty, select, signal, time, shlex
from pathlib import Path

SCRATCH = Path('/home/jack/.cache/rule-v4-post-filing-scratch/n-ripgrep-2957')
PROMPT = b'READY> '

PREAMBLE = [
    "PROMPT='READY> '",
    "fpath=(${fpath:#*/vendor-completions})",
    "autoload -Uz compinit && compinit -D",
    "unset '_comps[rg]'",
]


def widget(capture):
    return [
        'capture_line() { print -r -- "$BUFFER" > ' + shlex.quote(str(capture)) + '; zle send-break; }',
        "zle -N capture_line; bindkey '^X^B' capture_line",
    ]


def state(path, label):
    # Safe under nounset: every expansion has a default.
    return ('print -r -- "STATE ' + label + ' status=$? comps_rg=${_comps[rg]-UNSET} '
            'funcstack=(${funcstack[*]-})" >> ' + shlex.quote(str(path)))


_counter = [0]


def session(name, rev, zshrc_lines, typed=(), tab_line=None, tabs=1, extra_env=None):
    _counter[0] += 1
    home = SCRATCH / 'homes' / f'{_counter[0]:03d}-{name}'
    home.mkdir(parents=True, exist_ok=False)
    capture = home / 'buffer.txt'
    statefile = home / 'state.txt'
    text = '\n'.join(PREAMBLE + widget(capture) + [l.replace('@STATE@', str(statefile)) for l in zshrc_lines]) + '\n'
    (home / '.zshrc').write_text(text)
    pid, fd = pty.fork()
    if pid == 0:
        env = dict(HOME=str(home), ZDOTDIR=str(home), TERM='xterm', LC_ALL='C',
                   PATH=str(SCRATCH / f'bin-{rev}') + ':/usr/bin:/bin')
        if extra_env:
            env.update(extra_env)
        os.chdir(home)
        os.execve('/usr/bin/zsh', ['zsh', '-d', '-i'], env)
    transcript = b''

    def read_for(seconds):
        nonlocal transcript
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            r, _, _ = select.select([fd], [], [], max(0, end - time.monotonic()))
            if r:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    return
                transcript += data

    def wait_prompts(n, timeout=8):
        end = time.monotonic() + timeout
        while transcript.count(PROMPT) < n and time.monotonic() < end:
            read_for(0.1)
        read_for(0.15)

    try:
        wait_prompts(1)
        for command in typed:
            n = transcript.count(PROMPT)
            os.write(fd, (command.replace('@STATE@', str(statefile)) + '\n').encode())
            wait_prompts(n + 1)
        buffer = None
        if tab_line is not None:
            os.write(fd, tab_line.encode())
            for _ in range(tabs):
                os.write(fd, b'\t')
                read_for(0.6)
            os.write(fd, b'\x18\x02')
            end = time.monotonic() + 5
            while not capture.exists() and time.monotonic() < end:
                read_for(0.2)
            buffer = capture.read_text().rstrip('\n') if capture.exists() else None
        os.write(fd, b'exit\n')
        read_for(0.3)
    finally:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.waitpid(pid, 0)
        os.close(fd)
    out = transcript.decode(errors='replace').replace('\r', '')
    return dict(
        name=name, rev=rev, zshrc=text, typed=list(typed), tab_line=tab_line, tabs=tabs,
        buffer_after_tab=buffer,
        state=statefile.read_text().splitlines() if statefile.exists() else [],
        error_lines=[l for l in out.splitlines() if 'not set' in l or 'can only be called' in l or 'command not found' in l or 'bad ' in l],
        transcript=out,
    )
