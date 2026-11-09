// SPDX-License-Identifier: GPL-3.0-or-later
// Regression test for the exponential blow-up when scripts source each other
// (koalaman/shellcheck#3442, #2954). See the DIVERGENCE in
// crates/parser/src/area_commands.rs (`read_source`).

use std::time::{Duration, Instant};

use shelllint_checker::check_script;
use shelllint_core::interface::{CheckSpec, empty_check_spec, mocked_system_interface};

/// `n` scripts, each sourcing all the others.
fn mutual_scripts(n: usize) -> Vec<(String, String)> {
    (1..=n)
        .map(|i| {
            let mut body = String::from("#!/bin/bash\n");
            for j in (1..=n).filter(|j| *j != i) {
                body.push_str(&format!("# shellcheck source=s{j}.sh\n. s{j}.sh\n"));
            }
            body.push_str("echo $x\n");
            (format!("s{i}.sh"), body)
        })
        .collect()
}

#[test]
fn mutually_sourcing_scripts_finish_quickly() {
    let files = mutual_scripts(8);
    let main = files[0].1.clone();
    let sys = mocked_system_interface(files);
    let spec = CheckSpec {
        filename: "s1.sh".to_string(),
        script: main,
        check_sourced: true,
        ..empty_check_spec()
    };
    let start = Instant::now();
    let result = check_script(&sys, &spec);
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(60), "took {elapsed:?}");
    assert!(
        result.comments.iter().any(|c| c.start_pos.file == "s1.sh" && c.comment.code == 2086),
        "expected SC2086 in the main script"
    );
}
