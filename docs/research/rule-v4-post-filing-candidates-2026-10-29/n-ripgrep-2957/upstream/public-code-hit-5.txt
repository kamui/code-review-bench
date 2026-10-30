# Zinit setup
ZINIT_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}/zinit/zinit.git"
[ ! -d "$ZINIT_HOME" ] && mkdir -p "$(dirname "$ZINIT_HOME")"
[ ! -d "$ZINIT_HOME/.git" ] && git clone https://github.com/zdharma-continuum/zinit.git "$ZINIT_HOME"
# shellcheck disable=SC1091
source "${ZINIT_HOME}/zinit.zsh"

autoload -Uz compinit && compinit
repo_dir="$HOME/reproducible-arch"
source "$repo_dir/zsh/.zshenv"
source "$repo_dir/zsh/.zshsecrets"
source "$repo_dir/zsh/.zsh_vi_mode"
source "$repo_dir/zsh/.zsh_aliases"

eval "$(mise activate zsh)"
# shfmt fmt:off #
# shellcheck disable=SC2206
precmd_functions=(${precmd_functions:#_mise_hook_precmd})
# shellcheck disable=SC2206
chpwd_functions=(${chpwd_functions:#_mise_hook_chpwd})
# shfmt fmt:on

if [[ $- == *i* ]]; then
  eval "$(starship init zsh)"
  eval "$(rg --generate=complete-zsh)"
  eval "$(zoxide init zsh --cmd cd)"
fi

# shellcheck disable=SC2034
typeset -U path PATH

export MANPAGER="sh -c 'awk '\''{ gsub(/\x1B\[[0-9;]*m/, \"\", \$0); gsub(/.\x08/, \"\", \$0); print }'\'' | bat -p -lman'"

# Plugins
zinit light zsh-users/zsh-syntax-highlighting
zinit light zsh-users/zsh-completions
zinit light zsh-users/zsh-autosuggestions
zinit light Aloxaf/fzf-tab
zinit snippet OMZP::git
zinit snippet OMZP::vi-mode

# Plugin configs
zinit cdreplay -q
zstyle ':completion:*' matcher-list 'm:{a-z}={A-Za-z}'
zstyle ':completion:*' menu no
# shellcheck disable=SC2016
zstyle ':fzf-tab:complete:cd:*' fzf-preview 'ls --color $realpath'
# shellcheck disable=SC2016
zstyle ':fzf-tab:complete:__zoxide_z:*' fzf-preview 'ls --color $realpath'

# needs to be after plugin setup
# shellcheck disable=SC1094
source "$repo_dir/home-shared/fzf-completion.zsh"
# shellcheck disable=SC1094
source "$repo_dir/home-shared/fzf-key-bindings.zsh"

# Keybindings
bindkey '^p' history-search-backward
bindkey '^n' history-search-forward
bindkey '^f' autosuggest-accept

# History setup
HISTSIZE=9999
HISTFILE=~/.zsh_history
# shellcheck disable=SC2034
SAVEHIST=$HISTSIZE
# shellcheck disable=SC2034
HISTDUP=erase
setopt appendhistory
setopt sharehistory
setopt hist_ignore_all_dups
setopt hist_save_no_dups
setopt hist_ignore_dups
setopt hist_find_no_dups
