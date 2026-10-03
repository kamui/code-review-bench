import os, pty, select, subprocess, time
from pathlib import Path
root = Path.cwd()

def read_available(master, duration=0.6):
    end = time.monotonic() + duration
    chunks = []
    while time.monotonic() < end:
        ready, _, _ = select.select([master], [], [], max(0, end-time.monotonic()))
        if ready:
            try:
                data = os.read(master, 65536)
            except OSError:
                break
            if not data:
                break
            chunks.append(data)
    return b''.join(chunks)

for mode in ('source', 'autoload'):
    master, slave = pty.openpty()
    env = dict(os.environ, TERM='dumb')
    proc = subprocess.Popen(['zsh', '-d', '-f'], stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True, cwd=root)
    os.close(slave)
    transcript = read_available(master)
    scratch = root / 'zsh-probes/head'
    setup = "PS1='REVIEW> '; RPS1=; "
    setup += "for review_fpath_entry in $fpath; do [[ -f $review_fpath_entry/_rg ]] && fpath=(${fpath:#$review_fpath_entry}); done; "
    if mode == 'autoload':
        setup += f"fpath=('{scratch}' $fpath); "
    setup += 'autoload -Uz compinit; compinit -D -i; '
    if mode == 'source':
        setup += f"source <(cat '{scratch}/_rg'); "
    setup += "print -r -- SETUP_DONE\n"
    os.write(master, setup.encode())
    transcript += read_available(master, 1.5)
    os.write(master, b'rg --generate=complete-zs\t')
    tab_result = read_available(master, 1.0)
    transcript += tab_result
    os.write(master, b'\x03exit\n')
    transcript += read_available(master)
    try:
        proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
    os.close(master)
    (root / 'thermo-report/evidence' / f'head-interactive-{mode}.txt').write_bytes(transcript)
    print(mode + ': ' + repr(tab_result.decode(errors='replace')))
    if b'complete-zsh' not in tab_result:
        raise SystemExit('Expected complete-zsh after tab')
