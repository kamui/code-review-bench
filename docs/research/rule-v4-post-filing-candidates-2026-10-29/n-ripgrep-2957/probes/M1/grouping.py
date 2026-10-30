"""M1 grouping runs (Ground rule 4): smallest fix for M1 alone, for each known problem's place
alone, and both together, each run against every failing case of both.

Run: python3 -B grouping.py   (from this directory)
Writes grouping/<variant>.patch (against the generated head script), grouping/results.json and
grouping/results.txt.

Known problems' smallest fixes are the ones the earlier rounds ran (rule-v4-candidates 2026-10-27,
probes/M1/grouping and probes/M2/grouping): GT-n4 adds a zsh_eval_context test, GT-n5 tests
$+compstate instead of the running name, GT-n6 adds the list request to the test, GT-n7 quotes
the return value. M1's smallest fix gives $funcstack[1] an empty default.
"""
import json, os, shlex, subprocess, difflib
from pathlib import Path
import ptyzsh
from ptyzsh import SCRATCH

HERE = Path(__file__).resolve().parent
OUT = HERE / 'grouping'
OLD = 'if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then'
RET = '  return ret\n}'
EDITS = {
    'M1': (OLD, 'if [[ ${funcstack[1]-} == _rg ]] || (( ! $+functions[compdef] )); then'),
    'GT-n4': (OLD, 'if [[ $funcstack[1] == _rg && $zsh_eval_context[-1] != file ]] || (( ! $+functions[compdef] )); then'),
    'GT-n5': (OLD, 'if (( $+compstate )) || (( ! $+functions[compdef] )); then'),
    'GT-n6': (OLD, 'if [[ $_RG_COMPLETE_LIST_ARGS == (1|t*|y*) ]] || [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then'),
    'GT-n7': (RET, '  return $ret\n}'),
}
COMBINED = {
    # Both edits applied to the same line where they meet.
    'M1+GT-n4': [(OLD, 'if [[ ${funcstack[1]-} == _rg && $zsh_eval_context[-1] != file ]] || (( ! $+functions[compdef] )); then')],
    'M1+GT-n5': [EDITS['GT-n5']],  # GT-n5's fix removes the $funcstack[1] read, so M1's edit has nothing left to change.
    'M1+GT-n6': [(OLD, 'if [[ $_RG_COMPLETE_LIST_ARGS == (1|t*|y*) ]] || [[ ${funcstack[1]-} == _rg ]] || (( ! $+functions[compdef] )); then')],
    'M1+GT-n6-guarded': [(OLD, 'if [[ ${_RG_COMPLETE_LIST_ARGS-} == (1|t*|y*) ]] || [[ ${funcstack[1]-} == _rg ]] || (( ! $+functions[compdef] )); then')],
    'M1+GT-n7': [EDITS['M1'], EDITS['GT-n7']],
}
VARIANTS = {'head': []}
VARIANTS.update({k: [v] for k, v in EDITS.items()})
VARIANTS.update(COMBINED)


def apply(text, edits):
    for old, new in edits:
        assert text.count(old) == 1, old
        text = text.replace(old, new)
    return text


def zsh_c(body, env_extra=None, cwd=None):
    env = dict(os.environ, PATH=str(SCRATCH / 'bin-head') + ':/usr/bin:/bin', HOME=str(SCRATCH / 'homes'))
    if env_extra:
        env.update(env_extra)
    return subprocess.run(['zsh', '-f', '-c', body], capture_output=True, text=True, env=env, cwd=cwd)


PRE = 'fpath=(${fpath:#*/vendor-completions}); autoload -Uz compinit && compinit -D; unset "_comps[rg]"'
STATE = 'print -r -- "status=$? comps_rg=${_comps[rg]-UNSET}"'


def case_m1(script):
    # Top-level eval with nounset + no_eval_lineno: zsh -c has the same empty funcstack as the prompt.
    r = zsh_c(f'{PRE}; setopt nounset no_eval_lineno; eval "$(<{shlex.quote(str(script))})"; {STATE}')
    return dict(stdout=r.stdout.strip(), stderr=r.stderr.strip(),
                fails=('comps_rg=_rg' not in r.stdout) or bool(r.stderr.strip()))


def case_m1_tab(script, name):
    d = ptyzsh.session(name, 'head', [], ['setopt nounset no_eval_lineno', f'eval "$(<{shlex.quote(str(script))})"'],
                       tab_line='rg --generate=complete-z')
    return dict(buffer=d['buffer_after_tab'], errors=d['error_lines'],
                fails=d['buffer_after_tab'] != 'rg --generate=complete-zsh ' or bool(d['error_lines']))


def case_n4(script):
    r = zsh_c(f'{PRE}; source _rg; {STATE}', cwd=script.parent)
    return dict(stdout=r.stdout.strip(), stderr=r.stderr.strip(),
                fails=('comps_rg=_rg' not in r.stdout) or bool(r.stderr.strip()))


