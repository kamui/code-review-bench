#!/bin/sh
# Reproduce: <mirror> is the bare mirror of BurntSushi/ripgrep (n-ripgrep-2957.git),
# <zsh-root> the extracted Debian trixie packages zsh_5.9-8+b24_amd64.deb and
# zsh-common_5.9-8_all.deb (dpkg -x each into the same directory), <scratch> any empty dir.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
mirror=$1; zr=$2; s=$3
"$here/gen.sh" "$mirror" 79cbe89deb1151e703f4d91b19af9cdcc128b765 "$s/base.zsh"
"$here/gen.sh" "$mirror" 855bfa6cdae4f4fe8762f892fc4957635397083e "$s/head.zsh"
sha256sum "$s/base.zsh" "$s/head.zsh" | sed "s|$s|<scratch>|g"
mkdir -p "$s/tty"
python3 -I "$here/drive.py" "$zr" "$s" "$s/tty"
"$here/funcstack.sh" "$zr" "$s/fs"
