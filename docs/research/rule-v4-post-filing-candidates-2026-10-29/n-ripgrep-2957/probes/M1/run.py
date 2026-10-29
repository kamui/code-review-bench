"""M1 matrix: where the generated script is loaded, with which shell options, at both commits.

Run: python3 -B run.py   (from this directory)
Writes result-base.txt, result-head.txt (summaries) and matrix-<rev>.json (full transcripts).
"""
import json, subprocess, os
from pathlib import Path
import ptyzsh
from ptyzsh import SCRATCH, state

HERE = Path(__file__).resolve().parent
OPTION_SETS = ['', 'nounset', 'no_eval_lineno', 'nounset no_eval_lineno']
EVAL = 'eval "$(rg --generate=complete-zsh)"'
SOURCE = 'source <(rg --generate complete-zsh)'
TAB = 'rg --generate=complete-z'


def contexts(setopt):
    # (name, lines for .zshrc, lines typed at the prompt)
    S = state('@STATE@', 'after-load')
    return [
        ('prompt-eval', [], [setopt, EVAL, S]),
        ('zshrc-eval', [setopt, EVAL, S], []),
        ('zshrc-if-eval', [setopt, 'if type rg > /dev/null; then', '  ' + EVAL, 'fi', S], []),
        ('zshrc-function-eval', [setopt, 'load_rg() { ' + EVAL + '; }', 'load_rg', S], []),
        ('prompt-source', [], [setopt, SOURCE, S]),
        ('zshrc-source', [setopt, SOURCE, S], []),
    ]


def noninteractive(rev, opts):
    """Top level of zsh -c and of a script run as a command: no file or function on funcstack."""
    env = dict(os.environ, PATH=str(SCRATCH / f'bin-{rev}') + ':/usr/bin:/bin', HOME=str(SCRATCH / 'homes'))
    body = '\n'.join([
        'fpath=(${fpath:#*/vendor-completions})',
        'autoload -Uz compinit && compinit -D',
        "unset '_comps[rg]'",
        ('setopt ' + opts) if opts else ':',
        EVAL,
        'print -r -- "STATE after-load status=$? comps_rg=${_comps[rg]-UNSET} funcstack=(${funcstack[*]-})"',
    ])
    out = []
    r = subprocess.run(['zsh', '-f', '-c', body], capture_output=True, text=True, env=env)
    out.append(dict(context='zsh-c-eval', options=opts, stdout=r.stdout, stderr=r.stderr, exit=r.returncode))
    script = SCRATCH / 'homes' / f'script-{rev}-{opts.replace(" ", "+") or "none"}.zsh'
    script.write_text(body + '\n')
    r = subprocess.run(['zsh', '-f', str(script)], capture_output=True, text=True, env=env)
    out.append(dict(context='script-eval', options=opts, stdout=r.stdout, stderr=r.stderr.replace(str(script), '<script>'), exit=r.returncode))
    return out


def main():
    (SCRATCH / 'homes').mkdir(exist_ok=True)
    for rev in ['base', 'head']:
        rows = []
        lines = [f'# M1 matrix at {rev}', '# context | options | errors printed | state after loading | buffer after one Tab on "' + TAB + '"']
        for opts in OPTION_SETS:
            setopt = ('setopt ' + opts) if opts else ':'
            for name, rc, typed in contexts(setopt):
                d = ptyzsh.session(f'{rev}-{name}-{opts.replace(" ", "+") or "none"}', rev, rc, typed, tab_line=TAB)
                d.update(context=name, options=opts)
                rows.append(d)
                lines.append(f'{name} | {opts or "(defaults)"} | {d["error_lines"] or "none"} | {d["state"]} | {d["buffer_after_tab"]!r}')
                print(lines[-1], flush=True)
            for n in noninteractive(rev, opts):
                rows.append(n)
                lines.append(f'{n["context"]} | {opts or "(defaults)"} | stderr={n["stderr"].strip()!r} | {n["stdout"].strip()} | (no Tab: not interactive)')
                print(lines[-1], flush=True)
        (HERE / f'matrix-{rev}.json').write_text(json.dumps(rows, indent=1) + '\n')
        (HERE / f'result-{rev}.txt').write_text('\n'.join(lines) + '\n')


if __name__ == '__main__':
    main()
