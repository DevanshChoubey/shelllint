# shelllint

A Rust port of ShellCheck. This npm package is a launcher: the binary for your
platform comes in as an optional dependency (`@shelllint/<os>-<arch>`).

    npm install -g shelllint
    shelllint -f gcc script.sh
    npx shelllint --version

Supported: linux x64/arm64, macOS x64/arm64, Windows x64.
