# SPDX-License-Identifier: GPL-3.0-or-later
# Library functions that run under `emulate -L zsh` with extended globbing
# (own code, written for these tests).
emulate -L zsh
setopt extended_glob

lib_newest() {
  emulate -L zsh
  setopt extended_glob null_glob
  local dir=${1:-.}
  local -a hits
  hits=( $dir/*(.Nom[1]) )
  print -r -- $hits[1]
}

lib_prune() {
  emulate -L zsh
  setopt extended_glob
  local dir=$1 days=${2:-30}
  [[ -d $dir ]] || return 1
  cd $dir
  rm -f -- *(.m+$days)
}

lib_classify() {
  emulate -L zsh
  setopt extended_glob
  local name=$1
  [[ $name == (#i)*.(jpg|png|gif) ]] && print -r -- image
  if [[ $name == (#b)(*)-<->.<->.<->* ]]; then
    print -r -- $match[1]
  fi
  case $name in
    (*.tar.(gz|bz2|xz)) print -r -- archive ;;
    <->) print -r -- number ;;
    *) print -r -- other ;;
  esac
}

lib_newest ~/Downloads
