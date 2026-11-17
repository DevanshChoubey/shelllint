// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only regression tests for non-zsh shells (no ShellCheck
// counterpart). Each input is a near-miss of zsh-only syntax (a fallthrough or
// clobber redirect, a `repeat` loop, a `[[ =~ ]]` operand, a function named like
// the zsh anonymous one, ...). Under sh, bash, dash and ksh it must give exactly
// the json1 output that the pre-zsh tree (v0.4.0, main) gave. The expected text
// is that output, recorded from the main build, byte for byte; a difference means
// a zsh rule leaked into another shell.
//
// The oracle (shellcheck-hs) is not available in CI, so this file cannot compare
// against it directly.

use std::io::Write;
use std::process::{Command, Stdio};

struct Case {
    name: &'static str,
    shell: &'static str,
    script: &'static str,
    expected: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "case_fallthrough_semicolon_pipe",
        shell: "sh",
        script: r##"#!/bin/sh
case "$1" in
  a) echo a ;|
  b) echo b ;;
esac
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"info","code":1009,"message":"The mentioned syntax error was in this case expression.","fix":null},{"file":"-","line":3,"endLine":3,"column":3,"endColumn":3,"level":"error","code":1073,"message":"Couldn't parse this case item. Fix to allow more checks.","fix":null},{"file":"-","line":3,"endLine":3,"column":14,"endColumn":14,"level":"error","code":1072,"message":"Expected a command. Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "case_fallthrough_semicolon_pipe",
        shell: "bash",
        script: r##"#!/bin/bash
case "$1" in
  a) echo a ;|
  b) echo b ;;
esac
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"info","code":1009,"message":"The mentioned syntax error was in this case expression.","fix":null},{"file":"-","line":3,"endLine":3,"column":3,"endColumn":3,"level":"error","code":1073,"message":"Couldn't parse this case item. Fix to allow more checks.","fix":null},{"file":"-","line":3,"endLine":3,"column":14,"endColumn":14,"level":"error","code":1072,"message":"Expected a command. Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "case_fallthrough_semicolon_pipe",
        shell: "dash",
        script: r##"#!/bin/dash
case "$1" in
  a) echo a ;|
  b) echo b ;;
esac
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"info","code":1009,"message":"The mentioned syntax error was in this case expression.","fix":null},{"file":"-","line":3,"endLine":3,"column":3,"endColumn":3,"level":"error","code":1073,"message":"Couldn't parse this case item. Fix to allow more checks.","fix":null},{"file":"-","line":3,"endLine":3,"column":14,"endColumn":14,"level":"error","code":1072,"message":"Expected a command. Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "case_fallthrough_semicolon_pipe",
        shell: "ksh",
        script: r##"#!/bin/ksh
case "$1" in
  a) echo a ;|
  b) echo b ;;
