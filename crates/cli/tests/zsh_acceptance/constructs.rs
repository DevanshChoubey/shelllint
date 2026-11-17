// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only acceptance tests for zsh support (no ShellCheck counterpart).
//
// One group per construct C01-C29 of the zsh design. Every snippet is our own and was
// checked with `zsh -n` (zsh 5.9) when written; the `snippets_are_what_zsh_says` test
// re-checks them whenever zsh is installed. For each construct:
//   * every positive snippet parses under `--shell=zsh` with no SC1xxx at all,
//   * the near-misses (zsh rejects them too) are still reported, not silently accepted,
//   * the same snippets under bash/sh/ksh give exactly the output recorded from the
//     tree before zsh support (dialect guard: no zsh rule may leak into other shells).
use crate::support::*;

pub struct Snippet {
    pub construct: &'static str,
    pub label: &'static str,
    pub code: &'static str,
    /// Codes under bash, sh and ksh on the tree before zsh support (report order).
    pub bash: &'static [u32],
    pub sh: &'static [u32],
    pub ksh: &'static [u32],
}

pub struct NearMiss {
    pub construct: &'static str,
    pub code: &'static str,
}

pub static SNIPPETS: &[Snippet] = &[
    Snippet {
        construct: "C01",
        label: "minimal",
        code: "() { echo hi; }\n",
        bash: &[1073, 1072],
        sh: &[1073, 1072],
        ksh: &[1073, 1072],
    },
    Snippet {
        construct: "C01",
        label: "in_function_with_args",
        code: "f() {\n  () { echo \"$1\" } a b\n}\nf\n",
        bash: &[1009, 1073, 1072],
        sh: &[1009, 1073, 1072],
        ksh: &[1009, 1073, 1072],
    },
    Snippet {
        construct: "C01",
        label: "function_keyword",
        code: "function { local x=1; echo \"$x\" }\n",
        bash: &[1083, 2154, 1083],
        sh: &[1083, 2154, 1083],
        ksh: &[1083, 2154, 1083],
    },
    Snippet {
        construct: "C01",
        label: "piped_with_args",
        code: "() { echo \"$1\"; } \"$@\" | cat\n",
        bash: &[1073, 1072],
        sh: &[1073, 1072],
        ksh: &[1073, 1072],
    },
    Snippet { construct: "C02", label: "amp_pipe", code: "sleep 1 &|\n", bash: &[1070], sh: &[1070], ksh: &[1070] },
    Snippet {
        construct: "C02",
        label: "amp_bang",
        code: "sleep 1 &!\n",
        bash: &[1035, 1072],
        sh: &[1035, 1072],
        ksh: &[1035, 1072],
    },
    Snippet {
        construct: "C02",
        label: "group_in_function",
        code: "f() {\n  { sleep 1; echo x; } &|\n}\n",
        bash: &[1009, 1073, 1056, 1072],
        sh: &[1009, 1073, 1056, 1072],
        ksh: &[1009, 1073, 1056, 1072],
    },
    Snippet {
        construct: "C02",
        label: "subshell",
        code: "(sleep 1; echo hi) &!\n",
        bash: &[1035, 1072],
        sh: &[1035, 1072],
        ksh: &[1035, 1072],
    },
    Snippet {
        construct: "C03",
        label: "diff_two",
        code: "diff =(echo a) =(echo b)\n",
        bash: &[2283, 1036, 1088],
        sh: &[2283, 1036, 1088],
        ksh: &[2283, 1036, 1088],
    },
    Snippet {
        construct: "C03",
        label: "in_function",
        code: "f() {\n  cat =(print -r -- \"$1\")\n}\nf x\n",
        bash: &[1009, 1073, 1036, 1056, 1072],
        sh: &[1009, 1073, 1036, 1056, 1072],
        ksh: &[1009, 1073, 1036, 1056, 1072],
    },
    Snippet {
        construct: "C03",
        label: "mixed_with_process_subst",
        code: "diff =(sort a) <(sort b)\n",
        bash: &[2283, 1036, 1088],
        sh: &[2283, 1036, 1088],
        ksh: &[2283, 1036, 1088],
    },
    Snippet {
        construct: "C03",
        label: "as_redirect_source",
        code: "while read -r l; do echo \"$l\"; done < =(echo hi)\n",
        bash: &[1036, 1088],
        sh: &[1036, 1088],
        ksh: &[1036, 1088],
    },
    Snippet {
        construct: "C04",
        label: "alternation",
        code: "[[ $x == (a|b)* ]]\n",
        bash: &[1073, 1027, 1036, 1072],
        sh: &[1073, 1027, 1036, 1072],
        ksh: &[1073, 1027, 1036, 1072],
    },
    Snippet {
        construct: "C04",
        label: "backref_flag_group",
        code: "[[ $x == (#b)(a)(b) ]]\n",
        bash: &[1073, 1027, 1036, 1072],
        sh: &[1073, 1027, 1036, 1072],
        ksh: &[1073, 1027, 1036, 1072],
    },
    Snippet {
        construct: "C04",
        label: "repeat_hash_hash",
        code: "[[ $x == (foo|bar)##baz ]]\n",
        bash: &[1073, 1027, 1036, 1072],
        sh: &[1073, 1027, 1036, 1072],
        ksh: &[1073, 1027, 1036, 1072],
    },
    Snippet {
        construct: "C04",
        label: "tilde_param_in_pattern",
        code: "pat='a*'\n[[ $x == ${~pat} ]]\n",
        bash: &[2034, 2154, 2053, 2296],
        sh: &[2034, 3010, 2154, 3014, 2053, 2296],
        ksh: &[2034, 2154, 2053, 2296],
    },
    Snippet {
        construct: "C05",
        label: "any_number",
        code: "[[ $x == <-> ]]\n",
        bash: &[1073, 1027, 1072],
        sh: &[1073, 1027, 1072],
        ksh: &[1073, 1027, 1072],
    },
    Snippet {
        construct: "C05",
        label: "bounded",
        code: "[[ $p == <0-65535> ]]\n",
        bash: &[1073, 1027, 1072],
        sh: &[1073, 1027, 1072],
        ksh: &[1073, 1027, 1072],
    },
    Snippet {
        construct: "C05",
        label: "case_label",
        code: "case $x in <->) echo n;; esac\n",
        bash: &[1073, 1072],
        sh: &[1073, 1072],
        ksh: &[1073, 1072],
    },
    Snippet {
        construct: "C05",
        label: "in_function_with_and",
        code: "f() {\n  [[ $1 == <1-9> && $2 == <-> ]] && echo ok\n}\n",
        bash: &[1009, 1073, 1027, 1072],
        sh: &[1009, 1073, 1027, 1072],
        ksh: &[1009, 1073, 1027, 1072],
    },
    Snippet {
        construct: "C06",
        label: "nullglob_qualifier",
        code: "print x(N)\n",
        bash: &[1036, 1088],
        sh: &[1036, 1088],
        ksh: &[1036, 1088],
    },
    Snippet {
        construct: "C06",
        label: "recursive_dot_tail",
        code: "ls **/*(.N:t)\n",
        bash: &[2035],
        sh: &[2035, 3002],
        ksh: &[2035],
    },
    Snippet {
        construct: "C06",
        label: "loop_mtime",
        code: "for f in *(Nm-1); do echo \"$f\"; done\n",
        bash: &[],
        sh: &[3002],
        ksh: &[],
    },
    Snippet {
        construct: "C06",
        label: "array_assign_head",
        code: "files=(*(-.N:h))\n",
        bash: &[2034],
        sh: &[2034, 3030, 3002],
        ksh: &[2034],
    },
    Snippet {
        construct: "C07",
        label: "group_label",
        code: "case $x in (a|b)*) :;; esac\n",
        bash: &[1009, 1073, 1072, 1074],
        sh: &[1009, 1073, 1072, 1074],
        ksh: &[1009, 1073, 1072, 1074],
    },
    Snippet {
        construct: "C07",
        label: "number_and_group",
        code: "case $x in <->) :;; (x|y) :;; esac\n",
        bash: &[1073, 1072],
        sh: &[1073, 1072],
        ksh: &[1073, 1072],
    },
    Snippet {
        construct: "C07",
        label: "multiline_in_function",
        code: "f() {\n  case $1 in\n    (a|b)*) echo ab ;;\n    *) echo other ;;\n  esac\n}\n",
        bash: &[1009, 1073, 1072, 1074],
        sh: &[1009, 1073, 1072, 1074],
        ksh: &[1009, 1073, 1072, 1074],
    },
    Snippet {
        construct: "C08",
        label: "paren_list",
        code: "for f (a b) echo $f\n",
        bash: &[1073, 1058, 1072],
        sh: &[1073, 1058, 1072],
        ksh: &[1073, 1058, 1072],
    },
    Snippet {
        construct: "C08",
        label: "brace_body",
        code: "for f (a b); { echo $f }\n",
        bash: &[1073, 1058, 1072],
        sh: &[1073, 1058, 1072],
        ksh: &[1073, 1058, 1072],
    },
    Snippet {
        construct: "C08",
        label: "body_on_next_line",
        code: "for f (a b c)\n  echo $f\n",
        bash: &[1073, 1058, 1072],
        sh: &[1073, 1058, 1072],
        ksh: &[1073, 1058, 1072],
    },
    Snippet {
        construct: "C08",
        label: "in_with_semicolon_cmd",
        code: "for f (a b); echo $f\n",
        bash: &[1073, 1058, 1072],
        sh: &[1073, 1058, 1072],
        ksh: &[1073, 1058, 1072],
    },
    Snippet {
        construct: "C09",
        label: "minimal",
        code: "for k v in a 1 b 2; do echo $k $v; done\n",
        bash: &[1073, 1058, 1072],
        sh: &[1073, 1058, 1072],
        ksh: &[1073, 1058, 1072],
    },
    Snippet {
        construct: "C09",
        label: "in_function",
        code: "f() {\n  for k v in $@; do print -r -- $k $v; done\n}\n",
        bash: &[1009, 1073, 1058, 1072],
        sh: &[1009, 1073, 1058, 1072],
        ksh: &[1009, 1073, 1058, 1072],
    },
    Snippet {
        construct: "C09",
        label: "assoc_kv",
        code: "typeset -A m\nfor k v in ${(kv)m}; do :; done\n",
        bash: &[1073, 1058, 1072],
        sh: &[1073, 1058, 1072],
        ksh: &[1073, 1058, 1072],
    },
    Snippet {
        construct: "C10",
        label: "minimal",
        code: "{ :; } always { :; }\n",
        bash: &[1070, 1141],
        sh: &[1070, 1141],
        ksh: &[1070, 1141],
    },
    Snippet {
        construct: "C10",
        label: "cleanup_no_semicolon",
        code: "{ echo a } always { echo cleanup }\n",
        bash: &[1073, 1083, 1083, 1083, 1056, 1072],
        sh: &[1073, 1083, 1083, 1083, 1056, 1072],
        ksh: &[1073, 1083, 1083, 1083, 1056, 1072],
    },
    Snippet {
        construct: "C10",
        label: "in_function",
        code: "f() {\n  {\n    echo work\n  } always {\n    echo cleanup\n  }\n}\n",
        bash: &[1009, 1073, 1056, 1072, 1141],
        sh: &[1009, 1073, 1056, 1072, 1141],
        ksh: &[1009, 1073, 1056, 1072, 1141],
    },
    Snippet {
        construct: "C11",
        label: "function_body",
        code: "f() { echo hi }\n",
        bash: &[1009, 1073, 1083, 1056, 1072],
        sh: &[1009, 1073, 1083, 1056, 1072],
        ksh: &[1009, 1073, 1083, 1056, 1072],
    },
    Snippet {
        construct: "C11",
        label: "plain_group",
        code: "{ echo hi }\n",
        bash: &[1073, 1083, 1056, 1072],
        sh: &[1073, 1083, 1056, 1072],
        ksh: &[1073, 1083, 1056, 1072],
    },
    Snippet {
        construct: "C11",
        label: "two_commands",
        code: "f() { echo a; echo b }\n",
        bash: &[1009, 1073, 1083, 1056, 1072],
        sh: &[1009, 1073, 1083, 1056, 1072],
        ksh: &[1009, 1073, 1083, 1056, 1072],
    },
    Snippet {
        construct: "C11",
        label: "redirect_last",
        code: "f() { echo hi > /dev/null }\n",
        bash: &[1009, 1073, 1083, 1056, 1072],
        sh: &[1009, 1073, 1083, 1056, 1072],
        ksh: &[1009, 1073, 1083, 1056, 1072],
    },
    Snippet {
        construct: "C12",
        label: "function_spaced",
        code: "f() { }\n",
        bash: &[1009, 1073, 1055, 1072],
        sh: &[1009, 1073, 1055, 1072],
        ksh: &[1009, 1073, 1055, 1072],
    },
    Snippet {
        construct: "C12",
        label: "function_tight",
        code: "f() {}\n",
        bash: &[1009, 1073, 1054, 1055, 1072],
        sh: &[1009, 1073, 1054, 1055, 1072],
        ksh: &[1009, 1073, 1054, 1055, 1072],
    },
    Snippet {
        construct: "C12",
        label: "anonymous",
        code: "() { }\n",
        bash: &[1073, 1072],
        sh: &[1073, 1072],
        ksh: &[1073, 1072],
    },
    Snippet {
        construct: "C12",
        label: "function_keyword",
        code: "function f { }\n",
        bash: &[1009, 1073, 1055, 1072],
        sh: &[1009, 1073, 1055, 1072],
        ksh: &[1009, 1073, 1055, 1072],
    },
    Snippet {
        construct: "C13",
        label: "two_names",
        code: "function a b { :; }\n",
        bash: &[1073, 1064, 1072],
        sh: &[1073, 1064, 1072],
        ksh: &[1073, 1064, 1072],
    },
    Snippet {
        construct: "C13",
        label: "three_names_no_semi",
        code: "function a b c { echo hi }\n",
        bash: &[1073, 1064, 1072],
        sh: &[1073, 1064, 1072],
        ksh: &[1073, 1064, 1072],
    },
    Snippet {
        construct: "C13",
        label: "then_called",
        code: "function a b { echo hi; }\na\nb\n",
        bash: &[1073, 1064, 1072],
        sh: &[1073, 1064, 1072],
        ksh: &[1073, 1064, 1072],
    },
    Snippet {
        construct: "C14",
        label: "expansion_name",
        code: "V='x y'\nfunction ${=V} { :; }\n",
        bash: &[2034, 2086, 1083, 1089],
        sh: &[2034, 2086, 1083, 1089],
        ksh: &[2034, 2086, 1083, 1089],
    },
    Snippet {
        construct: "C14",
        label: "quoted_param_suffix",
        code: "function foo\"${1:-}\" { :; }\n",
        bash: &[1073, 1064, 1072],
        sh: &[1073, 1064, 1072],
        ksh: &[1073, 1064, 1072],
    },
    Snippet {
        construct: "C14",
        label: "brace_expansion_name",
        code: "function {a,b}_x { :; }\n",
        bash: &[1083, 1089],
        sh: &[3009, 1083, 1089],
        ksh: &[1083, 1089],
    },
    Snippet {
        construct: "C14",
        label: "tilde_name",
        code: "function cfh~ { :; }\n",
        bash: &[1073, 1064, 1072],
        sh: &[1073, 1064, 1072],
        ksh: &[1073, 1064, 1072],
    },
    Snippet {
        construct: "C15",
        label: "then_comment",
        code: "if true; then # c\nfi\n",
        bash: &[1009, 1073, 1048, 1072],
        sh: &[1009, 1073, 1048, 1072],
        ksh: &[1009, 1073, 1048, 1072],
    },
    Snippet {
        construct: "C15",
        label: "else_comment",
        code: "if false; then :; else # nothing\nfi\n",
        bash: &[1009, 1073, 1048, 1072],
        sh: &[1009, 1073, 1048, 1072],
        ksh: &[1009, 1073, 1048, 1072],
    },
    Snippet {
        construct: "C15",
        label: "elif_comment_body",
        code: "if false; then :; elif true; then\n  # later\nfi\n",
        bash: &[1009, 1073, 1048, 1072],
        sh: &[1009, 1073, 1048, 1072],
        ksh: &[1009, 1073, 1048, 1072],
    },
    Snippet {
        construct: "C16",
        label: "if_arith_brace",
        code: "if (( 1 )) { :; }\n",
        bash: &[1073, 1050, 1072],
        sh: &[1073, 1050, 1072],
        ksh: &[1073, 1050, 1072],
    },
    Snippet {
        construct: "C16",
        label: "for_in_brace",
        code: "for i in 1 2; { echo $i }\n",
        bash: &[1009, 1073, 1083, 1056, 1072],
        sh: &[1009, 1073, 1083, 1056, 1072],
        ksh: &[1009, 1073, 1083, 1056, 1072],
    },
    Snippet {
        construct: "C16",
        label: "if_elif_else_brace",
        code: "if [[ -n $x ]] { echo a } elif [[ -z $x ]] { echo b } else { echo c }\n",
        bash: &[1073, 1050, 1140, 1083, 1072],
        sh: &[1073, 1050, 1140, 1083, 1072],
        ksh: &[1073, 1050, 1140, 1083, 1072],
    },
    Snippet {
        construct: "C17",
        label: "I_flag",
        code: "echo $a[(I)x]\n",
        bash: &[1087, 2154, 2086, 1036, 1088],
        sh: &[1087, 2154, 2086, 1036, 1088],
        ksh: &[1087, 2154, 2086, 1036, 1088],
    },
    Snippet {
        construct: "C17",
        label: "Ie_flag_braced",
        code: "echo ${a[(Ie)$w]}\n",
        bash: &[2154, 2086],
        sh: &[2154, 3054, 2086],
        ksh: &[2154, 2086],
    },
    Snippet {
        construct: "C17",
        label: "split_flag",
        code: "echo ${s[(s: :w)2]}\n",
        bash: &[2154, 2086],
        sh: &[2154, 3054, 2086],
        ksh: &[2154, 2086],
    },
    Snippet {
        construct: "C17",
        label: "in_condition",
        code: "if [[ -n ${a[(r)x]} ]]; then echo found; fi\n",
        bash: &[],
        sh: &[3010, 3054],
        ksh: &[],
    },
    Snippet {
        construct: "C18",
        label: "hex",
        code: "echo $(( [##16] 255 ))\n",
        bash: &[1073, 1009, 1072],
        sh: &[1073, 1009, 1072],
        ksh: &[1073, 1009, 1072],
    },
    Snippet {
        construct: "C18",
        label: "single_hash",
        code: "echo $(( [#16] 255 ))\n",
        bash: &[1073, 1009, 1072],
        sh: &[1073, 1009, 1072],
        ksh: &[1073, 1009, 1072],
    },
    Snippet {
        construct: "C18",
        label: "base36",
        code: "echo $(( [##36] 12345 ))\n",
        bash: &[1073, 1009, 1072],
        sh: &[1073, 1009, 1072],
        ksh: &[1073, 1009, 1072],
    },
    Snippet {
        construct: "C19",
        label: "minimal",
        code: "echo a >>| f\n",
        bash: &[1009, 1073, 1072],
        sh: &[1009, 1073, 1072],
        ksh: &[1009, 1073, 1072],
    },
    Snippet {
        construct: "C19",
        label: "group_redirect",
        code: "{ echo a; echo b; } >>| log\n",
        bash: &[1073, 1072],
        sh: &[1073, 1072],
        ksh: &[1073, 1072],
    },
    Snippet {
        construct: "C19",
        label: "in_function",
        code: "f() { print -r -- \"$1\" >>| \"$2\" }\n",
        bash: &[1009, 1073, 1072],
        sh: &[1009, 1073, 1072],
        ksh: &[1009, 1073, 1072],
    },
    Snippet {
        construct: "C20",
        label: "close_1",
        code: "exec {1}<&-\n",
        bash: &[1083, 1083],
        sh: &[1083, 1083],
        ksh: &[1083, 1083],
    },
    Snippet {
        construct: "C20",
        label: "in_function_close_2",
        code: "f() { exec {2}>&- }\n",
        bash: &[1009, 1073, 1083, 1083, 1083, 1056, 1072],
        sh: &[1009, 1073, 1083, 1083, 1083, 1056, 1072],
        ksh: &[1009, 1073, 1083, 1083, 1083, 1056, 1072],
    },
    Snippet {
        construct: "C20",
        label: "named_fd_still_fine",
        code: "exec {fd}<&-\n",
        bash: &[2154],
        sh: &[2154, 3022],
        ksh: &[2154],
    },
    Snippet {
        construct: "C21",
        label: "match_replace",
        code: "echo ${(@)${:-{$#p..1}}/(#m)*/${p[$MATCH]}}\n",
        bash: &[2086, 2296, 1036, 1088],
        sh: &[2086, 2296, 1036, 1088],
        ksh: &[2086, 2296, 1036, 1088],
    },
    Snippet {
        construct: "C21",
        label: "simple_nested",
        code: "echo ${(L)${(U)x}}\n",
        bash: &[2086, 2296, 2296],
        sh: &[2086, 2296, 2296],
        ksh: &[2086, 2296, 2296],
    },
    Snippet {
        construct: "C21",
        label: "quoted_nested_flag",
        code: "echo \"${(j:,:)${(@s: :)x}}\"\n",
        bash: &[2296, 2296],
        sh: &[2296, 2296],
        ksh: &[2296, 2296],
    },
    Snippet {
        construct: "C22",
        label: "cat",
        code: "cat <(() { :; })\n",
        bash: &[1009, 1073, 1072],
        sh: &[1009, 1073, 1072],
        ksh: &[1009, 1073, 1072],
    },
    Snippet {
        construct: "C22",
        label: "diff_with_body",
        code: "diff <(() { echo a }) <(echo b)\n",
        bash: &[1009, 1073, 1072],
        sh: &[1009, 1073, 1072],
        ksh: &[1009, 1073, 1072],
    },
    Snippet {
        construct: "C23",
        label: "compare",
        code: "(( f() == 1 ))\n",
        bash: &[1073, 1009, 1072],
        sh: &[1073, 1009, 1072],
        ksh: &[1073, 1009, 1072],
    },
    Snippet {
        construct: "C23",
        label: "in_if_condition",
        code: "if (( f() > 2 )); then :; fi\n",
        bash: &[1073, 1009, 1072],
        sh: &[1073, 1009, 1072],
        ksh: &[1073, 1009, 1072],
    },
    Snippet {
        construct: "C23",
        label: "with_args",
        code: "(( g(1, 2) == 3 ))\n",
        bash: &[1073, 1009, 1072],
        sh: &[1073, 1009, 1072],
        ksh: &[1073, 1009, 1072],
    },
    Snippet {
        construct: "C24",
        label: "percent_braces",
        code: "PROMPT=%{%F{blue}%}x%{%f%}\n",
        bash: &[2034, 1083, 1083, 1083, 1083, 1083, 1083],
        sh: &[2034, 1083, 1083, 1083, 1083, 1083, 1083],
        ksh: &[2034, 1083, 1083, 1083, 1083, 1083, 1083],
    },
    Snippet {
        construct: "C24",
        label: "color_only",
        code: "print -P %F{red}hi%f\n",
        bash: &[1083, 1083],
        sh: &[1083, 1083],
        ksh: &[1083, 1083],
    },
    Snippet {
        construct: "C24",
        label: "at_brace_upstream",
        code: "git log @{u}..\n",
        bash: &[1083, 1083],
        sh: &[1083, 1083],
        ksh: &[1083, 1083],
    },
    Snippet {
        construct: "C24",
        label: "rprompt_single_quoted_style",
        code: "RPROMPT='%F{green}%~%f'\n",
        bash: &[2034],
        sh: &[2034],
        ksh: &[2034],
    },
    Snippet {
        construct: "C25",
        label: "args_amp_pipe",
        code: "() { :; } \"$1\" &|\n",
        bash: &[1073, 1072],
        sh: &[1073, 1072],
        ksh: &[1073, 1072],
    },
    Snippet {
        construct: "C25",
        label: "args_amp_bang",
        code: "() { echo $1 } a &!\n",
        bash: &[1073, 1072],
        sh: &[1073, 1072],
        ksh: &[1073, 1072],
    },
    Snippet {
        construct: "C25",
        label: "in_function",
        code: "f() {\n  () { echo \"$1\"; sleep 1 } \"$2\" &|\n}\n",
        bash: &[1009, 1073, 1072],
        sh: &[1009, 1073, 1072],
        ksh: &[1009, 1073, 1072],
    },
    Snippet {
        construct: "C26",
        label: "case_insens",
        code: "[[ a == (#i)A ]]\n",
        bash: &[1073, 1027, 1036, 1072],
        sh: &[1073, 1027, 1036, 1072],
        ksh: &[1073, 1027, 1036, 1072],
    },
    Snippet {
        construct: "C26",
        label: "match_flag",
        code: "[[ $x == (#m)a* ]]\n",
        bash: &[1073, 1027, 1036, 1072],
        sh: &[1073, 1027, 1036, 1072],
        ksh: &[1073, 1027, 1036, 1072],
    },
    Snippet {
        construct: "C26",
        label: "backref_then_match",
        code: "[[ $x == (#b)(a)* ]] && echo $match[1]\n",
        bash: &[1073, 1027, 1036, 1072],
        sh: &[1073, 1027, 1036, 1072],
        ksh: &[1073, 1027, 1036, 1072],
    },
    Snippet {
        construct: "C26",
        label: "qualifier_flag",
        code: "[[ $x == (#q)(a) ]]\n",
        bash: &[1073, 1027, 1036, 1072],
        sh: &[1073, 1027, 1036, 1072],
        ksh: &[1073, 1027, 1036, 1072],
    },
    Snippet {
        construct: "C27",
        label: "I_in_test",
        code: "[[ $arr[(I)x] -gt 0 ]]\n",
        bash: &[1073, 1036, 1072],
        sh: &[1073, 1036, 1072],
        ksh: &[1073, 1036, 1072],
    },
    Snippet {
        construct: "C27",
        label: "r_in_n_test",
        code: "[[ -n $arr[(r)x] ]]\n",
        bash: &[1073, 1036, 1072],
        sh: &[1073, 1036, 1072],
        ksh: &[1073, 1036, 1072],
    },
    Snippet {
        construct: "C27",
        label: "braced_Ie",
        code: "[[ ${arr[(Ie)$w]} -ne 0 ]]\n",
        bash: &[2154],
        sh: &[3010, 2154, 3054],
        ksh: &[2154],
    },
    Snippet {
        construct: "C28",
        label: "one_line",
        code: "if [[ -n $x ]] echo hi\n",
        bash: &[1073, 1050, 1140, 1072],
        sh: &[1073, 1050, 1140, 1072],
        ksh: &[1073, 1050, 1140, 1072],
    },
    Snippet {
        construct: "C28",
        label: "then_more_commands",
        code: "if [[ -n $x ]] echo hi; echo there\n",
        bash: &[1073, 1050, 1140, 1072],
        sh: &[1073, 1050, 1140, 1072],
        ksh: &[1073, 1050, 1140, 1072],
    },
    Snippet {
        construct: "C29",
        label: "lowercase_slice",
        code: "echo ${(L)\"${$(( [##36] 12345 ))}\":1:5}\n",
        bash: &[1073, 1009, 1072],
        sh: &[1073, 1009, 1072],
        ksh: &[1073, 1009, 1072],
    },
];

