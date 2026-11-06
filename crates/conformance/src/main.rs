// SPDX-License-Identifier: GPL-3.0-only
// Conformance harness for the port of ShellCheck (e11f964); no Haskell counterpart

use shellcheck_conformance::runner::{
    Filter, FullParityResult, diff_lines, group_mismatches, run_all, run_parity, run_parity_full,
};
use shellcheck_conformance::{Outcome, default_corpus_dir, load_corpus};
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "usage: shellcheck-conformance report [--module M] [--kind K] [--target T] [--json]\n       \
         shellcheck-conformance parity [--update-baseline] [--verbose]\n       \
         shellcheck-conformance parity --full [--update-baseline] [--verbose] [--markdown FILE]\n\n\
         parity compares deduplicated code sets against oracle_codes (baseline\n\
         corpus/parity-baseline.json); --full compares the exact ordered json1\n\
         comment list against corpus/oracle-full.jsonl (baseline\n\
         corpus/parity-full-baseline.json)."
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
            let (mut update, mut verbose, mut full, mut markdown) = (false, false, false, None);
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--update-baseline" => update = true,
                    "--verbose" => verbose = true,
                    "--full" => full = true,
                    "--codes" => full = false,
                    "--markdown" => match it.next() {
                        Some(p) => markdown = Some(p.clone()),
                        None => return usage(),
                    },
                    _ => return usage(),
                }
            }
            if markdown.is_some() && !full {
                return usage();
            }
            if full {
                return parity_full(&recs, &corpus_dir, update, verbose, markdown.as_deref());
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
            if verbose {
                for (k, g, o) in &res.mismatches {
                    println!("MISMATCH {k} ours={g:?} oracle={o:?}");
                }
            }
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

fn parity_full(
    recs: &[shellcheck_conformance::Record],
    corpus_dir: &std::path::Path,
    update: bool,
    verbose: bool,
    markdown: Option<&str>,
) -> ExitCode {
    let oracle = match shellcheck_conformance::load_oracle_full(&corpus_dir.join("oracle-full.jsonl")) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let res = run_parity_full(recs, &oracle);
    if !res.available {
        println!("parity --full: checker feature disabled; nothing to compare ({} candidate scripts)", res.total);
        return ExitCode::SUCCESS;
    }
    println!(
        "parity --full: {}/{} match oracle json1 exactly ({:.1}%)",
        res.matched,
        res.total,
        100.0 * res.matched as f64 / res.total.max(1) as f64
    );
    for k in &res.missing_oracle {
        println!("NO-ORACLE {k}");
    }
    for k in &res.crashed {
        println!("CRASH {k}");
    }
    let groups = group_mismatches(&res.mismatches);
    for ((code, kind), keys) in &groups {
        println!("  SC{code} {}: {} script(s)", kind.name(), keys.len());
    }
    if verbose {
        for m in &res.mismatches {
            let kinds: Vec<String> = m.kinds.iter().map(|(c, k)| format!("SC{c}:{}", k.name())).collect();
            println!("MISMATCH {} [{}]", m.key, kinds.join(", "));
            for l in diff_lines(&m.ours, &m.oracle) {
                println!("    {l}");
            }
        }
    }
    if let Some(p) = markdown
        && let Err(e) = std::fs::write(p, markdown_report(&res))
    {
        eprintln!("{p}: {e}");
        return ExitCode::from(2);
    }
    let path = corpus_dir.join("parity-full-baseline.json");
    if update {
        let s = serde_json::to_string_pretty(&res.matches).unwrap();
        if let Err(e) = std::fs::write(&path, s + "\n") {
            eprintln!("{}: {e}", path.display());
            return ExitCode::from(2);
        }
        println!("full baseline updated ({} entries)", res.matches.len());
        return ExitCode::SUCCESS;
    }
    let baseline = match shellcheck_conformance::load_baseline(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let regressed: Vec<_> =
        baseline.iter().filter(|(k, v)| **v && !res.matches.contains_key(*k)).map(|(k, _)| k).collect();
    for k in &regressed {
        println!("REGRESSION {k}");
    }
    if regressed.is_empty() && res.crashed.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn markdown_report(res: &FullParityResult) -> String {
    let mut s = String::from("# Full-fidelity parity mismatches\n\n");
    s.push_str(&format!(
        "Generated by `shellcheck-conformance parity --full --markdown`. Each script's \
         ordered json1 comment list (without `file`) is compared with corpus/oracle-full.jsonl.\n\n\
         Score: {}/{} scripts match exactly ({:.1}%).\n\n",
        res.matched,
        res.total,
        100.0 * res.matched as f64 / res.total.max(1) as f64
    ));
    s.push_str("## By code and kind of difference\n\n| code | kind | scripts |\n|---|---|---|\n");
    for ((code, kind), keys) in group_mismatches(&res.mismatches) {
        s.push_str(&format!("| SC{code} | {} | {} |\n", kind.name(), keys.len()));
    }
    for ((code, kind), keys) in group_mismatches(&res.mismatches) {
        s.push_str(&format!("\n### SC{code} {}\n\n", kind.name()));
        for k in keys {
            s.push_str(&format!("- `{k}`\n"));
        }
    }
    s.push_str("\n## Diffs\n\n`-` oracle only, `+` ours only. Format: line:col-endLine:endCol level code message [fix].\n");
    for m in &res.mismatches {
        s.push_str(&format!("\n### `{}`\n\n```diff\n", m.key));
        for l in diff_lines(&m.ours, &m.oracle) {
            s.push_str(&l);
            s.push('\n');
        }
        s.push_str("```\n");
    }
    s
}
