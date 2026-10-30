"""M1 platform, comparable-tool and documented-way checks, typed at an interactive prompt.

Run: python3 -B platform.py   (from this directory)
Writes platform-results.json and platform-results.txt.

1. zsh's own completion (`ls --colo` + Tab) under each option set.
2. Comparable tools whose generated zsh script also tests $funcstack[1]: eval at the prompt
   under each option set, then Tab. Installed versions are current (2026), not 2024.
3. The FAQ's two documented setups at head under nounset + no_eval_lineno: the fpath file
   (with compinit after the fpath line) and `source <(rg --generate complete-zsh)` typed and
   in .zshrc. (The matrix in run.py already covers source; this repeats it beside fpath.)
"""
import json, shlex
from pathlib import Path
import ptyzsh
from ptyzsh import SCRATCH, state

HERE = Path(__file__).resolve().parent
OPTION_SETS = ['', 'nounset', 'no_eval_lineno', 'nounset no_eval_lineno']
TOOL_ENV = dict(PATH=str(SCRATCH / 'bin-head') + ':/home/jack/.cargo/bin:/home/jack/.local/bin:/usr/local/bin:/usr/bin:/bin',
                RUSTUP_HOME='/home/jack/.rustup', CARGO_HOME='/home/jack/.cargo')
TOOLS = [
    # (command name, generator, line to complete, registration key)
    ('gh', 'gh completion -s zsh', 'gh compl', 'gh'),
    ('rustup', 'rustup completions zsh', 'rustup completi', 'rustup'),
    ('kubectl', 'kubectl completion zsh', 'kubectl versi', 'kubectl'),
    ('uv', 'uv generate-shell-completion zsh', 'uv generate-shell-compl', 'uv'),
]


def comps_state(key, label):
    return ('print -r -- "STATE ' + label + ' status=$? comps_' + key + '=${_comps[' + key + ']-UNSET}" >> @STATE@')


def main():
    rows, lines = [], []
    for opts in OPTION_SETS:
        setopt = ('setopt ' + opts) if opts else ':'
        d = ptyzsh.session('platform-ls-' + (opts.replace(' ', '+') or 'none'), 'head', [], [setopt], tab_line='ls --colo')
        d.update(check='zsh-own-ls', options=opts)
        rows.append(d)
        lines.append(f'zsh own ls | {opts or "(defaults)"} | errors={d["error_lines"] or "none"} | Tab: {d["buffer_after_tab"]!r}')
        print(lines[-1], flush=True)
        for name, gen, tab, key in TOOLS:
            typed = [setopt, f"unset '_comps[{key}]'", f'eval "$({gen})"', comps_state(key, 'after-eval')]
            d = ptyzsh.session(f'tool-{name}-' + (opts.replace(' ', '+') or 'none'), 'head', [], typed, tab_line=tab, extra_env=TOOL_ENV)
            d.update(check='comparable-' + name, options=opts, generator=gen)
            rows.append(d)
            lines.append(f'{name} prompt eval | {opts or "(defaults)"} | errors={d["error_lines"] or "none"} | {d["state"]} | Tab on {tab!r}: {d["buffer_after_tab"]!r}')
            print(lines[-1], flush=True)
    # Documented ways at head under both options.
    both = 'setopt nounset no_eval_lineno'
    compdir = SCRATCH / 'completions' / 'head'
    compdir.mkdir(parents=True, exist_ok=True)
    (compdir / '_rg').write_text((SCRATCH / 'gen' / 'head.zsh').read_text())
    for rev in ['base', 'head']:
        cdir = SCRATCH / 'completions' / rev
        cdir.mkdir(parents=True, exist_ok=True)
        (cdir / '_rg').write_text((SCRATCH / 'gen' / f'{rev}.zsh').read_text())
        # FAQ fpath line, then compinit run again after it (the order GT-n2 requires).
        rc = [both, 'fpath=(' + shlex.quote(str(cdir)) + ' $fpath)', 'compinit -D', state('@STATE@', 'after-fpath')]
        d = ptyzsh.session(f'documented-fpath-{rev}', rev, rc, [], tab_line='rg --generate=complete-z')
        d.update(check='documented-fpath', options='nounset no_eval_lineno')
        rows.append(d)
        lines.append(f'FAQ fpath file at {rev} | nounset no_eval_lineno | errors={d["error_lines"] or "none"} | {d["state"]} | Tab: {d["buffer_after_tab"]!r}')
        print(lines[-1], flush=True)
        for where in ['prompt', 'zshrc']:
            src = 'source <(rg --generate complete-zsh)'
            rc, typed = ([both, src, state('@STATE@', 'after-source')], []) if where == 'zshrc' else ([], [both, src, state('@STATE@', 'after-source')])
            d = ptyzsh.session(f'documented-source-{where}-{rev}', rev, rc, typed, tab_line='rg --generate=complete-z')
            d.update(check='documented-source-' + where, options='nounset no_eval_lineno')
            rows.append(d)
            lines.append(f'FAQ source line ({where}) at {rev} | nounset no_eval_lineno | errors={d["error_lines"] or "none"} | {d["state"]} | Tab: {d["buffer_after_tab"]!r}')
            print(lines[-1], flush=True)
    (HERE / 'platform-results.json').write_text(json.dumps(rows, indent=1) + '\n')
    (HERE / 'platform-results.txt').write_text('\n'.join(lines) + '\n')


if __name__ == '__main__':
    main()