pub static NEAR_MISSES: &[NearMiss] = &[
    NearMiss { construct: "C01", code: "() {\n" },
    NearMiss { construct: "C01", code: "() { echo hi; \n" },
    NearMiss { construct: "C02", code: "sleep 1 &| &|\n" },
    NearMiss { construct: "C02", code: "&|\n" },
    NearMiss { construct: "C03", code: "diff =(echo a\n" },
    NearMiss { construct: "C04", code: "[[ $x == (a|b* ]]\n" },
    NearMiss { construct: "C05", code: "[[ $x == <-> \n" },
    NearMiss { construct: "C06", code: "files=(*(N\n" },
    NearMiss { construct: "C07", code: "case $x in (a|b :;; esac\n" },
    NearMiss { construct: "C08", code: "for f (a b\n" },
    NearMiss { construct: "C09", code: "for k v in a b; do\n" },
    NearMiss { construct: "C10", code: "{ :; } always\n" },
    NearMiss { construct: "C11", code: "f() { echo hi\n" },
    NearMiss { construct: "C12", code: "f() {\n" },
    NearMiss { construct: "C13", code: "function a b {\n" },
    NearMiss { construct: "C14", code: "function foo\"${1:-}\" { echo hi\n" },
    NearMiss { construct: "C15", code: "if true; then\n" },
    NearMiss { construct: "C16", code: "if [[ -n $x ]] { echo a\n" },
    NearMiss { construct: "C17", code: "echo $a[(I]\n" },
    NearMiss { construct: "C18", code: "echo $(( [##16] 255 \n" },
    NearMiss { construct: "C19", code: "echo a >>|\n" },
    NearMiss { construct: "C20", code: "exec {1}<&\n" },
    NearMiss { construct: "C21", code: "echo ${(L)${(U)x}\n" },
    NearMiss { construct: "C22", code: "cat <(() { :; }\n" },
    NearMiss { construct: "C23", code: "(( f( == 1 ))\n" },
    NearMiss { construct: "C25", code: "() { :; } \"$1\" &| &|\n" },
    NearMiss { construct: "C26", code: "[[ a == (#i A ]]\n" },
    NearMiss { construct: "C27", code: "[[ $arr[(I)x -gt 0 \n" },
    NearMiss { construct: "C28", code: "if [[ -n $x ]] echo hi; fi\n" },
    NearMiss { construct: "C29", code: "echo ${(L)\"${$(( [##36] 12345 ))}\":1:5\n" },
];

