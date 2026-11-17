// SPDX-License-Identifier: GPL-3.0-or-later
// `--bash-version` tests for the ${..} and brace rows (SC9107 9108 9117 9125 9134 9135); no Haskell counterpart.

mod bash_version_common;
use bash_version_common::*;

#[test]
fn sc9107_case_modification() {
    for s in ["echo \"${v,}\"\n", "echo \"${v,,}\"\n", "echo \"${v^}\"\n", "echo \"${v^^}\"\n"] {
        assert_pair(9107, "4.0", s);
    }
    for s in ["echo \"${a[@]^^}\"\n"] {
        assert_pair(9107, "4.0", s);
    }
    assert_pair(9107, "4.0", "echo \"${v^^[aeiou]}\"\n");
}

#[test]
fn sc9107_guards() {
    for s in [
        "echo \"${v:-,x}\"\n",
        "echo \"${v:-^^}\"\n",
        "echo \"${v/,/x}\"\n",
        "echo \"${v#,}\"\n",
        "echo \"${v%^}\"\n",
        "echo \"${#v}\" \"$@\"\n",
        "echo \"${v}\"\n",
    ] {
        assert_silent("3.0", s);
    }
}

#[test]
fn sc9108_zero_padded_brace() {
    assert_pair(9108, "4.0", "echo {01..10}\n");
    assert_pair(9108, "4.0", "echo {1..010}\n");
    assert_pair(9108, "4.0", "echo file{001..100}.txt\n");
    assert_pair(9108, "4.0", "echo {-01..5}\n");
    assert_pair(9108, "4.0", "echo {01..10..2}\n");
}

#[test]
fn sc9108_guards() {
    for s in [
        "echo {1..10}\n",
        "echo {0..9}\n",
        "echo {1..9..2}\n",
        "echo {a..e}\n",
        "echo {01,02}\n",
        "echo {a,b}01..10\n",
        "echo 01..10\n",
    ] {
        assert_silent("3.0", s);
    }
}

#[test]
fn sc9117_negative_index_and_length() {
    assert_pair(9117, "4.2", "echo \"${a[-1]}\"\n");
    assert_pair(9117, "4.2", "echo \"${a[ -2 ]}\"\n");
    assert_pair(9117, "4.2", "echo \"${#a[-1]}\"\n");
    assert_pair(9117, "4.2", "echo \"${v:0:-1}\"\n");
    assert_pair(9117, "4.2", "echo \"${v: 1: -2}\"\n");
    assert_pair(9117, "4.2", "echo \"${v:a?1:2:-1}\"\n");
}

#[test]
fn sc9117_guards() {
    for s in [
        "echo \"${a[1]}\"\n",
        "echo \"${a[$i-1]}\"\n",
        "echo \"${a[i-1]}\"\n",
        "echo \"${a[@]}\" \"${a[*]}\"\n",
        "echo \"${v:1:2}\"\n",
        "echo \"${v: -1}\"\n",
        "echo \"${v: -2:3}\"\n",
        "echo \"${v:-1}\"\n",
        "echo \"${v:-1:2}\"\n",
        "echo \"${v:=-1}\"\n",
        "echo \"${v:0:$((n-1))}\"\n",
        "echo \"${v:a?1:2:3}\"\n",
        "echo \"${v/-1/x}\"\n",
    ] {
        assert_silent("3.0", s);
    }
}

#[test]
fn sc9125_transformations() {
    for op in ["Q", "E", "P", "A", "a"] {
        assert_pair(9125, "4.4", &format!("echo \"${{v@{op}}}\"\n"));
    }
    assert_pair(9125, "4.4", "echo \"${a[@]@Q}\"\n");
    assert_pair(9125, "4.4", "echo \"${@@Q}\"\n");
}

#[test]
fn sc9125_guards() {
    for s in [
        "echo \"${v@}\"\n",
        "echo \"$@\" \"${@}\"\n",
        "echo \"${!p@}\"\n",
        "echo \"${v:-@Q}\"\n",
        "echo \"${v/@Q/x}\"\n",
    ] {
        assert_silent("3.0", s);
    }
}

#[test]
fn sc9134_case_and_key_transformations() {
    for op in ["U", "u", "L", "K"] {
        assert_pair(9134, "5.1", &format!("echo \"${{v@{op}}}\"\n"));
    }
}

#[test]
fn sc9134_guards() {
    // the 4.4 operators belong to SC9125, the 5.2 one to SC9135
    assert_silent("4.4", "echo \"${v@Q}\"\n");
    assert_eq!(bash("5.1", "echo \"${v@k}\"\n"), [9135]);
    assert_silent("3.0", "echo \"${v:-@U}\" \"${v@X}\"\n");
}

#[test]
fn sc9135_lowercase_k() {
    assert_pair(9135, "5.2", "echo \"${v@k}\"\n");
    assert_silent("5.1", "echo \"${v@K}\"\n");
}

#[test]
fn sc9135_guards() {
    assert_silent("3.0", "echo \"${k}\" \"${v@}\" \"${v:-@k}\"\n");
}

#[test]
fn sh_and_unset_target_stay_quiet() {
    use shelllint_core::interface::Shell;
    let s = "echo \"${v^^}\" {01..10} \"${a[-1]}\" \"${v@Q}\"\n";
    assert!(codes(Shell::Sh, Some("3.2"), s).is_empty());
    assert!(codes(Shell::Bash, None, s).is_empty());
}

#[test]
fn message_names_feature_and_target() {
    assert_eq!(messages("3.2", "echo \"${v^^}\"\n"), ["case modification in ${..} requires bash 4.0 (target is 3.2)."]);
}

#[test]
fn sc9117_skips_associative_arrays() {
    // `-1` is a string key on an associative array; only `declare -A` itself is reported
    let s = "declare -A m\necho \"${m[-1]}\" \"${#m[-1]}\"\n";
    assert_eq!(bash("3.0", s), vec![9102]);
    assert!(!bash("4.1", "declare -a m\necho \"${m[-1]}\"\n").is_empty());
}