esac
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"info","code":1009,"message":"The mentioned syntax error was in this case expression.","fix":null},{"file":"-","line":3,"endLine":3,"column":3,"endColumn":3,"level":"error","code":1073,"message":"Couldn't parse this case item. Fix to allow more checks.","fix":null},{"file":"-","line":3,"endLine":3,"column":14,"endColumn":14,"level":"error","code":1072,"message":"Expected a command. Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "redirect_clobber_pipe",
        shell: "sh",
        script: r##"#!/bin/sh
echo hi >| out.txt
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "redirect_clobber_pipe",
        shell: "bash",
        script: r##"#!/bin/bash
echo hi >| out.txt
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "redirect_clobber_pipe",
        shell: "dash",
        script: r##"#!/bin/dash
echo hi >| out.txt
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "redirect_clobber_pipe",
        shell: "ksh",
        script: r##"#!/bin/ksh
echo hi >| out.txt
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "redirect_clobber_bang",
        shell: "sh",
        script: r##"#!/bin/sh
echo hi >! out.txt
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "redirect_clobber_bang",
        shell: "bash",
        script: r##"#!/bin/bash
echo hi >! out.txt
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "redirect_clobber_bang",
        shell: "dash",
        script: r##"#!/bin/dash
echo hi >! out.txt
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "redirect_clobber_bang",
        shell: "ksh",
        script: r##"#!/bin/ksh
echo hi >! out.txt
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "background_pipe_amp",
        shell: "sh",
        script: r##"#!/bin/sh
sleep 1 &|
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":10,"endColumn":10,"level":"error","code":1070,"message":"Parsing stopped here. Mismatched keywords or invalid parentheses?","fix":null}]}
"##,
    },
    Case {
        name: "background_pipe_amp",
        shell: "bash",
        script: r##"#!/bin/bash
sleep 1 &|
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":10,"endColumn":10,"level":"error","code":1070,"message":"Parsing stopped here. Mismatched keywords or invalid parentheses?","fix":null}]}
"##,
    },
    Case {
        name: "background_pipe_amp",
        shell: "dash",
        script: r##"#!/bin/dash
sleep 1 &|
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":10,"endColumn":10,"level":"error","code":1070,"message":"Parsing stopped here. Mismatched keywords or invalid parentheses?","fix":null}]}
"##,
    },
    Case {
        name: "background_pipe_amp",
        shell: "ksh",
        script: r##"#!/bin/ksh
sleep 1 &|
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":10,"endColumn":10,"level":"error","code":1070,"message":"Parsing stopped here. Mismatched keywords or invalid parentheses?","fix":null}]}
"##,
    },
    Case {
        name: "process_substitution_equals",
        shell: "sh",
        script: r##"#!/bin/sh
cat =(echo hi)
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":5,"endColumn":6,"level":"error","code":2283,"message":"Remove spaces around = to assign (or use [ ] to compare, or quote '=' if literal).","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1036,"message":"'(' is invalid here. Did you forget to escape it?","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1088,"message":"Parsing stopped here. Invalid use of parentheses?","fix":null}]}
"##,
    },
    Case {
        name: "process_substitution_equals",
        shell: "bash",
        script: r##"#!/bin/bash
cat =(echo hi)
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":5,"endColumn":6,"level":"error","code":2283,"message":"Remove spaces around = to assign (or use [ ] to compare, or quote '=' if literal).","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1036,"message":"'(' is invalid here. Did you forget to escape it?","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1088,"message":"Parsing stopped here. Invalid use of parentheses?","fix":null}]}
"##,
    },
    Case {
        name: "process_substitution_equals",
        shell: "dash",
        script: r##"#!/bin/dash
cat =(echo hi)
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":5,"endColumn":6,"level":"error","code":2283,"message":"Remove spaces around = to assign (or use [ ] to compare, or quote '=' if literal).","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1036,"message":"'(' is invalid here. Did you forget to escape it?","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1088,"message":"Parsing stopped here. Invalid use of parentheses?","fix":null}]}
"##,
    },
    Case {
        name: "process_substitution_equals",
        shell: "ksh",
        script: r##"#!/bin/ksh
cat =(echo hi)
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":5,"endColumn":6,"level":"error","code":2283,"message":"Remove spaces around = to assign (or use [ ] to compare, or quote '=' if literal).","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1036,"message":"'(' is invalid here. Did you forget to escape it?","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1088,"message":"Parsing stopped here. Invalid use of parentheses?","fix":null}]}
"##,
    },
    Case {
        name: "brace_at_word_end",
        shell: "sh",
        script: r##"#!/bin/sh
echo a}}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null},{"file":"-","line":2,"endLine":2,"column":8,"endColumn":8,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null}]}
"##,
    },
    Case {
        name: "brace_at_word_end",
        shell: "bash",
        script: r##"#!/bin/bash
echo a}}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null},{"file":"-","line":2,"endLine":2,"column":8,"endColumn":8,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null}]}
"##,
    },
    Case {
        name: "brace_at_word_end",
        shell: "dash",
        script: r##"#!/bin/dash
echo a}}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null},{"file":"-","line":2,"endLine":2,"column":8,"endColumn":8,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null}]}
"##,
    },
    Case {
        name: "brace_at_word_end",
        shell: "ksh",
        script: r##"#!/bin/ksh
echo a}}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null},{"file":"-","line":2,"endLine":2,"column":8,"endColumn":8,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null}]}
"##,
    },
    Case {
        name: "braced_default_with_braces",
        shell: "sh",
        script: r##"#!/bin/sh
echo ${a:-{b}}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":14,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":14,"endColumn":14,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}},{"file":"-","line":2,"endLine":2,"column":14,"endColumn":14,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null}]}
"##,
    },
    Case {
        name: "braced_default_with_braces",
        shell: "bash",
        script: r##"#!/bin/bash
echo ${a:-{b}}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":14,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":14,"endColumn":14,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}},{"file":"-","line":2,"endLine":2,"column":14,"endColumn":14,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null}]}
"##,
    },
    Case {
        name: "braced_default_with_braces",
        shell: "dash",
        script: r##"#!/bin/dash
echo ${a:-{b}}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":14,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":14,"endColumn":14,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}},{"file":"-","line":2,"endLine":2,"column":14,"endColumn":14,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null}]}
"##,
    },
    Case {
        name: "braced_default_with_braces",
        shell: "ksh",
        script: r##"#!/bin/ksh
echo ${a:-{b}}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":14,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":14,"endColumn":14,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}},{"file":"-","line":2,"endLine":2,"column":14,"endColumn":14,"level":"warning","code":1083,"message":"This } is literal. Check expression (missing ;/\\n?) or quote it.","fix":null}]}
"##,
    },
    Case {
        name: "function_two_names",
        shell: "sh",
        script: r##"#!/bin/sh
function a b { }
a
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this function. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1064,"message":"Expected a { to open the function definition.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1072,"message":" Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "function_two_names",
        shell: "bash",
        script: r##"#!/bin/bash
function a b { }
a
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this function. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1064,"message":"Expected a { to open the function definition.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1072,"message":" Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "function_two_names",
        shell: "dash",
        script: r##"#!/bin/dash
function a b { }
a
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this function. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1064,"message":"Expected a { to open the function definition.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1072,"message":" Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "function_two_names",
        shell: "ksh",
        script: r##"#!/bin/ksh
function a b { }
a
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this function. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1064,"message":"Expected a { to open the function definition.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1072,"message":" Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "repeat_loop",
        shell: "sh",
        script: r##"#!/bin/sh
repeat 2 echo hi
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "repeat_loop",
        shell: "bash",
        script: r##"#!/bin/bash
repeat 2 echo hi
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "repeat_loop",
        shell: "dash",
        script: r##"#!/bin/dash
repeat 2 echo hi
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "repeat_loop",
        shell: "ksh",
        script: r##"#!/bin/ksh
repeat 2 echo hi
"##,
        expected: r##"{"comments":[]}
"##,
    },
    Case {
        name: "case_group_pattern",
        shell: "sh",
        script: r##"#!/bin/sh
case $x in
  (a|b)*) echo y;;