fn snippet(construct: &str, label: &str) -> &'static Snippet {
    SNIPPETS
        .iter()
        .find(|s| s.construct == construct && s.label == label)
        .unwrap_or_else(|| panic!("no snippet {construct}/{label}"))
}

fn parses(construct: &str, label: &str) {
    let s = snippet(construct, label);
    let got = zsh(s.code);
    let bad: Vec<(u32, u32)> = pairs(&got).into_iter().filter(|(c, _)| (1000..2000).contains(c)).collect();
    assert!(bad.is_empty(), "{construct}/{label}: SC1xxx (code, line) {bad:?} under zsh for:\n{}", s.code);
}

fn bash_unchanged(construct: &str) {
    for s in SNIPPETS.iter().filter(|s| s.construct == construct) {
        assert_eq!(
            codes(&lint(&["--shell=bash"], s.code)),
            s.bash,
            "bash output changed for {}/{}:\n{}",
            s.construct,
            s.label,
            s.code
        );
        assert_eq!(
            codes(&lint(&["--shell=sh"], s.code)),
            s.sh,
            "sh output changed for {}/{}:\n{}",
            s.construct,
            s.label,
            s.code
        );
        assert_eq!(
            codes(&lint(&["--shell=ksh"], s.code)),
            s.ksh,
            "ksh output changed for {}/{}:\n{}",
            s.construct,
            s.label,
            s.code
        );
    }
}

