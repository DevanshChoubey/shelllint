// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Data.hs (e11f964)

use crate::interface::Shell;

// DIVERGENCE: Haskell takes this from Paths_ShellCheck; the pinned reference
// is ShellCheck 0.11.0 (ShellCheck.cabal).
pub const SHELLCHECK_VERSION: &str = "0.11.0"; // VERSIONSTRING

pub const INTERNAL_VARIABLES: &[&str] = &[
    // Generic
    "",
    "_",
    "rest",
    "REST",
    // POSIX
    "CDPATH",
    "ENV",
    "FCEDIT",
    "HISTFILE",
    "HISTSIZE",
    "HOME",
    "IFS",
    "LANG",
    "LC_ALL",
    "LC_COLLATE",
    "LC_CTYPE",
    "LC_MESSAGES",
    "LC_MONETARY",
    "LC_NUMERIC",
    "LC_TIME",
    "MAIL",
    "MAILCHECK",
    "MAILPATH",
    "OLDPWD",
    "OPTARG",
    "OPTIND",
    "PATH",
    "PWD",
    // Bash
    "BASH",
    "BASHOPTS",
    "BASHPID",
    "BASH_ALIASES",
    "BASH_ARGC",
    "BASH_ARGV",
    "BASH_ARGV0",
    "BASH_CMDS",
    "BASH_COMMAND",
    "BASH_EXECUTION_STRING",
    "BASH_LINENO",
    "BASH_LOADABLES_PATH",
    "BASH_REMATCH",
    "BASH_SOURCE",
    "BASH_SUBSHELL",
    "BASH_VERSINFO",
    "BASH_VERSION",
    "COMP_CWORD",
    "COMP_KEY",
    "COMP_LINE",
    "COMP_POINT",
    "COMP_TYPE",
    "COMP_WORDBREAKS",
    "COMP_WORDS",
    "COPROC",
    "DIRSTACK",
    "EPOCHREALTIME",
    "EPOCHSECONDS",
    "EUID",
    "FUNCNAME",
    "GROUPS",
    "HISTCMD",
    "HOSTNAME",
    "HOSTTYPE",
    "MACHTYPE",
    "MAPFILE",
    "OSTYPE",
    "PIPESTATUS",
    "RANDOM",
    "READLINE_ARGUMENT",
    "READLINE_LINE",
    "READLINE_MARK",
    "READLINE_POINT",
    "REPLY",
    "SECONDS",
    "SHELLOPTS",
    "SHLVL",
    "SRANDOM",
    "UID",
    "BASH_COMPAT",
    "BASH_ENV",
    "BASH_XTRACEFD",
    "CHILD_MAX",
    "COLUMNS",
    "COMPREPLY",
    "EMACS",
    "EXECIGNORE",
    "FIGNORE",
    "FUNCNEST",
    "GLOBIGNORE",
    "HISTCONTROL",
    "HISTFILESIZE",
    "HISTIGNORE",
    "HISTTIMEFORMAT",
    "HOSTFILE",
    "IGNOREEOF",
    "INPUTRC",
    "INSIDE_EMACS",
    "LINES",
    "OPTERR",
    "POSIXLY_CORRECT",
    "PROMPT_COMMAND",
    "PROMPT_DIRTRIM",
    "PS0",
    "PS1",
    "PS2",
    "PS3",
    "PS4",
    "SHELL",
    "TIMEFORMAT",
    "TMOUT",
    "BASH_MONOSECONDS",
    "BASH_TRAPSIG",
    "GLOBSORT",
    "auto_resume",
    "histchars",
    // Other
    "USER",
    "TZ",
    "TERM",
    "LOGNAME",
    "LD_LIBRARY_PATH",
    "LANGUAGE",
    "DISPLAY",
    "HOSTNAME",
    "KRB5CCNAME",
    "LINENO",
    "PPID",
    "TMPDIR",
    "XAUTHORITY",
    // Ksh
    ".sh.version",
    // shflags
    "FLAGS_ARGC",
    "FLAGS_ARGV",
    "FLAGS_ERROR",
    "FLAGS_FALSE",
    "FLAGS_HELP",
    "FLAGS_PARENT",
    "FLAGS_RESERVED",
    "FLAGS_TRUE",
    "FLAGS_VERSION",
    "flags_error",
    "flags_return",
];

pub const SPECIAL_INTEGER_VARIABLES: &[&str] = &["$", "?", "!", "#"];

/// `"-" : specialIntegerVariables`
pub const SPECIAL_VARIABLES_WITHOUT_SPACES: &[&str] = &["-", "$", "?", "!", "#"];

