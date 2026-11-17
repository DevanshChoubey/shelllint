# A legacy script that switches on sh-style word splitting and has real splitting bugs
# (own code, written for these tests).
setopt shwordsplit

files_dir=$1
backup_dir=${2:-/tmp/backup}

backup_all() {
  local f
  for f in $files_dir; do
    cp $f $backup_dir/
  done
  cp "$files_dir/index" "$backup_dir/"
}

remove_all() {
  rm -- $@
}

show_args() {
  local a
  for a in $*; do
    echo "arg: $a"
  done
}

split_into_array() {
  local -a parts
  parts=($1)
  print -r -- "${#parts}"
}

safe_copy() {
  setopt localoptions noshwordsplit
  cp $1 $2
}

list_dir() {
  ls $1
}

: ${cache_dir:=$HOME/.cache}

backup_all