esac
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"info","code":1009,"message":"The mentioned syntax error was in this case expression.","fix":null},{"file":"-","line":3,"endLine":3,"column":3,"endColumn":3,"level":"error","code":1073,"message":"Couldn't parse this case item. Fix to allow more checks.","fix":null},{"file":"-","line":3,"endLine":3,"column":9,"endColumn":9,"level":"error","code":1072,"message":" Fix any mentioned problems and try again.","fix":null},{"file":"-","line":3,"endLine":3,"column":9,"endColumn":9,"level":"error","code":1074,"message":"Did you forget the ;; after the previous case item?","fix":null}]}
"##,
    },
    Case {
        name: "case_group_pattern",
        shell: "bash",
        script: r##"#!/bin/bash
case $x in
  (a|b)*) echo y;;
esac
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"info","code":1009,"message":"The mentioned syntax error was in this case expression.","fix":null},{"file":"-","line":3,"endLine":3,"column":3,"endColumn":3,"level":"error","code":1073,"message":"Couldn't parse this case item. Fix to allow more checks.","fix":null},{"file":"-","line":3,"endLine":3,"column":9,"endColumn":9,"level":"error","code":1072,"message":" Fix any mentioned problems and try again.","fix":null},{"file":"-","line":3,"endLine":3,"column":9,"endColumn":9,"level":"error","code":1074,"message":"Did you forget the ;; after the previous case item?","fix":null}]}
"##,
    },
    Case {
        name: "case_group_pattern",
        shell: "dash",
        script: r##"#!/bin/dash
case $x in
  (a|b)*) echo y;;
esac
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"info","code":1009,"message":"The mentioned syntax error was in this case expression.","fix":null},{"file":"-","line":3,"endLine":3,"column":3,"endColumn":3,"level":"error","code":1073,"message":"Couldn't parse this case item. Fix to allow more checks.","fix":null},{"file":"-","line":3,"endLine":3,"column":9,"endColumn":9,"level":"error","code":1072,"message":" Fix any mentioned problems and try again.","fix":null},{"file":"-","line":3,"endLine":3,"column":9,"endColumn":9,"level":"error","code":1074,"message":"Did you forget the ;; after the previous case item?","fix":null}]}
"##,
    },
    Case {
        name: "case_group_pattern",
        shell: "ksh",
        script: r##"#!/bin/ksh
case $x in
  (a|b)*) echo y;;
