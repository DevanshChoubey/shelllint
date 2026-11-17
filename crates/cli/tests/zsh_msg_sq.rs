// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for SC2016 under zsh (prompt variables, `zstyle -e`, `bindkey -s`).
#[path = "zsh_msg_common.rs"]
mod zsh_msg_common;
use zsh_msg_common::*;

const MSG: &str = "Expressions don't expand in single quotes, use double quotes for that.";

#[test]
fn sc2016_zsh_prompt_without_promptsubst_fires_but_bash_ps1_does_not() {
    // evidence: without promptsubst the single-quoted `$x` stays literal in zsh
    assert_eq!(zsh_run("x=hi; PS1='$x> '; print -P $PS1"), "$x>");
    let z = zsh("PS1='$x> '\n");
    assert_eq!(lane(&z), vec![2016]);
    assert_eq!(message_of(&z, 2016), MSG);
    assert_eq!(lane(&zsh("PROMPT='$(git_prompt)'\nRPROMPT='$x'\n")), vec![2016, 2016]);
    // bash: PS1 is always fine, unchanged
    assert_eq!(lane(&bash("PS1='$x> '\n")), Vec::<u32>::new());
}

#[test]
fn sc2016_zsh_prompt_is_fine_with_promptsubst_on() {
    assert_eq!(zsh_run("x=hi; setopt promptsubst; PS1='$x> '; print -P $PS1"), "hi>");
    for var in ["PS1", "PS2", "PS3", "PS4", "PROMPT", "PROMPT2", "RPROMPT", "RPS1", "SPROMPT"] {
        let s = format!("setopt promptsubst\n{var}='$x> '\n");
        assert_eq!(lane(&zsh(&s)), Vec::<u32>::new(), "{var}");
    }
    assert_eq!(lane(&zsh("setopt promptsubst\nPROMPT='$(git_prompt)'\n")), Vec::<u32>::new());
}

#[test]
fn sc2016_zsh_prompt_fires_again_after_nopromptsubst() {
    assert_eq!(lane(&zsh("setopt promptsubst\nunsetopt promptsubst\nPS1='$x'\n")), vec![2016]);
    assert_eq!(lane(&zsh("setopt promptsubst nopromptsubst\nPS1='$x'\n")), vec![2016]);
}

#[test]
fn sc2016_zsh_other_single_quoted_assignments_still_fire() {
    assert_eq!(lane(&zsh("FOO='$x'\n")), vec![2016]);
    assert_eq!(lane(&zsh("echo '$x'\n")), vec![2016]);
}

#[test]
fn sc2016_zsh_zstyle_e_code_argument_is_fine_but_plain_zstyle_is_not() {
    // evidence: `zstyle -e` keeps the code string; plain zstyle stores `$HOME` literally
    assert!(
        zsh_run("zstyle -e ':completion:*' special-dirs 'reply=($PWD)'; zstyle -L ':completion:*'")
            .contains("reply=($PWD)")
    );
    assert_eq!(zsh_run("zstyle ':x' fmt '$HOME'; zstyle -g r ':x' fmt; print -r -- $r"), "$HOME");
    assert_eq!(lane(&zsh("zstyle -e ':completion:*' special-dirs 'reply=($PWD)'\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("zstyle -e ':completion:*:hosts' hosts 'reply=(${(f)\"$(cat $f)\"})'\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("zstyle ':x' fmt '$HOME'\n")), vec![2016]);
    // bash: zstyle is not special
    assert_eq!(lane(&bash("zstyle -e ':completion:*' special-dirs 'reply=($PWD)'\n")), vec![2016]);
}

#[test]
fn sc2016_zsh_bindkey_s_macro_is_fine() {
    // evidence: the macro is stored literally and typed into the line later
    assert_eq!(zsh_run("bindkey -s '^o' 'cd $(pwd)\\n'; bindkey '^o'"), "\"^O\" \"cd \\$(pwd)^J\"");
    assert_eq!(lane(&zsh("bindkey -s '^o' 'cd $(pwd)\\n'\n")), Vec::<u32>::new());
    assert_eq!(lane(&bash("bindkey -s '^o' 'cd $(pwd)\\n'\n")), vec![2016]);
}

#[test]
fn sc2016_zsh_bindkey_without_s_flag_is_not_exempt() {
    assert_eq!(lane(&zsh("bindkey '^o' '$x'\n")), vec![2016]);
}

#[test]
fn sc2016_zsh_existing_exemptions_still_apply() {
    assert_eq!(lane(&zsh("trap 'echo $x' EXIT\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("eval 'echo $x'\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("zsh -c 'echo $x'\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("alias a='echo $x'\n")), Vec::<u32>::new());
}
