// SPDX-License-Identifier: GPL-3.0-or-later
// Criterion micro-benchmarks (no Haskell counterpart): parse-only versus the
// full parse + analyze pipeline. Inputs: the hand-written scripts in
// bench/handwritten, plus `echo $x` repeated to 1k and 10k lines.
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use shelllint_checker::check_script;
use shelllint_core::interface::{CheckSpec, empty_check_spec, mocked_system_interface, new_parse_spec};
use shelllint_parser::parse_script;
use std::hint::black_box;

fn inputs() -> Vec<(String, String)> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../bench/handwritten");
    let mut v = vec![];
    for n in ["installer.sh", "build.sh", "long_case.sh"] {
        v.push((n.to_string(), std::fs::read_to_string(format!("{dir}/{n}")).unwrap()));
    }
    v.push(("echo_1k".into(), "echo $x\n".repeat(1_000)));
    v.push(("echo_10k".into(), "echo $x\n".repeat(10_000)));
    v
}

fn bench(c: &mut Criterion) {
    let sys = mocked_system_interface(vec![]);
    let inputs = inputs();
    let mut parse = c.benchmark_group("parse_only");
    for (name, script) in &inputs {
        parse.throughput(Throughput::Bytes(script.len() as u64));
        let pspec = shelllint_core::interface::ParseSpec {
            filename: "script.sh".into(),
            script: script.clone(),
            ignore_rc: true,
            ..new_parse_spec()
        };
        parse.bench_function(name, |b| b.iter(|| black_box(parse_script(&sys, black_box(&pspec)))));
    }
    parse.finish();
    let mut full = c.benchmark_group("parse_and_analyze");
    for (name, script) in &inputs {
        full.throughput(Throughput::Bytes(script.len() as u64));
        let cspec = CheckSpec {
            filename: "script.sh".into(),
            script: script.clone(),
            ignore_rc: true,
            optional_checks: vec!["all".into()],
            ..empty_check_spec()
        };
        full.bench_function(name, |b| b.iter(|| black_box(check_script(&sys, black_box(&cspec)))));
    }
    full.finish();
}

criterion_group! { name = benches; config = Criterion::default().sample_size(20); targets = bench }
criterion_main!(benches);
