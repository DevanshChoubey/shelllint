// SPDX-License-Identifier: GPL-3.0-only
// Conformance harness for the port of ShellCheck (e11f964); no Haskell counterpart

use shellcheck_conformance::runner::{Filter, run_all, run_parity};
use shellcheck_conformance::{Outcome, default_corpus_dir, load_corpus};
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "usage: shellcheck-conformance report [--module M] [--kind K] [--target T] [--json]\n       \
         shellcheck-conformance parity [--update-baseline]"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else {
        return usage();
    };
    let corpus_dir = default_corpus_dir();
    let recs = match load_corpus(&corpus_dir.join("tests.jsonl")) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match cmd {
        "report" | "run" => {
            let mut f = Filter::default();
            let mut json = false;
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--module" => f.module = it.next().cloned(),
                    "--kind" => f.kind = it.next().cloned(),
                    "--target" => f.target = it.next().cloned(),
                    "--json" => json = true,
                    _ => return usage(),
                }
            }
            let (c, results) = run_all(&recs, &f);
            if json {
                let failures: Vec<_> = results
                    .iter()
                    .filter_map(|(r, o)| match o {
                        Outcome::Fail(m) => Some(serde_json::json!({"key": r.key(), "message": m})),
                        _ => None,
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({
                        "pass": c.pass, "fail": c.fail, "blocked": c.blocked,
                        "unported": c.unported, "skipped": c.skipped, "failures": failures,
                    })
                );
            } else {
                for (r, o) in &results {
                    if let Outcome::Fail(m) = o {
                        println!("FAIL {}: {m}", r.key());
                    }
                }
                println!(
                    "pass {} / fail {} / blocked {} / unported {}",
                    c.pass, c.fail, c.blocked, c.unported
                );
                println!("(skipped, not run: {} unit/unparsed records)", c.skipped);
            }
            if c.fail > 0 { ExitCode::from(1) } else { ExitCode::SUCCESS }
        }
        "parity" => {
            let update = args[1..].iter().any(|a| a == "--update-baseline");
            if args[1..].iter().any(|a| a != "--update-baseline") {
                return usage();
            }
            let res = run_parity(&recs);
            if !res.available {
                println!("parity: checker feature disabled; nothing to compare ({} candidate scripts)", res.total);
                return ExitCode::SUCCESS;
            }
            println!(
                "parity: {}/{} match oracle ({:.1}%)",
                res.matched,
                res.total,
                100.0 * res.matched as f64 / res.total.max(1) as f64
            );
            let path = corpus_dir.join("parity-baseline.json");
            if update {
                let s = serde_json::to_string_pretty(&res.matches).unwrap();
                if let Err(e) = std::fs::write(&path, s + "\n") {
                    eprintln!("{}: {e}", path.display());
                    return ExitCode::from(2);
                }
                println!("baseline updated ({} entries)", res.matches.len());
                return ExitCode::SUCCESS;
            }
            let baseline = match shellcheck_conformance::load_baseline(&path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            let regressed: Vec<_> = baseline
                .iter()
                .filter(|(k, v)| **v && !res.matches.contains_key(*k))
                .map(|(k, _)| k)
                .collect();
            for k in &regressed {
                println!("REGRESSION {k}");
            }
            if regressed.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        _ => usage(),
    }
}
