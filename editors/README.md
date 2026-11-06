# Editor integration

`shellcheck-rust-lsp` is a Language Server (LSP over stdio) for ShellCheck
diagnostics. It provides:

- diagnostics on open, on save, and on edit (debounced by 250 ms);
- quick fixes from ShellCheck's own suggested fixes;
- "Disable SCxxxx for this line", which inserts or extends a
  `# shellcheck disable=` directive;
- hover on a diagnostic showing the message and a wiki link;
- `.shellcheckrc` and `.editorconfig` discovery, and the `# shellcheck shell=`
  and `# shellcheck disable=` directives;
- incremental document sync.

Build and install the server first:

```sh
cargo build --release -p shellcheck-lsp
install -m 0755 target/release/shellcheck-rust-lsp ~/.local/bin/   # any dir on PATH
```

The server reads these settings (from `initializationOptions`, or from
`workspace/didChangeConfiguration`):

| key        | values                                                | meaning                                   |
|------------|-------------------------------------------------------|-------------------------------------------|
| `dialect`  | `auto`, `sh`, `bash`, `dash`, `ksh`, `busybox`        | shell to assume when the script does not say |
| `enable`   | list of optional check names, or `all`               | optional checks to turn on                |
| `exclude`  | list of codes, for example `SC2034` or `2034`         | codes never reported                      |
| `severity` | `error`, `warning`, `info`, `style`                  | minimum severity reported (default `style`) |

Subcommands: `shellcheck-rust-lsp --version`, `--help`. With no arguments it
speaks LSP on stdin and stdout.

The VS Code extension lives in `editors/vscode/`.

## Neovim (0.11+ with `vim.lsp.config`, or nvim-lspconfig)

Native (no plugin needed on 0.11+):

```lua
-- ~/.config/nvim/lsp/shellcheck_rust.lua
return {
  cmd = { 'shellcheck-rust-lsp' },
  filetypes = { 'sh', 'bash' },
  root_markers = { '.shellcheckrc', '.git' },
  single_file_support = true,
  init_options = { dialect = 'auto', enable = {}, exclude = {}, severity = 'style' },
}
```

```lua
-- init.lua
vim.lsp.enable('shellcheck_rust')
```

With nvim-lspconfig (custom config, works on older Neovim too):

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')
if not configs.shellcheck_rust then
  configs.shellcheck_rust = {
    default_config = {
      cmd = { 'shellcheck-rust-lsp' },
      filetypes = { 'sh', 'bash' },
      root_dir = lspconfig.util.root_pattern('.shellcheckrc', '.git'),
      single_file_support = true,
      init_options = { dialect = 'auto', enable = {}, exclude = {} },
    },
  }
end
lspconfig.shellcheck_rust.setup({})
```

Quick fixes: `vim.lsp.buf.code_action()`. Hover: `vim.lsp.buf.hover()`.

## Helix (`~/.config/helix/languages.toml`)

```toml
[language-server.shellcheck-rust]
command = "shellcheck-rust-lsp"

# Optional: settings, sent as initialization options.
[language-server.shellcheck-rust.config]
dialect = "auto"
enable = []
exclude = []
severity = "style"

[[language]]
name = "bash"
language-servers = ["shellcheck-rust"]
```

Helix's built-in `bash` language covers `sh` and `bash` files, so the
language-level entry above is enough. Add the server to the list alongside
any existing one, for example `language-servers = ["bash-language-server", "shellcheck-rust"]`.

## Zed

Zed starts language servers that an extension registers. A settings file can
point a registered server at this binary and set its arguments:

```json
// ~/.config/zed/settings.json
{
  "lsp": {
    "shellcheck-rust": {
      "binary": { "path": "shellcheck-rust-lsp", "arguments": [] },
      "initialization_options": { "dialect": "auto", "enable": [], "exclude": [] }
    }
  },
  "languages": {
    "Shell Script": {
      "language_servers": ["shellcheck-rust", "..."]
    }
  }
}
```

Caveat: this only takes effect where an installed extension already defines a
language server named `shellcheck-rust`. Zed has no built-in way to add a
custom server from settings alone, so a small Zed extension (a `language_servers`
entry in `extension.toml` with a `command` that runs `shellcheck-rust-lsp`) is
needed first. Untested here.

## VS Code

See `editors/vscode/`:

```sh
cd editors/vscode && npm install && npm run compile
npx @vscode/vsce package --no-yarn   # produces a .vsix; install with "Extensions: Install from VSIX"
```

Settings: `shellcheck-rust.server.path`, `shellcheck-rust.dialect`,
`shellcheck-rust.enable`, `shellcheck-rust.exclude`, `shellcheck-rust.severity`.
