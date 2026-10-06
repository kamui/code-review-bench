import os
import shlex
import subprocess
import sys
from pathlib import Path

binary = str(Path(sys.argv[1]).resolve())
work = Path(sys.argv[2]).resolve()
root = Path(__file__).resolve().parent
work.mkdir(parents=True, exist_ok=True)
setup = """fpath=(/usr/share/zsh/functions/Completion/Unix /usr/share/zsh/functions/Completion/Base /usr/share/zsh/functions/Completion/Zsh /usr/share/zsh/functions/Completion)
setopt ksh_arrays
autoload -Uz compinit
compinit -D
unfunction _rg 2>/dev/null
unset '_comps[rg]'
source <(RG_BINARY --generate complete-zsh)
print -r -- "STARTUP_SOURCE_STATUS=$? REGISTRATION=${_comps[rg]-NONE} KSH_ARRAYS=${options[ksharrays]}"
""".replace("RG_BINARY", shlex.quote(binary))
(work / ".zshrc").write_text(setup)
env = os.environ.copy()
env["ZDOTDIR"] = str(work)
for n in [1, 2]:
    print("\nZSHRC_STARTUP attempt=" + str(n), flush=True)
    result = subprocess.run(["zsh", "-i", "-c", "print -r -- SHELL_COMMAND_RAN=yes"], env=env, capture_output=True, text=True)
    print("stdout:\n" + result.stdout + "stderr:\n" + result.stderr + "process_exit=" + str(result.returncode), flush=True)
if sys.argv[3] == "head":
    generated = subprocess.check_output([binary, "--generate", "complete-zsh"], text=True)
    patched = generated.replace("$funcstack[1]", "${funcstack[1]}").replace("$+functions[compdef]", "${+functions[compdef]}")
    patched_path = root / "braced-control.zsh"
    patched_path.write_text(patched)
    print("\nBRACED_CONTROL after_compinit ksh_arrays=on", flush=True)
    command = 'autoload -Uz compinit; compinit -D; setopt ksh_arrays; source ' + shlex.quote(str(patched_path)) + '; print -r -- "SOURCE_STATUS=$? REGISTRATION=${_comps[rg]-NONE}"'
    result = subprocess.run(["zsh", "-f", "-c", command], capture_output=True, text=True)
    print("stdout:\n" + result.stdout + "stderr:\n" + result.stderr + "process_exit=" + str(result.returncode), flush=True)