def case_n5_renamed(script, name):
    d = SCRATCH / 'renamed' / name
    d.mkdir(parents=True, exist_ok=True)
    (d / '_ripgrep').write_text(script.read_text())
    s = ptyzsh.session(name, 'head', ['fpath=(' + shlex.quote(str(d)) + ' $fpath)', 'compinit -D'], [],
                       tab_line='rg --generate=complete-z')
    return dict(buffer=s['buffer_after_tab'], errors=s['error_lines'],
                fails=s['buffer_after_tab'] != 'rg --generate=complete-zsh ')


def case_n5_wrapper(script, name):
    s = ptyzsh.session(name, 'head', ['_rgw() { eval "$(<' + shlex.quote(str(script)) + ')"; }', 'compdef _rgw rg'], [],
                       tab_line='rg --generate=complete-z')
    return dict(buffer=s['buffer_after_tab'], errors=s['error_lines'],
                fails=s['buffer_after_tab'] != 'rg --generate=complete-zsh ')


def case_n6(template_text, name):
    tree = SCRATCH / 'test-trees' / name
    (tree / 'ci').mkdir(parents=True, exist_ok=True)
    (tree / 'crates/core/flags/complete').mkdir(parents=True, exist_ok=True)
    (tree / 'target/release').mkdir(parents=True, exist_ok=True)
    (tree / 'ci/test-complete').write_text((SCRATCH / 'rg-head/ci/test-complete').read_text())
    (tree / 'crates/core/flags/complete/rg.zsh').write_text(template_text)
    link = tree / 'target/release/rg'
    if not link.exists():
        link.symlink_to(SCRATCH / 'target-head/release/rg')
    zd = SCRATCH / 'test-homes' / 'stub'
    zd.mkdir(parents=True, exist_ok=True)
    (zd / '.zshenv').write_text('compdef() { :; }\n')
    env = dict(os.environ, ZDOTDIR=str(zd), PATH='/usr/bin:/bin')
    r = subprocess.run(['zsh', str(tree / 'ci/test-complete')], capture_output=True, text=True, env=env, cwd=tree)
    last = (r.stdout.strip().splitlines() or [''])[-1]
    return dict(exit=r.returncode, last_stdout=last, stderr=r.stderr.strip()[:300], fails=r.returncode != 0)


def case_n7(script, name):
    s = ptyzsh.session(name, 'head', [], [
        'alias -g ret=XX', f'source {shlex.quote(str(script))}',
        "zstyle ':completion:*' completer _complete _approximate",
        "zstyle ':completion:*:approximate:*' max-errors 2 numeric"], tab_line='rg --genrate')
    return dict(buffer=s['buffer_after_tab'], errors=s['error_lines'], fails=s['buffer_after_tab'] != 'rg --generate=')


def main():
    OUT.mkdir(exist_ok=True)
    gen = (SCRATCH / 'gen' / 'head.zsh').read_text()
    template = (SCRATCH / 'rg-head/crates/core/flags/complete/rg.zsh').read_text()
    results, lines = [], ['variant | M1 eval (zsh -c) | M1 prompt Tab | GT-n4 source _rg | GT-n5 renamed _ripgrep first Tab | GT-n5 wrapper eval first Tab | GT-n6 ci/test-complete with compdef stub | GT-n7 alias typo Tab']
    for variant, edits in VARIANTS.items():
        text = apply(gen, edits)
        vdir = SCRATCH / 'variants' / variant
        vdir.mkdir(parents=True, exist_ok=True)
        script = vdir / '_rg'
        script.write_text(text)
        if edits:
            diff = difflib.unified_diff(gen.splitlines(True), text.splitlines(True), 'a/_rg (generated at head)', 'b/_rg')
            (OUT / f'{variant}.patch').write_text(''.join(diff))
        row = dict(variant=variant,
                   m1=case_m1(script),
                   m1_tab=case_m1_tab(script, f'g-{variant}-m1tab'),
                   n4=case_n4(script),
                   n5_renamed=case_n5_renamed(script, f'g-{variant}-n5r'),
                   n5_wrapper=case_n5_wrapper(script, f'g-{variant}-n5w'),
                   n6=case_n6(apply(template, edits), f'g-{variant}-n6'),
                   n7=case_n7(script, f'g-{variant}-n7'))
        results.append(row)
        f = lambda c: 'FAILS' if row[c]['fails'] else 'ok'
        lines.append(f"{variant} | {f('m1')} {row['m1']['stderr'] or ''} {row['m1']['stdout']} | {f('m1_tab')} {row['m1_tab']['buffer']!r} | {f('n4')} {row['n4']['stdout']} | {f('n5_renamed')} {row['n5_renamed']['buffer']!r} | {f('n5_wrapper')} {row['n5_wrapper']['buffer']!r} | {f('n6')} exit={row['n6']['exit']} {row['n6']['last_stdout']!r} {row['n6']['stderr'][:80]!r} | {f('n7')} {row['n7']['buffer']!r}")
        print(lines[-1], flush=True)
    (OUT / 'results.json').write_text(json.dumps(results, indent=1) + '\n')
    (OUT / 'results.txt').write_text('\n'.join(lines) + '\n')


if __name__ == '__main__':
    main()
