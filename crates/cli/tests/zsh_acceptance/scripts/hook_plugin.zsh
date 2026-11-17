# SPDX-License-Identifier: GPL-3.0-or-later
# A hook-registering plugin: report slow commands (own code, written for these tests).
autoload -Uz add-zsh-hook

typeset -g _slow_cmd_start=0
typeset -g _slow_cmd_last=
typeset -gi _slow_cmd_threshold=${SLOW_CMD_THRESHOLD:-5}

_slow_cmd_preexec() {
  local started=$EPOCHSECONDS
  _slow_cmd_start=$EPOCHSECONDS
  _slow_cmd_last=$1
}

_slow_cmd_precmd() {
  (( _slow_cmd_start )) || return 0
  local elapsed=$(( EPOCHSECONDS - _slow_cmd_start ))
  _slow_cmd_start=0
  (( elapsed >= _slow_cmd_threshold )) && print -P "%F{yellow}took ${elapsed}s%f: $_slow_cmd_last"
  return 0
}

_slow_cmd_widget() {
  zle -M "last command: ${_slow_cmd_last:-none}"
}
zle -N slow-cmd-info _slow_cmd_widget
bindkey '^Xi' slow-cmd-info

_slow_cmd_chpwd() { _slow_cmd_start=0 }
_slow_cmd_cleanup() { unset _slow_cmd_start _slow_cmd_last }

add-zsh-hook preexec _slow_cmd_preexec
precmd_functions+=(_slow_cmd_precmd)
add-zsh-hook chpwd _slow_cmd_chpwd
zshexit_functions+=(_slow_cmd_cleanup)
