# Porting a check

Read this guide before you port any check. `python3 -I tools/task_prompt.py Txxx` prints your task's brief. It lists the checks, the Haskell line spans, the SC codes, the `CheckEntry` values, the tests, and the gate commands. The pilots are worked examples:

| Kind | Pilot |
|---|---|
| Analytics node check | `crates/checks/src/analytics/check_echo_wc.rs` |
| Analytics tree check | `crates/checks/src/analytics/check_shebang_parameters.rs` |
| Commands check | `crates/checks/src/commands/check_tr.rs` |
| ShellSupport check | `crates/checks/src/shell_support/check_for_decimals.rs` |

## 1. One file per check

Put each check in `crates/checks/src/<module>/<snake_name>.rs`. The module directories are:

- `analytics` for `Analytics.hs`
- `commands` for `Checks/Commands.hs`
- `shell_support` for `Checks/ShellSupport.hs`

The file name is the Haskell name in snake_case. For example, `checkEchoWc` becomes `check_echo_wc.rs`. Don't add a `mod` line anywhere, because `crates/checks/build.rs` generates one for every `.rs` file in the module directory.

Each file starts with these two lines:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkEchoWc`, lines 483-494 (e11f964)
```

## 2. Register a `CheckEntry`

```rust
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",       // exactly as in corpus/tasks.json
    name: "checkEchoWc",          // the Haskell name
    registry: "node",             // from corpus/tasks.json
    order: 21,                    // from corpus/tasks.json; copy it, never guess
    optional: None,
    kind: CheckKind::Node(check_echo_wc),
};
```

Write the declaration exactly like this, starting at the beginning of a line: `crates/checks/build.rs` scans each file for `pub(crate) static <NAME>: CheckEntry` and lists every one it finds in the generated `shellcheck_checks::CHECKS` registry, which the checker passes to the analyzer. A static written any other way (indented, `pub`, or without `pub(crate)`) is not registered.

Each registry needs a matching `kind` and function signature:

| Registry | `kind` | Function signature |
|---|---|---|
| `node` | `Node(f)` | `fn(&Params, Id, &mut Comments)` |
| `tree` | `Tree(f)` | `fn(&Params, Id) -> Vec<TokenComment>` |
| `optional-tree` | `Tree(f)`, or `Node(f)` when Haskell wraps it as `nodeChecksToTreeCheck [f]` | as above |
| `command` / `optional-command` | `Command(CommandName::Exactly("x") \| Basename("x"), f)` | `f` receives the `T_SimpleCommand` |
| `shell-support` | `ForShell(&[Shell::Sh, ...], f)` | `fn(&Params, Id, &mut Comments)` |

Optional checks set `optional: Some(StaticCheckDescription { name, id, description, positive, negative })`. Copy all five fields verbatim from the Haskell `newCheckDescription`.

The registry and `run_check` look a check up by both module and name. Two modules can define a check with the same name. For example, `checkForDecimals` exists in both `Analytics.hs` and `ShellSupport.hs`, so always write the exact module string.

A function that no Haskell list registers but that has `prop_` tests is registered under `registry: "unregistered"`. The tests can still reach it, but no module checker ever runs it. `analytics/check_for_decimals.rs` is an example.

### Checks that take an argument

Some checks are registered once per argument, for example `map checkSudoArgs privilegeElevationCommands`. The 49 corpus tests for these checks call them as `(checkSudoArgs "sudo")`. The `registrations` field in tasks.json (which the brief prints) lists one expression per argument. For each expression:

- For command checks, use `parametrised_command_checks!` from `crate::support` (at the start of a line, at most once per file). It declares `pub(crate) static ENTRIES`, one entry per argument, each with `name` set to the expression exactly (for example `checkSudoArgs "sudo"`, the string the harness rebuilds from the test source), all with the same `order`. `build.rs` registers every element:

```rust
parametrised_command_checks!("checkSudoArgs", 49, Basename, check_sudo_args_, ["sudo", "doas", "run0"]);
fn check_sudo_args_(cmd: &str, p: &Params<'_>, t: Id, c: &mut Comments) { /* the port */ }
```

- Otherwise, register one static per argument (`ENTRY_SUDO`, `ENTRY_DOAS`, ...) with the same `order`. `NodeFn` is a plain `fn` pointer and can't capture the argument, so write one small wrapper function per argument that calls a shared function with the argument.

`checkUnassignedReferences` works the same way. It has a second registration, `checkUnassignedReferences' True`, in `optional-tree`.

