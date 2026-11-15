// SPDX-License-Identifier: GPL-3.0-or-later
// A variable declared with -n (a nameref) counts as used, and so does a literal target.

use shelllint_checker::check_script_codes;
use shelllint_core::interface::Shell;

fn has_2034(script: &str) -> bool {
    check_script_codes(script, Some(Shell::Bash)).contains(&2034)
}

#[test]
fn nameref_assignment_not_unused() {
    let s = "#!/bin/bash\nread_bool() {\n  local -n out=\"$1\"\n  out=1\n}\nread_bool x\n";
    assert!(!has_2034(s));
}

#[test]
fn nameref_literal_target_used() {
    let s = "#!/bin/bash\narray=(1 2 3)\ndeclare -n ref=\"array\"\nref+=(4)\necho \"${ref[@]}\"\n";
    assert!(!has_2034(s));
    let s = "#!/bin/bash\narray=(1 2 3)\ndeclare -n ref=array\nref+=(4)\n";
    assert!(!has_2034(s));
}

#[test]
fn nameref_multiple_names() {
    let s = "#!/bin/bash\nf() {\n  local -n o2=\"${1}\" str=\"${2}\"\n  str=\"${o2[*]}\"\n}\nf a b\n";
    assert!(!has_2034(s));
}

#[test]
fn nameref_combined_flags() {
    assert!(!has_2034("#!/bin/bash\nf() { declare -rn r=\"$1\"; }\nf x\n"));
}

#[test]
fn dynamic_target_does_not_mark_others() {
    let s = "#!/bin/bash\nunused=1\nf() { local -n r=\"$1\"; r=2; }\nf x\n";
    assert!(has_2034(s));
}

#[test]
fn plain_local_unchanged() {
    assert!(has_2034("#!/bin/bash\nf() { local v=1; }\nf\n"));
}