fn near_miss_reported(construct: &str) {
    let mut n = 0;
    for m in NEAR_MISSES.iter().filter(|m| m.construct == construct) {
        n += 1;
        let got = zsh(m.code);
        let parse_codes: Vec<u32> = codes(&got)
            .into_iter()
            .filter(|c| {
                // C28's near-miss is a stray `fi`, which zsh -n reports as a parse error and
                // shelllint reports as SC1089 ("Parsing stopped here").
                let ignored: &[u32] = if construct == "C28" { &[1083, 1087, 1090, 1091] } else { &[1083, 1087, 1089, 1090, 1091] };
                (1000..2000).contains(c) && !ignored.contains(c)
            })
            .collect();
        assert!(
            !parse_codes.is_empty(),
            "{construct}: zsh rejects this but shelllint reports no parse error (codes {:?}):\n{}",
            codes(&got),
            m.code
        );
    }
    assert!(n > 0, "{construct} has no near-miss");
}

/// The snippets are what we say they are, according to the real zsh: positives parse,
/// near-misses do not. Skipped when no zsh is installed.
#[test]
fn snippets_are_what_zsh_says() {
    if !have_zsh() {
        eprintln!("zsh not installed: skipping");
        return;
    }
    for s in SNIPPETS {
        assert!(zsh_n_ok(s.code), "zsh -n rejects {}/{}:\n{}", s.construct, s.label, s.code);
    }
    for m in NEAR_MISSES {
        assert!(!zsh_n_ok(m.code), "zsh -n accepts the near-miss for {}:\n{}", m.construct, m.code);
    }
}

