// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/ShellSupport.hs `checkBashisms`, lines 249-536 (e11f964)

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Comments, Params, StackData};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{AssignmentMode, ConditionType, Id, Token};
use shellcheck_core::astlib::{
    get_braced_reference, get_command_name, get_leading_flags, get_literal_string, is_flag, is_glob,
    is_only_redirection, is_variable_name, only_literal_string, oversimplify,
};
use shellcheck_core::interface::Shell;
use shellcheck_core::regex::{match_regex, matches, mk_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkBashisms",
    registry: "shell-support",
    order: 1,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Sh, Shell::Dash, Shell::BusyboxSh], check_bashisms),
};

static ECHO_FLAG_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^-[eEsn]+$"));
static ECHO_BUSYBOX_FLAG_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^-[en]+$"));
static O_FLAG_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex(&format!("^[-+][{SET_OPTIONS}]*o$")));
static VALID_FLAGS_RE: LazyLock<Regex> =
    LazyLock::new(|| mk_regex(&format!("^[-+]([{SET_OPTIONS}]+o?|o)$")));
static STARTS_OPTION_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^(\\+|-[^-])"));
static DOUBLE_DASH_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^--.+$"));
static RADIX_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^[0-9]+#"));

const SET_OPTIONS: &str = "abCefhmnuvxo";
const VAR_CHARS: &str = "_0-9a-zA-Z";

static ADVANCED_EXPANSIONS: LazyLock<Vec<(Regex, i64, &'static str)>> = LazyLock::new(|| {
    vec![
        (mk_regex(&format!("^![{VAR_CHARS}]")), 3053, "indirect expansion is"),
        (mk_regex(&format!("^[{VAR_CHARS}]+\\[.*\\]$")), 3054, "array references are"),
        (mk_regex(&format!("^![{VAR_CHARS}]+\\[[*@]]$")), 3055, "array key expansion is"),
        (mk_regex(&format!("^![{VAR_CHARS}]+[*@]$")), 3056, "name matching prefixes are"),
        (mk_regex(&format!("^[{VAR_CHARS}*@]+(\\[.*\\])?[,^]")), 3059, "case modification is"),
    ]
});
static SIMPLE_EXPANSIONS: LazyLock<Vec<(Regex, i64, &'static str)>> = LazyLock::new(|| {
    vec![
        (mk_regex(&format!("^[{VAR_CHARS}*@]+:[^-=?+]")), 3057, "string indexing is"),
        (mk_regex("^([*@][%#]|#[@*])"), 3058, "string operations on $@/$* are"),
        (mk_regex(&format!("^[{VAR_CHARS}*@]+(\\[.*\\])?/")), 3060, "string replacement is"),
    ]
});

const BASH_VARS: &[&str] = &[
    // This list deliberately excludes $BASH_VERSION as it's often used
    // for shell identification.
    "OSTYPE", "MACHTYPE", "HOSTTYPE", "HOSTNAME",
    "DIRSTACK", "EUID", "UID", "SHLVL", "PIPESTATUS", "SHELLOPTS",
    "_", "BASH", "BASHOPTS", "BASHPID", "BASH_ALIASES", "BASH_ARGC",
    "BASH_ARGV", "BASH_ARGV0", "BASH_CMDS", "BASH_COMMAND",
    "BASH_EXECUTION_STRING", "BASH_LINENO", "BASH_LOADABLES_PATH",
    "BASH_REMATCH", "BASH_SOURCE", "BASH_SUBSHELL", "BASH_VERSINFO",
    "COMP_CWORD", "COMP_KEY", "COMP_LINE", "COMP_POINT", "COMP_TYPE",
    "COMP_WORDBREAKS", "COMP_WORDS", "COPROC", "FUNCNAME", "GROUPS",
    "HISTCMD", "MAPFILE",
];
const BASH_DYNAMIC_VARS: &[&str] =
    &["BASH_MONOSECONDS", "EPOCHREALTIME", "EPOCHSECONDS", "RANDOM", "SECONDS", "SRANDOM"];
const DASH_VARS: &[&str] = &["_"];

const UNSUPPORTED_COMMANDS: &[&str] = &[
    "let", "caller", "builtin", "complete", "compgen", "declare", "dirs", "disown",
    "enable", "mapfile", "readarray", "pushd", "popd", "shopt", "suspend",
    "typeset",
];

const LONG_OPTIONS: &[&str] = &[
    "allexport", "errexit", "ignoreeof", "monitor", "noclobber",
    "noexec", "noglob", "nolog", "notify", "nounset", "pipefail",
    "verbose", "vi", "xtrace",
];

