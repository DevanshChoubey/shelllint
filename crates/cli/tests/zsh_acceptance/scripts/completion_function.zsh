#compdef mytool
# SPDX-License-Identifier: GPL-3.0-or-later

# Completion for a made-up command (own code, written for these tests).

_mytool_profiles() {
  local -a profiles
  profiles=(${(f)"$(mytool profiles list 2>/dev/null)"})
  compadd -- "${profiles[@]}"
}

_mytool_envs() {
  local -a envs
  envs=($(mytool envs 2>/dev/null))
  compadd -- "${envs[@]}"
}

_mytool() {
  local state line ret=1

  _arguments -C \
    '(-h --help)'{-h,--help}'[show help]' \
    '--profile=[profile to use]:profile:->profile' \
    '1:command:->command' \
    '*::arg:->args' && ret=0

  case $state in
    command)
      _values 'command' 'build[build the thing]' 'clean[remove outputs]' 'deploy[ship it]' && ret=0
      ;;
    profile)
      _mytool_profiles && ret=0
      ;;
    args)
      case $line[1] in
        (build|deploy)
          _mytool_envs
          _files -g '*.(yml|yaml)' && ret=0
          ;;
        (clean)
          _message 'no arguments' && ret=0
          ;;
      esac
      ;;
  esac
  return $ret
}

compdef _mytool mytool