esac
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"info","code":1009,"message":"The mentioned syntax error was in this case expression.","fix":null},{"file":"-","line":3,"endLine":3,"column":3,"endColumn":3,"level":"error","code":1073,"message":"Couldn't parse this case item. Fix to allow more checks.","fix":null},{"file":"-","line":3,"endLine":3,"column":9,"endColumn":9,"level":"error","code":1072,"message":" Fix any mentioned problems and try again.","fix":null},{"file":"-","line":3,"endLine":3,"column":9,"endColumn":9,"level":"error","code":1074,"message":"Did you forget the ;; after the previous case item?","fix":null}]}
"##,
    },
    Case {
        name: "bare_subscript",
        shell: "sh",
        script: r##"#!/bin/sh
echo $a[1]
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1087,"message":"Use braces when expanding arrays, e.g. ${array[idx]} (or ${var}[.. to quiet).","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":8,"level":"warning","code":2154,"message":"a is referenced but not assigned.","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":8,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":8,"endColumn":8,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}}]}
"##,
    },
    Case {
        name: "bare_subscript",
        shell: "bash",
        script: r##"#!/bin/bash
echo $a[1]
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1087,"message":"Use braces when expanding arrays, e.g. ${array[idx]} (or ${var}[.. to quiet).","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":8,"level":"warning","code":2154,"message":"a is referenced but not assigned.","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":8,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":8,"endColumn":8,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}}]}
"##,
    },
    Case {
        name: "bare_subscript",
        shell: "dash",
        script: r##"#!/bin/dash
echo $a[1]
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1087,"message":"Use braces when expanding arrays, e.g. ${array[idx]} (or ${var}[.. to quiet).","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":8,"level":"warning","code":2154,"message":"a is referenced but not assigned.","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":8,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":8,"endColumn":8,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}}]}
"##,
    },
    Case {
        name: "bare_subscript",
        shell: "ksh",
        script: r##"#!/bin/ksh
echo $a[1]
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":6,"level":"error","code":1087,"message":"Use braces when expanding arrays, e.g. ${array[idx]} (or ${var}[.. to quiet).","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":8,"level":"warning","code":2154,"message":"a is referenced but not assigned.","fix":null},{"file":"-","line":2,"endLine":2,"column":6,"endColumn":8,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":8,"endColumn":8,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}}]}
"##,
    },
    Case {
        name: "quoted_regex_match",
        shell: "sh",
        script: r##"#!/bin/sh
if [[ x =~ "a.c" ]]; then echo m; fi
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":4,"endColumn":20,"level":"warning","code":3010,"message":"In POSIX sh, [[ ]] is undefined.","fix":null},{"file":"-","line":2,"endLine":2,"column":9,"endColumn":11,"level":"warning","code":2050,"message":"This expression is constant. Did you forget the $ on a variable?","fix":null},{"file":"-","line":2,"endLine":2,"column":9,"endColumn":11,"level":"warning","code":3015,"message":"In POSIX sh, =~ regex matching is undefined.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":17,"level":"warning","code":2076,"message":"Remove quotes from right-hand side of =~ to match as a regex rather than literally.","fix":null}]}
"##,
    },
    Case {
        name: "quoted_regex_match",
        shell: "bash",
        script: r##"#!/bin/bash
if [[ x =~ "a.c" ]]; then echo m; fi
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":9,"endColumn":11,"level":"warning","code":2050,"message":"This expression is constant. Did you forget the $ on a variable?","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":17,"level":"warning","code":2076,"message":"Remove quotes from right-hand side of =~ to match as a regex rather than literally.","fix":null}]}
"##,
    },
    Case {
        name: "quoted_regex_match",
        shell: "dash",
        script: r##"#!/bin/dash
if [[ x =~ "a.c" ]]; then echo m; fi
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":4,"endColumn":20,"level":"error","code":3010,"message":"In dash, [[ ]] is not supported.","fix":null},{"file":"-","line":2,"endLine":2,"column":9,"endColumn":11,"level":"error","code":3015,"message":"In dash, =~ regex matching is not supported.","fix":null},{"file":"-","line":2,"endLine":2,"column":9,"endColumn":11,"level":"warning","code":2050,"message":"This expression is constant. Did you forget the $ on a variable?","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":17,"level":"warning","code":2076,"message":"Remove quotes from right-hand side of =~ to match as a regex rather than literally.","fix":null}]}
"##,
    },
    Case {
        name: "quoted_regex_match",
        shell: "ksh",
        script: r##"#!/bin/ksh
if [[ x =~ "a.c" ]]; then echo m; fi
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":9,"endColumn":11,"level":"warning","code":2050,"message":"This expression is constant. Did you forget the $ on a variable?","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":17,"level":"warning","code":2076,"message":"Remove quotes from right-hand side of =~ to match as a regex rather than literally.","fix":null}]}
"##,
    },
    Case {
        name: "equals_split_flag",
        shell: "sh",
        script: r##"#!/bin/sh
echo ${=x}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":11,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":11,"endColumn":11,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}}]}
"##,
    },
    Case {
        name: "equals_split_flag",
        shell: "bash",
        script: r##"#!/bin/bash
echo ${=x}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":11,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":11,"endColumn":11,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}}]}
"##,
    },
    Case {
        name: "equals_split_flag",
        shell: "dash",
        script: r##"#!/bin/dash
echo ${=x}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":11,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":11,"endColumn":11,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}}]}
"##,
    },
    Case {
        name: "equals_split_flag",
        shell: "ksh",
        script: r##"#!/bin/ksh
echo ${=x}
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":6,"endColumn":11,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting.","fix":{"replacements":[{"column":6,"endColumn":6,"endLine":2,"insertionPoint":"afterEnd","line":2,"precedence":7,"replacement":"\""},{"column":11,"endColumn":11,"endLine":2,"insertionPoint":"beforeStart","line":2,"precedence":7,"replacement":"\""}]}}]}
"##,
    },
    Case {
        name: "for_two_variables",
        shell: "sh",
        script: r##"#!/bin/sh
for k v in a b; do echo $k; done
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this for loop. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"error","code":1058,"message":"Expected 'do'.","fix":null},{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"error","code":1072,"message":"Expected 'do'. Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "for_two_variables",
        shell: "bash",
        script: r##"#!/bin/bash
for k v in a b; do echo $k; done
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this for loop. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"error","code":1058,"message":"Expected 'do'.","fix":null},{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"error","code":1072,"message":"Expected 'do'. Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "for_two_variables",
        shell: "dash",
        script: r##"#!/bin/dash
for k v in a b; do echo $k; done
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this for loop. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"error","code":1058,"message":"Expected 'do'.","fix":null},{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"error","code":1072,"message":"Expected 'do'. Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "for_two_variables",
        shell: "ksh",
        script: r##"#!/bin/ksh
for k v in a b; do echo $k; done
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this for loop. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"error","code":1058,"message":"Expected 'do'.","fix":null},{"file":"-","line":2,"endLine":2,"column":7,"endColumn":7,"level":"error","code":1072,"message":"Expected 'do'. Fix any mentioned problems and try again.","fix":null}]}
"##,
    },
    Case {
        name: "short_if_without_fi",
        shell: "sh",
        script: r##"#!/bin/sh
if [[ c ]] cmd
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this if expression. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1050,"message":"Expected 'then'.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1072,"message":"Expected 'then'. Fix any mentioned problems and try again.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1140,"message":"Unexpected parameters after condition. Missing &&/||, or bad expression?","fix":null}]}
"##,
    },
    Case {
        name: "short_if_without_fi",
        shell: "bash",
        script: r##"#!/bin/bash
if [[ c ]] cmd
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this if expression. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1050,"message":"Expected 'then'.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1072,"message":"Expected 'then'. Fix any mentioned problems and try again.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1140,"message":"Unexpected parameters after condition. Missing &&/||, or bad expression?","fix":null}]}
"##,
    },
    Case {
        name: "short_if_without_fi",
        shell: "dash",
        script: r##"#!/bin/dash
if [[ c ]] cmd
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this if expression. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1050,"message":"Expected 'then'.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1072,"message":"Expected 'then'. Fix any mentioned problems and try again.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1140,"message":"Unexpected parameters after condition. Missing &&/||, or bad expression?","fix":null}]}
"##,
    },
    Case {
        name: "short_if_without_fi",
        shell: "ksh",
        script: r##"#!/bin/ksh
if [[ c ]] cmd
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":1,"level":"error","code":1073,"message":"Couldn't parse this if expression. Fix to allow more checks.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1050,"message":"Expected 'then'.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1072,"message":"Expected 'then'. Fix any mentioned problems and try again.","fix":null},{"file":"-","line":2,"endLine":2,"column":12,"endColumn":12,"level":"error","code":1140,"message":"Unexpected parameters after condition. Missing &&/||, or bad expression?","fix":null}]}
"##,
    },
    Case {
        name: "anon_function_named_like_zsh",
        shell: "sh",
        script: r##"#!/bin/sh
zsh-anonymous-function() { echo "$1"; }
zsh-anonymous-function
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":40,"level":"warning","code":2120,"message":"zsh-anonymous-function references arguments, but none are ever passed.","fix":null},{"file":"-","line":2,"endLine":2,"column":1,"endColumn":40,"level":"warning","code":3033,"message":"In POSIX sh, naming functions outside [a-zA-Z_][a-zA-Z0-9_]* is undefined.","fix":null},{"file":"-","line":3,"endLine":3,"column":1,"endColumn":23,"level":"info","code":2119,"message":"Use zsh-anonymous-function \"$@\" if function's $1 should mean script's $1.","fix":null}]}
"##,
    },
    Case {
        name: "anon_function_named_like_zsh",
        shell: "bash",
        script: r##"#!/bin/bash
zsh-anonymous-function() { echo "$1"; }
zsh-anonymous-function
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":40,"level":"warning","code":2120,"message":"zsh-anonymous-function references arguments, but none are ever passed.","fix":null},{"file":"-","line":3,"endLine":3,"column":1,"endColumn":23,"level":"info","code":2119,"message":"Use zsh-anonymous-function \"$@\" if function's $1 should mean script's $1.","fix":null}]}
"##,
    },
    Case {
        name: "anon_function_named_like_zsh",
        shell: "dash",
        script: r##"#!/bin/dash
zsh-anonymous-function() { echo "$1"; }
zsh-anonymous-function
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":40,"level":"error","code":3033,"message":"In dash, naming functions outside [a-zA-Z_][a-zA-Z0-9_]* is not supported.","fix":null},{"file":"-","line":2,"endLine":2,"column":1,"endColumn":40,"level":"warning","code":2120,"message":"zsh-anonymous-function references arguments, but none are ever passed.","fix":null},{"file":"-","line":3,"endLine":3,"column":1,"endColumn":23,"level":"info","code":2119,"message":"Use zsh-anonymous-function \"$@\" if function's $1 should mean script's $1.","fix":null}]}
"##,
    },
    Case {
        name: "anon_function_named_like_zsh",
        shell: "ksh",
        script: r##"#!/bin/ksh
zsh-anonymous-function() { echo "$1"; }
zsh-anonymous-function
"##,
        expected: r##"{"comments":[{"file":"-","line":2,"endLine":2,"column":1,"endColumn":40,"level":"warning","code":2120,"message":"zsh-anonymous-function references arguments, but none are ever passed.","fix":null},{"file":"-","line":3,"endLine":3,"column":1,"endColumn":23,"level":"info","code":2119,"message":"Use zsh-anonymous-function \"$@\" if function's $1 should mean script's $1.","fix":null}]}
"##,
    },
];

fn json1(shell: &str, script: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "-f", "json1", &format!("--shell={shell}"), "-"])
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    String::from_utf8(o.stdout).unwrap()
}

#[test]
fn near_miss_inputs_match_the_pre_zsh_output_byte_for_byte() {
    let mut failures = Vec::new();
    for c in CASES {
        let got = json1(c.shell, c.script);
        if got != c.expected {
            failures.push(format!("{} under {}:\n  want {:?}\n  got  {:?}", c.name, c.shell, c.expected, got));
        }
    }
    assert!(failures.is_empty(), "{} near-miss output(s) changed:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn every_near_miss_input_is_covered_under_all_four_shells() {
    let names: std::collections::BTreeSet<&str> = CASES.iter().map(|c| c.name).collect();
    assert_eq!(names.len(), 16, "one name per near-miss input");
    for shell in ["sh", "bash", "dash", "ksh"] {
        assert_eq!(CASES.iter().filter(|c| c.shell == shell).count(), names.len(), "{shell} rows");
    }
}