type TestFlag = (i64, &'static [Shell], fn(&str) -> String);

fn bashism_binary_test_flags(op: &str) -> Option<TestFlag> {
    match op {
        "<" | ">" | "\\<" | "\\>" | "<=" | ">=" | "\\<=" | "\\>=" => Some((
            3012,
            &[Shell::Dash, Shell::BusyboxSh],
            |op| format!("lexicographical {op} is"),
        )),
        "==" => Some((3014, &[Shell::BusyboxSh], |op| format!("{op} in place of = is"))),
        "=~" => Some((3015, &[], |op| format!("{op} regex matching is"))),
        _ => None,
    }
}

fn bashism_unary_test_flags(op: &str) -> Option<TestFlag> {
    match op {
        "-v" => Some((3016, &[], |op| {
            format!("test {op} (in place of [ -n \"${{var+x}}\" ]) is")
        })),
        "-a" => Some((3017, &[], |op| format!("unary {op} in place of -e is"))),
        "-o" => Some((3062, &[], |op| format!("test {op} to check options is"))),
        "-R" => Some((3063, &[], |op| format!("test {op} and namerefs in general are"))),
        "-N" => Some((3064, &[], |op| format!("test {op} is"))),
        "-k" => Some((3065, &[Shell::Dash, Shell::BusyboxSh], |op| format!("test {op} is"))),
        "-G" => Some((3066, &[Shell::Dash, Shell::BusyboxSh], |op| format!("test {op} is"))),
        "-O" => Some((3067, &[Shell::Dash, Shell::BusyboxSh], |op| format!("test {op} is"))),
        _ => None,
    }
}

struct Ctx<'a, 'b> {
    p: &'a Params<'b>,
    is_busybox_sh: bool,
    is_dash: bool,
}

impl Ctx<'_, '_> {
    fn warn_msg(&self, c: &mut Comments, id: Id, code: i64, s: &str) {
        if self.is_dash {
            c.err(id, code, format!("In dash, {s} not supported."));
        } else {
            c.warn(id, code, format!("In POSIX sh, {s} undefined."));
        }
    }

    fn is_assigned(&self, var: &str) -> bool {
        self.p
            .variable_flow
            .iter()
            .any(|x| matches!(x, StackData::Assignment(_, _, name, _) if name == var))
    }

    fn is_bash_variable(&self, var: &str) -> bool {
        (BASH_DYNAMIC_VARS.contains(&var) || (BASH_VARS.contains(&var) && !self.is_assigned(var)))
            && !(self.is_dash && DASH_VARS.contains(&var))
    }

    fn check_test_op(
        &self,
        c: &mut Comments,
        table: fn(&str) -> Option<TestFlag>,
        op: &str,
        id: Id,
    ) {
        let Some((code, shells, msg)) = table(op) else { return };
        if shells.contains(&self.p.shell_type) {
            return;
        }
        self.warn_msg(c, id, code, &msg(op));
    }

    fn bashism(&self, c: &mut Comments, t: Id) {
        let ast = self.p.ast;
        let id = t;
        let as_str = |x: Id| get_literal_string(ast, x);
        match ast.get(t) {
            Token::T_ProcSub(..) => {
                if !self.is_busybox_sh {
                    self.warn_msg(c, id, 3001, "process substitution is");
                }
                return;
            }
            Token::T_Extglob(..) => return self.warn_msg(c, id, 3002, "extglob is"),
            Token::T_DollarDoubleQuoted(_) => return self.warn_msg(c, id, 3004, "$\"..\" is"),
            Token::T_ForArithmetic(..) => {
                return self.warn_msg(c, id, 3005, "arithmetic for loops are");
            }
            Token::T_Arithmetic(_) => return self.warn_msg(c, id, 3006, "standalone ((..)) is"),
            Token::T_DollarBracket(_) => {
                return self.warn_msg(c, id, 3007, "$[..] in place of $((..)) is");
            }
            Token::T_SelectIn(..) => return self.warn_msg(c, id, 3008, "select loops are"),
            Token::T_BraceExpansion(_) => return self.warn_msg(c, id, 3009, "brace expansion is"),
            Token::T_Condition(ConditionType::DoubleBracket, _, _) => {
                if !self.is_busybox_sh {
                    self.warn_msg(c, id, 3010, "[[ ]] is");
                }
                return;
            }
            Token::T_HereString(_) => return self.warn_msg(c, id, 3011, "here-strings are"),

            Token::TC_Binary(_, op, _, _) => {
                return self.check_test_op(c, bashism_binary_test_flags, op, id);
            }
            Token::T_SimpleCommand(_, words) if words.len() == 4
                && as_str(words[0]).as_deref() == Some("test")
                && as_str(words[2]).is_some() =>
            {
                let op = as_str(words[2]).unwrap();
                return self.check_test_op(c, bashism_binary_test_flags, &op, id);
            }
            Token::TC_Unary(_, op, _) => {
                return self.check_test_op(c, bashism_unary_test_flags, op, id);
            }
            Token::T_SimpleCommand(_, words) if words.len() == 3
                && as_str(words[0]).as_deref() == Some("test")
                && as_str(words[1]).is_some() =>
            {
                let op = as_str(words[1]).unwrap();
                return self.check_test_op(c, bashism_unary_test_flags, &op, id);
            }

            Token::TA_Unary(op, _) if ["|++", "|--", "++|", "--|"].contains(&op.as_str()) => {
                let s: String = op.chars().filter(|&x| x != '|').collect();
                return self.warn_msg(c, id, 3018, &format!("{s} is"));
            }
            Token::TA_Binary(op, _, _) if op == "**" => {
                return self.warn_msg(c, id, 3019, "exponentials are");
            }
            _ => {}
        }

        // Remaining clauses, in Haskell order. A clause whose pattern or guard
        // fails falls through to the next one.
        if let Token::T_FdRedirect(num, redir) = ast.get(t) {
            if num == "&" {
                if let Token::T_IoFile(op, _) = ast.get(*redir) {
                    if matches!(ast.get(*op), Token::T_Greater) {
                        if !self.is_busybox_sh {
                            self.warn_msg(c, id, 3020, "&> is");
                        }
                        return;
                    }
                }
            }
            if num.is_empty() {
                if let Token::T_IoFile(op, file) = ast.get(*redir) {
                    if matches!(ast.get(*op), Token::T_GREATAND) {
                        if !only_literal_string(ast, *file).chars().all(|x| x.is_ascii_digit()) {
                            self.warn_msg(c, id, 3021, ">& filename (as opposed to >& fd) is");
                        }
                        return;
                    }
                }
            }
            if num.starts_with('{') {
                return self.warn_msg(c, id, 3022, "named file descriptors are");
            }
            if num.chars().all(|x| x.is_ascii_digit()) && num.chars().count() > 1 {
                return self.warn_msg(c, id, 3023, "FDs outside 0-9 are");
            }
        }
        if let Token::T_Assignment(AssignmentMode::Append, ..) = ast.get(t) {
            return self.warn_msg(c, id, 3024, "+= is");
        }
        if let Token::T_IoFile(_, word) = ast.get(t) {
            let file = only_literal_string(ast, *word);
            if file.starts_with("/dev/tcp") || file.starts_with("/dev/udp") {
                return self.warn_msg(c, id, 3025, "/dev/{tcp,udp} is");
            }
        }
        if let Token::T_Glob(s) = ast.get(t) {
            if s.contains("[^") {
                return self.warn_msg(c, id, 3026, "^ in place of ! in glob bracket expressions is");
            }
        }
        if let Token::TA_Variable(s, _) = ast.get(t) {
            if self.is_bash_variable(s) {
                return self.warn_msg(c, id, 3028, &format!("{s} is"));
            }
        }
        if let Token::T_DollarBraced(_, token) = ast.get(t) {
            let s: String = oversimplify(ast, *token).concat();
            let var = get_braced_reference(&s);
            let check = |c: &mut Comments, list: &[(Regex, i64, &str)]| {
                for (re, code, feature) in list {
                    if match_regex(re, &s).is_some() {
                        self.warn_msg(c, id, *code, feature);
                    }
                }
            };
            if !self.is_busybox_sh {
                check(c, &SIMPLE_EXPANSIONS);
            }
            check(c, &ADVANCED_EXPANSIONS);
            if self.is_bash_variable(&var) {
                self.warn_msg(c, id, 3028, &format!("{var} is"));
            }
            return;
        }
        if let Token::T_Pipe(s) = ast.get(t) {
            if s == "|&" {
                return self.warn_msg(c, id, 3029, "|& in place of 2>&1 | is");
            }
        }
        if let Token::T_Array(_) = ast.get(t) {
            return self.warn_msg(c, id, 3030, "arrays are");
        }
        if let Token::T_IoFile(_, word) = ast.get(t) {
            if is_glob(ast, *word) {
                return self.warn_msg(c, id, 3031, "redirecting to/from globs is");
            }
        }
        if let Token::T_CoProc(..) = ast.get(t) {
            return self.warn_msg(c, id, 3032, "coproc is");
        }
        if let Token::T_Function(_, _, name, _) = ast.get(t) {
            if !is_variable_name(name) {
                return self.warn_msg(c, id, 3033, "naming functions outside [a-zA-Z_][a-zA-Z0-9_]* is");
            }
        }
        if let Token::T_DollarExpansion(l) = ast.get(t) {
            if l.len() == 1 && is_only_redirection(ast, l[0]) {
                return self.warn_msg(c, id, 3034, "$(<file) to read files is");
            }
        }
        if let Token::T_Backticked(l) = ast.get(t) {
            if l.len() == 1 && is_only_redirection(ast, l[0]) {
                return self.warn_msg(c, id, 3035, "`<file` to read files is");
            }
        }

        if let Token::T_SimpleCommand(_, words) = ast.get(t) {
            if words.len() >= 2 {
                let (cmd, arg) = (words[0], words[1]);
                let arg_string: String = oversimplify(ast, arg).concat();
                if shellcheck_analyzer::analyzerlib::is_command(ast, t, "echo")
                    && matches(&arg_string, &ECHO_FLAG_RE)
                {
                    if self.is_busybox_sh {
                        if !matches(&arg_string, &ECHO_BUSYBOX_FLAG_RE) {
                            self.warn_msg(c, arg, 3036, "echo flags besides -n and -e");
                        }
                    } else if self.is_dash {
                        if arg_string != "-n" {
                            self.warn_msg(c, arg, 3036, "echo flags besides -n");
                        }
                    } else {
                        self.warn_msg(c, arg, 3037, "echo flags are");
                    }
                    return;
                }
                if get_literal_string(ast, cmd).as_deref() == Some("exec")
                    && oversimplify(ast, arg).concat().starts_with('-')
                {
                    return self.warn_msg(c, arg, 3038, "exec flags are");
                }
            }
            if shellcheck_analyzer::analyzerlib::is_command(ast, t, "let") {
                return self.warn_msg(c, id, 3039, "'let' is");
            }
            if !words.is_empty() && shellcheck_analyzer::analyzerlib::is_command(ast, t, "set") {
                if !self.is_dash {
                    self.check_options(c, &get_literal_args(ast, &words[1..]));
                }
                return;
            }
            if !words.is_empty() {
                self.general_command(c, t, words);
                return;
            }
        }

        if let Token::T_SourceCommand(src, _) = ast.get(t) {
            if get_command_name(ast, *src).as_deref() == Some("source") {
                if !self.is_busybox_sh {
                    self.warn_msg(c, id, 3051, "'source' in place of '.' is");
                }
                return;
            }
        }
        if let Token::TA_Expansion(l) = ast.get(t) {
            if let Some(&first) = l.first() {
                if let Token::T_Literal(s) = ast.get(first) {
                    if matches(s, &RADIX_RE) {
                        self.warn_msg(c, first, 3052, "arithmetic base conversion is");
                    }
                }
            }
        }
    }

    // Check a flag-option pair (such as -o errexit)
    fn check_options(&self, c: &mut Comments, args: &[(Id, String)]) {
        match args {
            [flag @ (_, flag_s), (oid, opt), rest @ ..] => {
                if matches(flag_s, &O_FLAG_RE) {
                    if !LONG_OPTIONS.contains(&opt.as_str()) {
                        self.warn_msg(c, *oid, 3040, &format!("set option {opt} is"));
                    }
                    let mut v = vec![flag.clone()];
                    v.extend_from_slice(rest);
                    self.check_flags(c, &v);
                } else {
                    self.check_flags(c, args);
                }
            }
            [_] => self.check_flags(c, args),
            _ => {}
        }
    }

    // Check that each option in a sequence of flags
    // (such as -aveo) is valid
    fn check_flags(&self, c: &mut Comments, args: &[(Id, String)]) {
        let Some(((fid, flag), rest)) = args.split_first() else { return };
        if matches(flag, &STARTS_OPTION_RE) {
            if !matches(flag, &VALID_FLAGS_RE) {
                let options_set: HashSet<char> = SET_OPTIONS.chars().collect();
                for letter in flag.chars().skip(1) {
                    if !options_set.contains(&letter) {
                        self.warn_msg(c, *fid, 3041, &format!("set flag -{letter} is"));
                    }
                }
            }
            self.check_options(c, rest);
        } else if matches(flag, &DOUBLE_DASH_RE) {
            self.warn_msg(c, *fid, 3042, &format!("set flag {flag} is"));
            self.check_options(c, rest);
        }
        // Either a word that doesn't start with a dash, or simply '--',
        // so stop checking.
    }

    fn general_command(&self, c: &mut Comments, t: Id, words: &[Id]) {
        let ast = self.p.ast;
        let id = t;
        let cmd = words[0];
        let rest = &words[1..];
        let name = get_command_name(ast, t).unwrap_or_default();
        let flags = get_leading_flags(ast, t);

        if name == "local" && !self.is_dash {
            // This is so commonly accepted that we'll make it a special case
            self.warn_msg(c, id, 3043, "'local' is");
        }
        if UNSUPPORTED_COMMANDS.contains(&name.as_str()) {
            self.warn_msg(c, id, 3044, &format!("'{name}' is"));
        }
        if let Some(allowed) = self.allowed_flags(&name) {
            if let Some((word, flag)) =
                flags.iter().find(|(_, f)| !f.is_empty() && !allowed.contains(&f.as_str()))
            {
                self.warn_msg(c, *word, 3045, &format!("{name} -{flag} is"));
            }
        }

        if name == "source" && !self.is_busybox_sh {
            self.warn_msg(c, id, 3046, "'source' in place of '.' is");
        }
        if name == "trap" {
            for &token in rest.iter().skip(1) {
                let Some(s) = get_literal_string(ast, token) else { continue };
                let upper: String = s
                    .chars()
                    .map(|ch| {
                        let mut u = ch.to_uppercase();
                        match (u.next(), u.next()) {
                            (Some(x), None) => x,
                            _ => ch,
                        }
                    })
                    .collect();
                if ["ERR", "DEBUG", "RETURN"].contains(&upper.as_str()) {
                    self.warn_msg(c, token, 3047, &format!("trapping {s} is"));
                }
                if !self.is_busybox_sh && upper.starts_with("SIG") {
                    self.warn_msg(c, token, 3048, "prefixing signal names with 'SIG' is");
                }
                if !self.is_dash && upper != s {
                    self.warn_msg(c, token, 3049, "using lower/mixed case for signal names is");
                }
            }
        }

        if name == "printf" {
            // flags are covered by allowedFlags
            if let Some(&format) = rest.first() {
                let literal = only_literal_string(ast, format);
                if literal.contains("%q") {
                    self.warn_msg(c, format, 3050, "printf %q is");
                }
            }
        }

        if name == "read" && rest.iter().all(|&x| is_flag(ast, x)) {
            self.warn_msg(c, cmd, 3061, "read without a variable is");
        }
    }

    fn allowed_flags(&self, name: &str) -> Option<Vec<&'static str>> {
        let (is_dash, is_busybox_sh) = (self.is_dash, self.is_busybox_sh);
        Some(match name {
            "cd" => vec!["L", "P"],
            "exec" => vec![],
            "export" => vec!["p"],
            "hash" => if is_dash { vec!["r", "v"] } else { vec!["r"] },
            "jobs" => vec!["l", "p"],
            "printf" => vec![],
            "read" => if is_dash || is_busybox_sh { vec!["r", "p"] } else { vec!["r"] },
            "readonly" => vec!["p"],
            "trap" => vec![],
            "type" => if is_busybox_sh { vec!["p"] } else { vec![] },
            "ulimit" => {
                if is_dash {
                    vec!["H", "S", "a", "c", "d", "f", "l", "m", "n", "p", "r", "s", "t", "v", "w"]
                } else {
                    vec!["H", "S", "a", "c", "d", "f", "n", "s", "t", "v"] // POSIX.1-2024
                }
            }
            "umask" => vec!["S"],
            "unset" => vec!["f", "v"],
            "wait" => vec![],
            _ => return None,
        })
    }
}

// Get the literal options from a list of arguments,
// up until the first non-literal one
fn get_literal_args(ast: &shellcheck_core::ast::Ast, args: &[Id]) -> Vec<(Id, String)> {
    let mut out = Vec::new();
    for &a in args {
        match get_literal_string(ast, a) {
            Some(s) => out.push((a, s)),
            None => break,
        }
    }
    out
}

fn check_bashisms(p: &Params<'_>, t: Id, c: &mut Comments) {
    let is_busybox_sh = p.shell_type == Shell::BusyboxSh;
    let is_dash = p.shell_type == Shell::Dash || is_busybox_sh;
    let ctx = Ctx { p, is_busybox_sh, is_dash };
    ctx.bashism(c, t);
}