## 3. Read the Haskell

The brief gives a line span. The span covers the definition and its `where` clauses, and the `prop_` tests sit just above it. Port everything in the span and keep the Haskell order.

- Haskell names become snake_case: `getLiteralString` becomes `get_literal_string`.
- Many helpers are already ported:
  - `shellcheck_core::astlib` has ASTLib.
  - `shellcheck_analyzer::analyzerlib` has AnalyzerLib, including `Comments::warn`/`err`/`info`/`style` and their `_with_fix` versions.
  - `shellcheck_core::regex` has Regex. Build each regex once with a `static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("..."))`.
  - `shellcheck_core::prelude` and `shellcheck_core::data` cover Prelude and Data.
- Haskell `getId t` is the `Id` you already hold.
- Token `==` in Haskell compares structure, so use `ast.structurally_eq(a, b)`. Rust's `==` on `Token` compares child ids.
- Haskell `isDigit` is ASCII-only, so use `is_ascii_digit`.
- `Maybe`-monad `do` blocks become `let ... else { return }` or `?` inside a closure.
- Port literally. If you have to diverge, explain why in a `// DIVERGENCE:` comment.

## 4. Helpers

`crates/checks/src/support.rs` holds the top-level helpers that two or more checks share. The Analytics helpers are `checkList`, `runNodeAnalysis`, `nodeChecksToTreeCheck`, `checkCommand`, `hasFloatingPoint`, `isCondition`, `replaceStart`, `replaceEnd`, `replaceToken`, `surroundWith`, `fixWith`, `analyse`, `functions`, `doVariableFlowAnalysis` and `isSourced`. The Commands helpers are `checkGrepSendsPipefailImpl`, `returnOrExit`, `getSingleUnmodifiedBracedString` and `missingDestination`. Import them with `use crate::support::...`.

Any other helper goes in your own file as a private `fn`, including a top-level Haskell helper that only your check uses. Never edit `support.rs` or another check's file. If you need a shared helper changed, or you find a bug in one, report NEEDS_CONTEXT and include the exact change.

## 5. Synthetic tokens

Some Haskell checks build tokens the parser never made, such as `braceExpand`, `T_BraceGroup (Id 0) body` or `T_NormalWord (Id 0) l`. Our `&Ast` is read-only, so you can't do that directly. Choose one of these approaches:

1. **Compute on the parts (preferred).** If the synthetic token is only inspected, inspect its parts directly. For example, `getLiteralStringExt f (T_NormalWord (Id 0) l)` is the concatenation of each element of `l`, and you can write a local loop that does it. Keep a `// DIVERGENCE:` note.
2. **Use `support::Scratch`.** Do this when the token has to go into a function that walks the tree:
   - `let mut s = Scratch::new(p.ast);`
   - `let id = s.synth(like, Token::...)` or `s.brace_expand(word)`
   - `let p2 = s.params(p);` gives you params to run against the copy.
   - Before you write a comment, map ids back with `s.haskell_id(id)`.
   - Synthetic nodes have no recorded parent, so use `s.parent(id)` instead of `ast.parent`.
   - Cloning the arena is O(script). Build a `Scratch` only after you know you need one.

## 6. Gate

Run this for every target in your brief:

```
cargo run -q -p shellcheck-conformance -- report --module Analytics.hs --target checkEchoWc
```

The output must be `pass N / fail 0 / blocked 0 / unported 0`, where `N` is the test count in the brief. Always pass `--module`. Without it, names that exist in two modules mix their counts.

For argful checks, the target is the corpus `target` field, which the brief prints already quoted, for example `--target '(checkSudoArgs'`.

After that, `cargo test --workspace` must pass with no warnings.

When a test and the oracle disagree, the test wins, because the gate is unit-level. Use the oracle only as a tiebreaker for behaviour the tests don't cover:

```
printf '%s\n' 'script' | docker run --rm -i koalaman/shellcheck:latest -s bash -f gcc -
```

## 7. Rules

- Create only your check files, under `crates/checks/src/<module>/`. Do not edit:
  - shared files: `support.rs`, `lib.rs`, `build.rs`, `Cargo.toml`
  - another check's file
  - the analyzer, core, parser, checker or cli crates
  - the corpus
- Don't add any dependencies, and don't use `unsafe`.
- If something is missing or blocks you, stop and report BLOCKED or NEEDS_CONTEXT with specifics.
- Commit with this message format:

```
checks: port <names> (Txxx)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
```
