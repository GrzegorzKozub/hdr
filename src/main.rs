//! hdr: turn Windows HDR on or off, or show its state, for every active
//! display or just one.
//!
//! The command line lives in `cli.rs`; display enumeration and the Windows
//! API calls live in `display.rs`.

mod cli;
mod display;

use std::process::ExitCode;

use anstream::{eprintln, println};
use clap::Parser;

use cli::{Cli, Command};
use display::{Display, HdrInfo, enumerate_displays, get_hdr_info, set_hdr_state};

const ON: anstyle::Style = anstyle::AnsiColor::Yellow.on_default();
const OFF: anstyle::Style = anstyle::AnsiColor::White.on_default();
const ERR: anstyle::Style = anstyle::AnsiColor::Red.on_default();
const NAME: anstyle::Style = anstyle::AnsiColor::Magenta.on_default();

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(
        cli.command.unwrap_or(Command::Status),
        cli.display.as_deref(),
    ) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{ERR}error:{ERR:#} {e}");
            ExitCode::FAILURE
        }
    }
}

/// Returns Ok(false) if any per-display operation failed.
fn run(command: Command, selector: Option<&str>) -> Result<bool, String> {
    let displays = enumerate_displays()?;
    if displays.is_empty() {
        return Err("no active displays found".into());
    }
    let only = match selector.map(|s| (s, select_display(&displays, s))) {
        None => None,
        Some((_, Ok(n))) => Some(n),
        Some((s, Err(tip))) => cli::invalid_display(s, &tip).exit(),
    };

    let mut ok = true;
    for (i, display) in displays.iter().enumerate() {
        let index = i + 1;
        if only.is_some_and(|n| n != index) {
            continue;
        }
        // With a single display there is nothing to tell apart.
        let label = if displays.len() == 1 {
            String::new()
        } else {
            format!(
                "display {NAME}{index}{NAME:#}, {NAME}{}{NAME:#}, ",
                display.name
            )
        };

        let info = match get_hdr_info(display) {
            Ok(info) => info,
            Err(e) => {
                eprintln!("{ERR}error:{ERR:#} {label}{e}");
                ok = false;
                continue;
            }
        };

        if !info.supported {
            println!("{label}hdr not supported");
            continue;
        }
        if command == Command::Status {
            println!("{label}{}{}", state(info.enabled), policy(&info));
            continue;
        }

        let want = match command {
            Command::On => true,
            Command::Off => false,
            _ => !info.enabled,
        };
        if want == info.enabled {
            println!("{label}{}", state(want));
            continue;
        }
        match set_hdr_state(display, &info, want) {
            Ok(()) => println!("{label}{}", state(want)),
            Err(e) => {
                eprintln!("{ERR}error:{ERR:#} {label}{e}");
                ok = false;
            }
        }
    }
    Ok(ok)
}

/// "hdr on" in yellow or "hdr off" in gray.
fn state(on: bool) -> String {
    if on {
        format!("{ON}hdr on{ON:#}")
    } else {
        format!("{OFF}hdr off{OFF:#}")
    }
}

fn policy(info: &HdrInfo) -> &'static str {
    if info.limited_by_policy {
        ", limited by system policy"
    } else {
        ""
    }
}

/// Resolve a `--display` value to a 1-based index: a number is taken as an
/// index, anything else must match exactly one monitor name. The error is a
/// tip listing the displays to pick from.
fn select_display(displays: &[Display], selector: &str) -> Result<usize, String> {
    let list = |idx: &mut dyn Iterator<Item = usize>| {
        idx.map(|i| {
            format!(
                "{NAME}{}{NAME:#} or {NAME}{}{NAME:#}",
                i + 1,
                displays[i].name
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
    };
    if let Ok(n) = selector.parse::<usize>() {
        if n == 0 || n > displays.len() {
            return Err(format!(
                "available displays: {}",
                list(&mut (0..displays.len()))
            ));
        }
        return Ok(n);
    }
    let needle = selector.to_lowercase();
    let matches: Vec<usize> = (0..displays.len())
        .filter(|&i| displays[i].name.to_lowercase().contains(&needle))
        .collect();
    match matches[..] {
        [i] => Ok(i + 1),
        [] => Err(format!(
            "available displays: {}",
            list(&mut (0..displays.len()))
        )),
        _ => Err(format!(
            "several displays match: {}",
            list(&mut matches.iter().copied())
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The error text without colors.
    fn err(selector: &str) -> String {
        let e = select_display(&displays(), selector).unwrap_err();
        anstream::adapter::strip_str(&e).to_string()
    }

    fn displays() -> Vec<Display> {
        ["MPG321UX OLED", "DELL U2720Q", "DELL P2419H"]
            .map(Display::named)
            .into()
    }

    #[test]
    fn selects_by_index() {
        assert_eq!(select_display(&displays(), "1"), Ok(1));
        assert_eq!(select_display(&displays(), "3"), Ok(3));
    }

    #[test]
    fn rejects_out_of_range_index() {
        assert!(err("0").starts_with("available displays: 1 or "));
        assert!(err("4").starts_with("available displays: 1 or "));
    }

    #[test]
    fn selects_by_case_insensitive_name_part() {
        assert_eq!(select_display(&displays(), "oled"), Ok(1));
        assert_eq!(select_display(&displays(), "u2720"), Ok(2));
    }

    #[test]
    fn rejects_ambiguous_name() {
        assert_eq!(
            err("dell"),
            "several displays match: 2 or DELL U2720Q, 3 or DELL P2419H"
        );
    }

    #[test]
    fn rejects_unknown_name_listing_all() {
        assert_eq!(
            err("lg"),
            "available displays: 1 or MPG321UX OLED, 2 or DELL U2720Q, 3 or DELL P2419H"
        );
    }
}
