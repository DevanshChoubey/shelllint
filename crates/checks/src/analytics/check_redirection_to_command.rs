// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkRedirectionToCommand`, lines 4243-4249 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkRedirectionToCommand",
    registry: "node",
    order: 81,
    optional: None,
    kind: CheckKind::Node(check_redirection_to_command),
};

// Data.hs commonCommands (not ported elsewhere)
const COMMON_COMMANDS: &[&str] = &[
    "admin", "alias", "ar", "asa", "at", "awk", "basename", "batch",
    "bc", "bg", "break", "c99", "cal", "cat", "cd", "cflow", "chgrp",
    "chmod", "chown", "cksum", "cmp", "colon", "comm", "command",
    "compress", "continue", "cp", "crontab", "csplit", "ctags", "cut",
    "cxref", "date", "dd", "delta", "df", "diff", "dirname", "dot",
    "du", "echo", "ed", "env", "eval", "ex", "exec", "exit", "expand",
    "export", "expr", "fc", "fg", "file", "find", "fold", "fuser",
    "gencat", "get", "getconf", "getopts", "gettext", "grep", "hash",
    "head", "iconv", "ipcrm", "ipcs", "jobs", "join", "kill", "lex",
    "link", "ln", "locale", "localedef", "logger", "logname", "lp",
    "ls", "m4", "mailx", "make", "man", "mesg", "mkdir", "mkfifo",
    "more", "msgfmt", "mv", "newgrp", "ngettext", "nice", "nl", "nm",
    "nohup", "od", "paste", "patch", "pathchk", "pax", "pr", "printf",
    "prs", "ps", "pwd", "read", "readlink", "readonly", "realpath",
    "renice", "return", "rm", "rmdel", "rmdir", "sact", "sccs", "sed",
    "set", "sh", "shift", "sleep", "sort", "split", "strings", "strip",
    "stty", "tabs", "tail", "talk", "tee", "test", "time", "timeout",
    "times", "touch", "tput", "tr", "trap", "tsort", "tty", "type",
    "ulimit", "umask", "unalias", "uname", "uncompress", "unexpand",
    "unget", "uniq", "unlink", "unset", "uucp", "uudecode", "uuencode",
    "uustat", "uux", "val", "vi", "wait", "wc", "what", "who", "write",
    "xargs", "xgettext", "yacc", "zcat",
];

fn check_redirection_to_command(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_IoFile(_, w) = p.ast.get(t) {
        if let Token::T_NormalWord(parts) = p.ast.get(*w) {
            if let &[l] = &parts[..] {
                if let Token::T_Literal(s) = p.ast.get(l) {
                    if COMMON_COMMANDS.contains(&s.as_str()) && s != "file" {
                        // "file" would be confusing
                        c.warn(*w, 2238, "Redirecting to/from command name instead of file. Did you want pipes/xargs (or quote to ignore)?");
                    }
                }
            }
        }
    }
}
