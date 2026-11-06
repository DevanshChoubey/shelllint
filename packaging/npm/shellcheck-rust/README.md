# shellcheck-rust

A Rust port of ShellCheck. This npm package is a launcher: the binary for your
platform comes in as an optional dependency (`@shellcheck-rust/<os>-<arch>`).

    npm install -g shellcheck-rust
    shellcheck-rust -f gcc script.sh
    npx shellcheck-rust --version

Supported: linux x64/arm64, macOS x64/arm64, Windows x64.
