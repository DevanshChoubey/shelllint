// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: `--list-checks[=text|json]` is an extension; upstream only has
// `--list-optional`.
//
// What it lists: the optional checks, exactly the data `--list-optional`
// shows (name, codes, description, example, fix). It does NOT list every SC
// code with a severity: no such catalogue exists in the source. Each check
// picks its severity at its emit site (`err`/`warn`/`info`/`style` calls
// scattered over crates/checks and the parser), so a complete table would
// have to be written by hand, which this does not do.

use shelllint_checker::optional_checks;
use shelllint_formatter::tty::WIKI_LINK;

use crate::{Status, print_err};

const NOTE: &str = "Only the optional checks are listed. A complete table of SC codes and \
severities is not available: severity is chosen where each check emits its \
comment, and no catalogue of all codes exists in the source.";

pub fn print(format: &str) -> Result<(), Status> {
    let mut list = optional_checks();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    match format {
        "text" => {
            for c in &list {
                let ids: Vec<String> = c.id.iter().map(|n| format!("SC{}", n)).collect();
                println!("{}  {}  {}", c.name, ids.join(","), c.description);
            }
            println!();
            println!("{}", NOTE);
        }
        "json" => {
            let items: Vec<String> = list
                .iter()
                .map(|c| {
                    let codes: Vec<String> = c.id.iter().map(|n| n.to_string()).collect();
                    let wikis: Vec<String> = c.id.iter().map(|n| json_str(&format!("{}SC{}", WIKI_LINK, n))).collect();
                    format!(
                        "{{\"name\":{},\"codes\":[{}],\"wiki\":[{}],\"description\":{},\"example\":{},\"fix\":{}}}",
                        json_str(&c.name),
                        codes.join(","),
                        wikis.join(","),
                        json_str(&c.description),
                        json_str(&c.positive),
                        json_str(&c.negative)
                    )
                })
                .collect();
            println!("{{\"optional_checks\":[{}],\"all_codes\":null,\"note\":{}}}", items.join(","), json_str(NOTE));
        }
        _ => {
            print_err("Unknown value for --list-checks. Valid options are: text, json");
            return Err(Status::SupportFailure);
        }
    }
    Ok(())
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
