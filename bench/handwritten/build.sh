#!/bin/sh
# Build script for a C project with a few targets.
set -eu
ROOT=$(cd "$(dirname "$0")" && pwd)
BUILD_DIR=$ROOT/build
CC=${CC:-gcc}
CFLAGS="-O2 -Wall -Wextra"
JOBS=$(nproc 2>/dev/null || echo 2)
targets="lib tools tests docs"

usage() {
  cat <<USAGE
Usage: $0 [-j N] [-d] [target...]
  -j N   parallel jobs (default $JOBS)
  -d     debug build
Targets: $targets
USAGE
}

while getopts "j:dh" opt; do
  case $opt in
    j) JOBS=$OPTARG ;;
    d) CFLAGS="-O0 -g $CFLAGS" ;;
    h) usage; exit 0 ;;
    \?) usage >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -eq 0 ] && set -- $targets

mkdir -p "$BUILD_DIR"

compile() {
  src=$1
  obj="$BUILD_DIR/$(basename "${src%.c}").o"
  if [ ! -f "$obj" ] || [ "$src" -nt "$obj" ]; then
    echo "CC $src"
    $CC $CFLAGS -c "$src" -o "$obj"
  fi
}

build_lib() {
  for f in "$ROOT"/src/*.c; do
    compile "$f" &
    while [ "$(jobs -r | wc -l)" -ge "$JOBS" ]; do sleep 0.1; done
  done
  wait
  ar rcs "$BUILD_DIR/libacme.a" "$BUILD_DIR"/*.o
}

build_tools() {
  for t in "$ROOT"/tools/*.c; do
    name=$(basename $t .c)
    $CC $CFLAGS "$t" -L"$BUILD_DIR" -lacme -o "$BUILD_DIR/$name"
  done
}

run_tests() {
  fails=0
  for t in $BUILD_DIR/test_*; do
    [ -x "$t" ] || continue
    if ! "$t" > "$t.log" 2>&1; then
      echo "FAIL: $t"
      fails=$((fails + 1))
    fi
  done
  return $fails
}

for target in "$@"; do
  case "$target" in
    lib) build_lib ;;
    tools) build_tools ;;
    tests) run_tests || exit 1 ;;
    docs) (cd "$ROOT/docs" && make html) ;;
    clean) rm -rf "$BUILD_DIR" ;;
    *) echo "unknown target: $target" >&2; exit 2 ;;
  esac
done
echo "done"