#[test]
fn every_construct_c01_to_c29_has_three_snippets_and_a_near_miss() {
    for i in 1..=29 {
        let id = format!("C{i:02}");
        let n = SNIPPETS.iter().filter(|s| s.construct == id).count();
        let want = match id.as_str() {
            "C22" => 2,
            "C29" => 1,
            "C28" => 2,
            _ => 3,
        };
        assert!(n >= want, "{id} has {n} snippets, want at least {want}");
    }
}

case!(now, c01_anonymous_function__minimal_parses_under_zsh, { parses("C01", "minimal") });
case!(now, c01_anonymous_function__in_function_with_args_parses_under_zsh, { parses("C01", "in_function_with_args") });
case!(now, c01_anonymous_function__function_keyword_parses_under_zsh, { parses("C01", "function_keyword") });
case!(now, c01_anonymous_function__piped_with_args_parses_under_zsh, { parses("C01", "piped_with_args") });
case!(now, c01_anonymous_function__bash_sh_ksh_output_unchanged, { bash_unchanged("C01") });
case!(now, c01_anonymous_function__near_miss_is_still_reported, { near_miss_reported("C01") });

case!(now, c02_disown_amp_pipe_bang__amp_pipe_parses_under_zsh, { parses("C02", "amp_pipe") });
case!(now, c02_disown_amp_pipe_bang__amp_bang_parses_under_zsh, { parses("C02", "amp_bang") });
case!(now, c02_disown_amp_pipe_bang__group_in_function_parses_under_zsh, { parses("C02", "group_in_function") });
case!(now, c02_disown_amp_pipe_bang__subshell_parses_under_zsh, { parses("C02", "subshell") });
case!(now, c02_disown_amp_pipe_bang__bash_sh_ksh_output_unchanged, { bash_unchanged("C02") });
case!(now, c02_disown_amp_pipe_bang__near_miss_is_still_reported, { near_miss_reported("C02") });

