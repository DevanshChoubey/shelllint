# A small prompt theme with vcs info (own code, written for these tests).
setopt prompt_subst

autoload -Uz vcs_info add-zsh-hook
zstyle ':vcs_info:*' enable git
zstyle ':vcs_info:git:*' formats ' %F{yellow}(%b)%f'
zstyle ':vcs_info:git:*' actionformats ' %F{red}(%b|%a)%f'

ZSH_THEME_GIT_PROMPT_DIRTY='%F{red}*%f'
ZSH_THEME_GIT_PROMPT_CLEAN=''

_theme_precmd() {
  vcs_info
}
add-zsh-hook precmd _theme_precmd

_theme_branch() {
  local branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
  [[ -n $branch ]] && print -r -- "$branch"
}

_theme_status() {
  local -a parts
  parts=("%F{green}%n%f" "%F{blue}%~%f")
  (( EUID == 0 )) && parts[1]="%F{red}%n%f"
  print -r -- ${(j: :)parts}
}

PROMPT='$(_theme_status)${vcs_info_msg_0_} %(?.%F{green}.%F{red})%#%f '
RPROMPT='%F{240}%*%f'
PS2='%F{240}>%f '

print -r -- $(_theme_branch)
