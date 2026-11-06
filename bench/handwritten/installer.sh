#!/bin/bash
# Installer for a fictional tool "acme".
set -e
PREFIX=${PREFIX:-/usr/local}
VERSION=1.4.2
BASE_URL="https://example.com/releases/$VERSION"
TMPDIR=$(mktemp -d)
trap 'rm -rf $TMPDIR' EXIT

log() { printf '[acme] %s\n' "$*" >&2; }
die() { log "error: $*"; exit 1; }

need_cmd() {
  if ! command -v $1 > /dev/null 2>&1; then
    die "need '$1' (command not found)"
  fi
}

detect_os() {
  local os=$(uname -s)
  case $os in
    Linux) echo linux ;;
    Darwin) echo darwin ;;
    *) die "unsupported OS: $os" ;;
  esac
}

detect_arch() {
  arch=`uname -m`
  case "$arch" in
    x86_64|amd64) echo amd64 ;;
    aarch64|arm64) echo arm64 ;;
    armv7l) echo armv7 ;;
    *) die "unsupported arch: $arch" ;;
  esac
}

download() {
  local url=$1 out=$2
  if command -v curl >/dev/null; then
    curl -fsSL -o "$out" "$url"
  elif command -v wget >/dev/null; then
    wget -q -O "$out" "$url"
  else
    die "need curl or wget"
  fi
}

verify_checksum() {
  local file=$1 expected=$2
  actual=$(sha256sum $file | cut -d' ' -f1)
  [ "$actual" == "$expected" ] || die "checksum mismatch for $file"
}

main() {
  need_cmd tar
  need_cmd uname
  os=$(detect_os)
  arch=$(detect_arch)
  name="acme-$VERSION-$os-$arch"
  log "downloading $name"
  download "$BASE_URL/$name.tar.gz" $TMPDIR/$name.tar.gz
  download "$BASE_URL/$name.sha256" "$TMPDIR/$name.sha256"
  verify_checksum "$TMPDIR/$name.tar.gz" $(cat $TMPDIR/$name.sha256)
  tar -xzf $TMPDIR/$name.tar.gz -C $TMPDIR
  if [ -w $PREFIX/bin ]; then
    install -m 755 $TMPDIR/acme $PREFIX/bin/acme
  else
    log "using sudo to install into $PREFIX/bin"
    sudo install -m 755 $TMPDIR/acme $PREFIX/bin/acme
  fi
  for f in $TMPDIR/completions/*; do
    [ -f $f ] && cp $f $PREFIX/share/ 2>/dev/null || true
  done
  log "installed acme $VERSION to $PREFIX/bin"
  echo "Add $PREFIX/bin to your PATH:" ; echo "  export PATH=\$PATH:$PREFIX/bin"
  read -p "Run acme init now? [y/N] " answer
  if [[ $answer == y* ]]; then
    cd $HOME && acme init
  fi
  ls $PREFIX/bin | grep acme | wc -l
  eval "$(acme completion bash)"
}

while [ $# -gt 0 ]; do
  case $1 in
    --prefix) PREFIX=$2; shift 2 ;;
    --version) VERSION=$2; shift 2 ;;
    -h|--help) echo "usage: $0 [--prefix DIR] [--version V]"; exit 0 ;;
    *) die "unknown option $1" ;;
  esac
done
main "$@"