case!(now, c03_temp_file_subst__diff_two_parses_under_zsh, { parses("C03", "diff_two") });
case!(now, c03_temp_file_subst__in_function_parses_under_zsh, { parses("C03", "in_function") });
case!(now, c03_temp_file_subst__mixed_with_process_subst_parses_under_zsh, {
    parses("C03", "mixed_with_process_subst")
});
case!(now, c03_temp_file_subst__as_redirect_source_parses_under_zsh, { parses("C03", "as_redirect_source") });
case!(now, c03_temp_file_subst__bash_sh_ksh_output_unchanged, { bash_unchanged("C03") });
case!(now, c03_temp_file_subst__near_miss_is_still_reported, { near_miss_reported("C03") });

case!(now, c04_paren_pattern_groups__alternation_parses_under_zsh, { parses("C04", "alternation") });
case!(now, c04_paren_pattern_groups__backref_flag_group_parses_under_zsh, { parses("C04", "backref_flag_group") });
case!(now, c04_paren_pattern_groups__repeat_hash_hash_parses_under_zsh, { parses("C04", "repeat_hash_hash") });
case!(now, c04_paren_pattern_groups__tilde_param_in_pattern_parses_under_zsh, {
    parses("C04", "tilde_param_in_pattern")
});
case!(now, c04_paren_pattern_groups__bash_sh_ksh_output_unchanged, { bash_unchanged("C04") });
case!(now, c04_paren_pattern_groups__near_miss_is_still_reported, { near_miss_reported("C04") });

case!(now, c05_numeric_ranges__any_number_parses_under_zsh, { parses("C05", "any_number") });
case!(now, c05_numeric_ranges__bounded_parses_under_zsh, { parses("C05", "bounded") });
case!(now, c05_numeric_ranges__case_label_parses_under_zsh, { parses("C05", "case_label") });
case!(now, c05_numeric_ranges__in_function_with_and_parses_under_zsh, { parses("C05", "in_function_with_and") });
case!(now, c05_numeric_ranges__bash_sh_ksh_output_unchanged, { bash_unchanged("C05") });
case!(now, c05_numeric_ranges__near_miss_is_still_reported, { near_miss_reported("C05") });

case!(now, c06_glob_qualifiers__nullglob_qualifier_parses_under_zsh, { parses("C06", "nullglob_qualifier") });
case!(now, c06_glob_qualifiers__recursive_dot_tail_parses_under_zsh, { parses("C06", "recursive_dot_tail") });
case!(now, c06_glob_qualifiers__loop_mtime_parses_under_zsh, { parses("C06", "loop_mtime") });
case!(now, c06_glob_qualifiers__array_assign_head_parses_under_zsh, { parses("C06", "array_assign_head") });
case!(now, c06_glob_qualifiers__bash_sh_ksh_output_unchanged, { bash_unchanged("C06") });
case!(now, c06_glob_qualifiers__near_miss_is_still_reported, { near_miss_reported("C06") });

case!(now, c07_case_pattern_groups__group_label_parses_under_zsh, { parses("C07", "group_label") });
case!(now, c07_case_pattern_groups__number_and_group_parses_under_zsh, { parses("C07", "number_and_group") });
case!(now, c07_case_pattern_groups__multiline_in_function_parses_under_zsh, {
    parses("C07", "multiline_in_function")
});
case!(now, c07_case_pattern_groups__bash_sh_ksh_output_unchanged, { bash_unchanged("C07") });
case!(now, c07_case_pattern_groups__near_miss_is_still_reported, { near_miss_reported("C07") });