/// `specialVariablesWithoutSpaces ++ [...]`
pub const VARIABLES_WITHOUT_SPACES: &[&str] = &[
    "-",
    "$",
    "?",
    "!",
    "#",
    "BASHPID",
    "BASH_ARGC",
    "BASH_LINENO",
    "BASH_SUBSHELL",
    "EUID",
    "EPOCHREALTIME",
    "EPOCHSECONDS",
    "LINENO",
    "OPTIND",
    "PPID",
    "RANDOM",
    "READLINE_ARGUMENT",
    "READLINE_MARK",
    "READLINE_POINT",
    "SECONDS",
    "SHELLOPTS",
    "SHLVL",
    "SRANDOM",
    "UID",
    "COLUMNS",
    "HISTFILESIZE",
    "HISTSIZE",
    "LINES",
    "BASH_MONOSECONDS",
    "BASH_TRAPSIG",
    // shflags
    "FLAGS_ERROR",
    "FLAGS_FALSE",
    "FLAGS_TRUE",
];

/// `specialVariablesWithoutSpaces ++ ["@", "*"]`
pub const SPECIAL_VARIABLES: &[&str] = &["-", "$", "?", "!", "#", "@", "*"];

/// `specialVariables ++ ["0" .. "9"]`
pub const UNBRACED_VARIABLES: &[&str] =
    &["-", "$", "?", "!", "#", "@", "*", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

pub const ARRAY_VARIABLES: &[&str] = &[
    "BASH_ALIASES",
    "BASH_ARGC",
    "BASH_ARGV",
    "BASH_CMDS",
    "BASH_LINENO",
    "BASH_REMATCH",
    "BASH_SOURCE",
    "BASH_VERSINFO",
    "COMP_WORDS",
    "COPROC",
    "DIRSTACK",
    "FUNCNAME",
    "GROUPS",
    "MAPFILE",
    "PIPESTATUS",
    "COMPREPLY",
];

pub const COMMON_COMMANDS: &[&str] = &[
    "admin",
    "alias",
    "ar",
    "asa",
    "at",
    "awk",
    "basename",
    "batch",
    "bc",
    "bg",
    "break",
    "c99",
    "cal",
    "cat",
    "cd",
    "cflow",
    "chgrp",
    "chmod",
    "chown",
    "cksum",
    "cmp",
    "colon",
    "comm",
    "command",
    "compress",
    "continue",
    "cp",
    "crontab",
    "csplit",
    "ctags",
    "cut",
    "cxref",
    "date",
    "dd",
    "delta",
    "df",
    "diff",
    "dirname",
    "dot",
    "du",
    "echo",
    "ed",
    "env",
    "eval",
    "ex",
    "exec",
    "exit",
    "expand",
    "export",
    "expr",
    "fc",
    "fg",
    "file",
    "find",
    "fold",
    "fuser",
    "gencat",
    "get",
    "getconf",
    "getopts",
    "gettext",
    "grep",
    "hash",
    "head",
    "iconv",
    "ipcrm",
    "ipcs",
    "jobs",
    "join",
    "kill",
    "lex",
    "link",
    "ln",
    "locale",
    "localedef",
    "logger",
    "logname",
    "lp",
    "ls",
    "m4",
    "mailx",
    "make",
    "man",
    "mesg",
    "mkdir",
    "mkfifo",
    "more",
    "msgfmt",
    "mv",
    "newgrp",
    "ngettext",
    "nice",
    "nl",
    "nm",
    "nohup",
    "od",
    "paste",
    "patch",
    "pathchk",
    "pax",
    "pr",
    "printf",
    "prs",
    "ps",
    "pwd",
    "read",
    "readlink",
    "readonly",
    "realpath",
    "renice",
    "return",
    "rm",
    "rmdel",
    "rmdir",
    "sact",
    "sccs",
    "sed",
    "set",
    "sh",
    "shift",
    "sleep",
    "sort",
    "split",
    "strings",
    "strip",
    "stty",
    "tabs",
    "tail",
    "talk",
    "tee",
    "test",
    "time",
    "timeout",
    "times",
    "touch",
    "tput",
    "tr",
    "trap",
    "tsort",
    "tty",
    "type",
    "ulimit",
    "umask",
    "unalias",
    "uname",
    "uncompress",
    "unexpand",
    "unget",
    "uniq",
    "unlink",
    "unset",
    "uucp",
    "uudecode",
    "uuencode",
    "uustat",
    "uux",
    "val",
    "vi",
    "wait",
    "wc",
    "what",
    "who",
    "write",
    "xargs",
    "xgettext",
    "yacc",
    "zcat",
];

pub const NON_READING_COMMANDS: &[&str] = &[
    "alias", "basename", "bg", "cal", "cd", "chgrp", "chmod", "chown", "cp", "du", "echo", "export", "fg", "fuser",
    "getconf", "getopt", "getopts", "ipcrm", "ipcs", "jobs", "kill", "ln", "ls", "locale", "mv", "printf", "ps", "pwd",
    "readlink", "realpath", "renice", "rm", "rmdir", "set", "sleep", "touch", "trap", "ulimit", "unalias", "uname",
];

pub const SAMPLE_WORDS: &[&str] = &[
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliett", "kilo", "lima",
    "mike", "november", "oscar", "papa", "quebec", "romeo", "sierra", "tango", "uniform", "victor", "whiskey", "xray",
    "yankee", "zulu",
];

pub const BINARY_TEST_OPS: &[&str] = &[
    "-nt", "-ot", "-ef", "==", "!=", "<=", ">=", "-eq", "-ne", "-lt", "-le", "-gt", "-ge", "=~", ">", "<", "=", "\\<",
    "\\>", "\\<=", "\\>=",
];

pub const ARITHMETIC_BINARY_TEST_OPS: &[&str] = &["-eq", "-ne", "-lt", "-le", "-gt", "-ge"];

pub const UNARY_TEST_OPS: &[&str] = &[
    "!", "-a", "-b", "-c", "-d", "-e", "-f", "-g", "-h", "-L", "-k", "-p", "-r", "-s", "-S", "-t", "-u", "-w", "-x",
    "-O", "-G", "-N", "-z", "-n", "-o", "-v", "-R",
];

pub fn shell_for_executable(name: &str) -> Option<Shell> {
    match name {
        "sh" => Some(Shell::Sh),
        "bash" => Some(Shell::Bash),
        "bats" => Some(Shell::Bash),
        "busybox" => Some(Shell::BusyboxSh), // Used for directives and --shell=busybox
        "busybox sh" => Some(Shell::BusyboxSh),
        "busybox ash" => Some(Shell::BusyboxSh),
        "dash" => Some(Shell::Dash),
        "ash" => Some(Shell::Dash), // There's also a warning for this.
        "ksh" => Some(Shell::Ksh),
        "ksh88" => Some(Shell::Ksh),
        "ksh93" => Some(Shell::Ksh),
        "oksh" => Some(Shell::Ksh),
        _ => None,
    }
}

pub const FLAGS_FOR_READ: &str = "sreu:n:N:i:p:a:t:";
pub const FLAGS_FOR_MAPFILE: &str = "d:n:O:s:u:C:c:t";

pub const DECLARING_COMMANDS: &[&str] = &["local", "declare", "export", "readonly", "typeset", "let"];

pub const PRIVILEGE_ELEVATION_COMMANDS: &[&str] = &["sudo", "doas", "run0"];

#[cfg(test)]
mod tests {
    use super::*;

    fn concat(parts: &[&[&'static str]]) -> Vec<&'static str> {
        parts.iter().flat_map(|p| p.iter().copied()).collect()
    }

    // The derived tables are written out literally; check they still equal
    // the Haskell definitions in terms of each other.
    #[test]
    fn derived_tables_match_their_definitions() {
        assert_eq!(SPECIAL_VARIABLES_WITHOUT_SPACES, concat(&[&["-"], SPECIAL_INTEGER_VARIABLES]).as_slice());
        assert_eq!(
            &VARIABLES_WITHOUT_SPACES[..SPECIAL_VARIABLES_WITHOUT_SPACES.len()],
            SPECIAL_VARIABLES_WITHOUT_SPACES
        );
        assert_eq!(SPECIAL_VARIABLES, concat(&[SPECIAL_VARIABLES_WITHOUT_SPACES, &["@", "*"]]).as_slice());
        assert_eq!(
            UNBRACED_VARIABLES,
            concat(&[SPECIAL_VARIABLES, &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]]).as_slice()
        );
    }

    #[test]
    fn table_sizes() {
        assert_eq!(INTERNAL_VARIABLES.len(), 138);
        assert_eq!(VARIABLES_WITHOUT_SPACES.len(), 33);
        assert_eq!(COMMON_COMMANDS.len(), 167);
    }

    #[test]
    fn shells_for_executables() {
        assert_eq!(shell_for_executable("bats"), Some(Shell::Bash));
        assert_eq!(shell_for_executable("busybox ash"), Some(Shell::BusyboxSh));
        assert_eq!(shell_for_executable("ash"), Some(Shell::Dash));
        assert_eq!(shell_for_executable("zsh"), None);
    }
}
