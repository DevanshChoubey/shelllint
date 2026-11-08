# MCP server: `shelllint-mcp`

`shelllint-mcp` lets AI coding agents lint shell scripts with the
shelllint port. It speaks the Model Context Protocol over stdio, so any
MCP client can start it as a subprocess.

## Build

```sh
cargo build --release -p shelllint-mcp
# binary: target/release/shelllint-mcp (use an absolute path below)
```

## Tools

| Tool | Purpose |
|------|---------|
| `check_script` | Lint script text. Returns SC codes, severity, line/column, message, and whether an auto-fix exists. |
| `check_file` | Same as `check_script`, for a file on disk (1 MiB limit). |
| `explain` | One code: message, check name, wiki link, bad and good examples. |
| `list_checks` | Known checks, filtered by text or limited to optional checks. |
| `fix_script` | Applies all auto-fixes. Returns the fixed script, a unified diff, and what remains. |

`check_script`, `check_file` and `fix_script` take `shell` (sh, bash, dash,
ksh, busybox), `enable` (optional check names, or `all`), `exclude` (codes such
as `SC2086`), and `severity` (error, warning, info, style).

Bad input comes back as a tool error (`isError: true`) with a message the model
can act on. Only unknown tools and malformed JSON-RPC are protocol errors.

## Protocol

- JSON-RPC 2.0, one message per line on stdin/stdout. Logs are not written.
- Revisions: `2026-07-28` (current; `server/discover` and per-request `_meta`),
  `2025-11-25` and `2025-06-18` (`initialize` negotiation). Older clients that
  send `initialize` with an unknown version get `2025-11-25` back.
- Methods: `initialize`, `server/discover`, `ping`, `tools/list`, `tools/call`.
  Notifications are accepted and not answered. Batches are rejected.

## Client configuration

Replace `/abs/path/to/shelllint-mcp` with the built binary's absolute path.

### Claude Code

```sh
claude mcp add --transport stdio shellcheck -- /abs/path/to/shelllint-mcp
# for every project on this machine:
claude mcp add --transport stdio --scope user shellcheck -- /abs/path/to/shelllint-mcp
```

### Claude Desktop

Edit `claude_desktop_config.json` (Settings > Developer > Edit Config), then
restart Claude Desktop:

```json
{
  "mcpServers": {
    "shellcheck": {
      "command": "/abs/path/to/shelllint-mcp",
      "args": []
    }
  }
}
```

### Cursor

Put this in `~/.cursor/mcp.json` (all projects) or `.cursor/mcp.json` (one
project):

```json
{
  "mcpServers": {
    "shellcheck": {
      "command": "/abs/path/to/shelllint-mcp",
      "args": []
    }
  }
}
```

### VS Code

Put this in `.vscode/mcp.json` in the workspace, or run "MCP: Add Server" from
the command palette:

```json
{
  "servers": {
    "shellcheck": {
      "type": "stdio",
      "command": "/abs/path/to/shelllint-mcp",
      "args": []
    }
  }
}
```

## Testing by hand

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"check_script","arguments":{"script":"rm $1\n"}}}' \
  | /abs/path/to/shelllint-mcp
```

## Maintenance

`crates/mcp/data/codes.json` is generated from `crates/checks/src` (messages)
and `corpus/` (check names and examples) by `tools/gen_mcp_codes.py`. Rerun it
after adding or changing checks:

```sh
python3 -I tools/gen_mcp_codes.py .
```

## Limitations

- `check_file` does not follow `source` directives (the server uses the
  mocked file system, so no sourced files are read, and no `.shellcheckrc` or
  `.editorconfig` are applied).
- Optional-check descriptions come from the analyzer. For other codes,
  `explain` gives the message, check name and wiki link. 33 of the 236 codes have no
  good example, because the corpus has no non-firing test for them.
- Checks from the unported ControlFlow module are not reported (as in the CLI).