case!(now, c08_for_short_form__paren_list_parses_under_zsh, { parses("C08", "paren_list") });
case!(now, c08_for_short_form__brace_body_parses_under_zsh, { parses("C08", "brace_body") });
case!(now, c08_for_short_form__body_on_next_line_parses_under_zsh, { parses("C08", "body_on_next_line") });
case!(now, c08_for_short_form__in_with_semicolon_cmd_parses_under_zsh, { parses("C08", "in_with_semicolon_cmd") });
case!(now, c08_for_short_form__bash_sh_ksh_output_unchanged, { bash_unchanged("C08") });
case!(now, c08_for_short_form__near_miss_is_still_reported, { near_miss_reported("C08") });

case!(now, c09_multi_var_for__minimal_parses_under_zsh, { parses("C09", "minimal") });
case!(now, c09_multi_var_for__in_function_parses_under_zsh, { parses("C09", "in_function") });
case!(now, c09_multi_var_for__assoc_kv_parses_under_zsh, { parses("C09", "assoc_kv") });
case!(now, c09_multi_var_for__bash_sh_ksh_output_unchanged, { bash_unchanged("C09") });
case!(now, c09_multi_var_for__near_miss_is_still_reported, { near_miss_reported("C09") });

case!(now, c10_always_block__minimal_parses_under_zsh, { parses("C10", "minimal") });
case!(now, c10_always_block__cleanup_no_semicolon_parses_under_zsh, { parses("C10", "cleanup_no_semicolon") });
case!(now, c10_always_block__in_function_parses_under_zsh, { parses("C10", "in_function") });
case!(now, c10_always_block__bash_sh_ksh_output_unchanged, { bash_unchanged("C10") });
case!(now, c10_always_block__near_miss_is_still_reported, { near_miss_reported("C10") });

case!(now, c11_close_brace_without_separator__function_body_parses_under_zsh, { parses("C11", "function_body") });
case!(now, c11_close_brace_without_separator__plain_group_parses_under_zsh, { parses("C11", "plain_group") });
case!(now, c11_close_brace_without_separator__two_commands_parses_under_zsh, { parses("C11", "two_commands") });
case!(now, c11_close_brace_without_separator__redirect_last_parses_under_zsh, { parses("C11", "redirect_last") });
case!(now, c11_close_brace_without_separator__bash_sh_ksh_output_unchanged, { bash_unchanged("C11") });
case!(now, c11_close_brace_without_separator__near_miss_is_still_reported, { near_miss_reported("C11") });

case!(now, c12_empty_bodies__function_spaced_parses_under_zsh, { parses("C12", "function_spaced") });
case!(now, c12_empty_bodies__function_tight_parses_under_zsh, { parses("C12", "function_tight") });
case!(now, c12_empty_bodies__anonymous_parses_under_zsh, { parses("C12", "anonymous") });
case!(now, c12_empty_bodies__function_keyword_parses_under_zsh, { parses("C12", "function_keyword") });
case!(now, c12_empty_bodies__bash_sh_ksh_output_unchanged, { bash_unchanged("C12") });
case!(now, c12_empty_bodies__near_miss_is_still_reported, { near_miss_reported("C12") });

case!(now, c13_multi_name_function__two_names_parses_under_zsh, { parses("C13", "two_names") });
case!(now, c13_multi_name_function__three_names_no_semi_parses_under_zsh, { parses("C13", "three_names_no_semi") });
case!(now, c13_multi_name_function__then_called_parses_under_zsh, { parses("C13", "then_called") });
case!(now, c13_multi_name_function__bash_sh_ksh_output_unchanged, { bash_unchanged("C13") });
case!(now, c13_multi_name_function__near_miss_is_still_reported, { near_miss_reported("C13") });

case!(now, c14_odd_function_names__expansion_name_parses_under_zsh, { parses("C14", "expansion_name") });
case!(now, c14_odd_function_names__quoted_param_suffix_parses_under_zsh, { parses("C14", "quoted_param_suffix") });
case!(now, c14_odd_function_names__brace_expansion_name_parses_under_zsh, { parses("C14", "brace_expansion_name") });
case!(now, c14_odd_function_names__tilde_name_parses_under_zsh, { parses("C14", "tilde_name") });
case!(now, c14_odd_function_names__bash_sh_ksh_output_unchanged, { bash_unchanged("C14") });
case!(now, c14_odd_function_names__near_miss_is_still_reported, { near_miss_reported("C14") });

case!(now, c15_empty_then_else_comment_only__then_comment_parses_under_zsh, { parses("C15", "then_comment") });
case!(now, c15_empty_then_else_comment_only__else_comment_parses_under_zsh, { parses("C15", "else_comment") });
case!(now, c15_empty_then_else_comment_only__elif_comment_body_parses_under_zsh, {
    parses("C15", "elif_comment_body")
});
case!(now, c15_empty_then_else_comment_only__bash_sh_ksh_output_unchanged, { bash_unchanged("C15") });
case!(now, c15_empty_then_else_comment_only__near_miss_is_still_reported, { near_miss_reported("C15") });

case!(now, c16_brace_form_control_flow__if_arith_brace_parses_under_zsh, { parses("C16", "if_arith_brace") });
case!(now, c16_brace_form_control_flow__for_in_brace_parses_under_zsh, { parses("C16", "for_in_brace") });
case!(now, c16_brace_form_control_flow__if_elif_else_brace_parses_under_zsh, { parses("C16", "if_elif_else_brace") });
case!(now, c16_brace_form_control_flow__bash_sh_ksh_output_unchanged, { bash_unchanged("C16") });
case!(now, c16_brace_form_control_flow__near_miss_is_still_reported, { near_miss_reported("C16") });

case!(now, c17_subscript_flags__I_flag_parses_under_zsh, { parses("C17", "I_flag") });
case!(now, c17_subscript_flags__Ie_flag_braced_parses_under_zsh, { parses("C17", "Ie_flag_braced") });
case!(now, c17_subscript_flags__split_flag_parses_under_zsh, { parses("C17", "split_flag") });
case!(now, c17_subscript_flags__in_condition_parses_under_zsh, { parses("C17", "in_condition") });
case!(now, c17_subscript_flags__bash_sh_ksh_output_unchanged, { bash_unchanged("C17") });
case!(now, c17_subscript_flags__near_miss_is_still_reported, { near_miss_reported("C17") });

case!(now, c18_arith_base_prefix__hex_parses_under_zsh, { parses("C18", "hex") });
case!(now, c18_arith_base_prefix__single_hash_parses_under_zsh, { parses("C18", "single_hash") });
case!(now, c18_arith_base_prefix__base36_parses_under_zsh, { parses("C18", "base36") });
case!(now, c18_arith_base_prefix__bash_sh_ksh_output_unchanged, { bash_unchanged("C18") });
case!(now, c18_arith_base_prefix__near_miss_is_still_reported, { near_miss_reported("C18") });

case!(now, c19_clobber_append__minimal_parses_under_zsh, { parses("C19", "minimal") });
case!(now, c19_clobber_append__group_redirect_parses_under_zsh, { parses("C19", "group_redirect") });
case!(now, c19_clobber_append__in_function_parses_under_zsh, { parses("C19", "in_function") });
case!(now, c19_clobber_append__bash_sh_ksh_output_unchanged, { bash_unchanged("C19") });
case!(now, c19_clobber_append__near_miss_is_still_reported, { near_miss_reported("C19") });

case!(now, c20_numeric_fd_redirect__close_1_parses_under_zsh, { parses("C20", "close_1") });
case!(now, c20_numeric_fd_redirect__in_function_close_2_parses_under_zsh, { parses("C20", "in_function_close_2") });
case!(now, c20_numeric_fd_redirect__named_fd_still_fine_parses_under_zsh, { parses("C20", "named_fd_still_fine") });
case!(now, c20_numeric_fd_redirect__bash_sh_ksh_output_unchanged, { bash_unchanged("C20") });
case!(now, c20_numeric_fd_redirect__near_miss_is_still_reported, { near_miss_reported("C20") });

case!(now, c21_nested_param_expansion__match_replace_parses_under_zsh, { parses("C21", "match_replace") });
case!(now, c21_nested_param_expansion__simple_nested_parses_under_zsh, { parses("C21", "simple_nested") });
case!(now, c21_nested_param_expansion__quoted_nested_flag_parses_under_zsh, { parses("C21", "quoted_nested_flag") });
case!(now, c21_nested_param_expansion__bash_sh_ksh_output_unchanged, { bash_unchanged("C21") });
case!(now, c21_nested_param_expansion__near_miss_is_still_reported, { near_miss_reported("C21") });

case!(now, c22_anon_fn_in_process_subst__cat_parses_under_zsh, { parses("C22", "cat") });
case!(now, c22_anon_fn_in_process_subst__diff_with_body_parses_under_zsh, { parses("C22", "diff_with_body") });
case!(now, c22_anon_fn_in_process_subst__bash_sh_ksh_output_unchanged, { bash_unchanged("C22") });
case!(now, c22_anon_fn_in_process_subst__near_miss_is_still_reported, { near_miss_reported("C22") });

case!(now, c23_math_function_call__compare_parses_under_zsh, { parses("C23", "compare") });
case!(now, c23_math_function_call__in_if_condition_parses_under_zsh, { parses("C23", "in_if_condition") });
case!(now, c23_math_function_call__with_args_parses_under_zsh, { parses("C23", "with_args") });
case!(now, c23_math_function_call__bash_sh_ksh_output_unchanged, { bash_unchanged("C23") });
case!(now, c23_math_function_call__near_miss_is_still_reported, { near_miss_reported("C23") });

case!(now, c24_prompt_braces_no_sc1083__percent_braces_parses_under_zsh, { parses("C24", "percent_braces") });
case!(now, c24_prompt_braces_no_sc1083__color_only_parses_under_zsh, { parses("C24", "color_only") });
case!(now, c24_prompt_braces_no_sc1083__at_brace_upstream_parses_under_zsh, { parses("C24", "at_brace_upstream") });
case!(now, c24_prompt_braces_no_sc1083__rprompt_single_quoted_style_parses_under_zsh, {
    parses("C24", "rprompt_single_quoted_style")
});
case!(now, c24_prompt_braces_no_sc1083__bash_sh_ksh_output_unchanged, { bash_unchanged("C24") });

case!(now, c25_anon_fn_args_disown__args_amp_pipe_parses_under_zsh, { parses("C25", "args_amp_pipe") });
case!(now, c25_anon_fn_args_disown__args_amp_bang_parses_under_zsh, { parses("C25", "args_amp_bang") });
case!(now, c25_anon_fn_args_disown__in_function_parses_under_zsh, { parses("C25", "in_function") });
case!(now, c25_anon_fn_args_disown__bash_sh_ksh_output_unchanged, { bash_unchanged("C25") });
case!(now, c25_anon_fn_args_disown__near_miss_is_still_reported, { near_miss_reported("C25") });

case!(now, c26_glob_flags__case_insens_parses_under_zsh, { parses("C26", "case_insens") });
case!(now, c26_glob_flags__match_flag_parses_under_zsh, { parses("C26", "match_flag") });
case!(now, c26_glob_flags__backref_then_match_parses_under_zsh, { parses("C26", "backref_then_match") });
case!(now, c26_glob_flags__qualifier_flag_parses_under_zsh, { parses("C26", "qualifier_flag") });
case!(now, c26_glob_flags__bash_sh_ksh_output_unchanged, { bash_unchanged("C26") });
case!(now, c26_glob_flags__near_miss_is_still_reported, { near_miss_reported("C26") });

case!(now, c27_subscript_in_double_bracket__I_in_test_parses_under_zsh, { parses("C27", "I_in_test") });
case!(now, c27_subscript_in_double_bracket__r_in_n_test_parses_under_zsh, { parses("C27", "r_in_n_test") });
case!(now, c27_subscript_in_double_bracket__braced_Ie_parses_under_zsh, { parses("C27", "braced_Ie") });
case!(now, c27_subscript_in_double_bracket__bash_sh_ksh_output_unchanged, { bash_unchanged("C27") });
case!(now, c27_subscript_in_double_bracket__near_miss_is_still_reported, { near_miss_reported("C27") });

case!(now, c28_short_if__one_line_parses_under_zsh, { parses("C28", "one_line") });
case!(now, c28_short_if__then_more_commands_parses_under_zsh, { parses("C28", "then_more_commands") });
case!(now, c28_short_if__bash_sh_ksh_output_unchanged, { bash_unchanged("C28") });
case!(now, c28_short_if__near_miss_is_still_reported, { near_miss_reported("C28") });

case!(now, c29_nested_flags_base_slice__lowercase_slice_parses_under_zsh, { parses("C29", "lowercase_slice") });
case!(now, c29_nested_flags_base_slice__bash_sh_ksh_output_unchanged, { bash_unchanged("C29") });
case!(now, c29_nested_flags_base_slice__near_miss_is_still_reported, { near_miss_reported("C29") });
