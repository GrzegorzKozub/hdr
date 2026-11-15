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

const ON: anstyle::Style = anstyle::AnsiColor::Green.on_default().bold();
const OFF: anstyle::Style = anstyle::Style::new().bold();
const ERR: anstyle::Style = anstyle::AnsiColor::Red.on_default().bold();

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
    let only = selector.map(|s| select_display(&displays, s)).transpose()?;

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
            format!("Display {index} ({}): ", display.name)
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
            println!("{label}HDR not supported");
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

/// "HDR on" in green or "HDR off" in bold.
fn state(on: bool) -> String {
    if on {
        format!("{ON}HDR on{ON:#}")
    } else {
        format!("{OFF}HDR off{OFF:#}")
    }
}

fn policy(info: &HdrInfo) -> &'static str {
    if info.limited_by_policy {
        " (limited by system policy)"
    } else {
        ""
    }
}

/// Resolve a `--display` value to a 1-based index: a number is taken as an
/// index, anything else must match exactly one monitor name.
fn select_display(displays: &[Display], selector: &str) -> Result<usize, String> {
    if let Ok(n) = selector.parse::<usize>() {
        if n == 0 || n > displays.len() {
            return Err(format!(
                "display {n} does not exist (found {})",
                displays.len()
            ));
        }
        return Ok(n);
    }
    let needle = selector.to_lowercase();
    let matches: Vec<usize> = (0..displays.len())
        .filter(|&i| displays[i].name.to_lowercase().contains(&needle))
        .collect();
    let list = |idx: &mut dyn Iterator<Item = usize>| {
        idx.map(|i| format!("{} ({})", i + 1, displays[i].name))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match matches[..] {
        [i] => Ok(i + 1),
        [] => Err(format!(
            "no display matches '{selector}' (available: {})",
            list(&mut (0..displays.len()))
        )),
        _ => Err(format!(
            "'{selector}' matches several displays: {}; use a more specific name or an index",
            list(&mut matches.iter().copied())
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(
            select_display(&displays(), "0")
                .unwrap_err()
                .contains("does not exist")
        );
        assert!(
            select_display(&displays(), "4")
                .unwrap_err()
                .contains("does not exist")
        );
    }

    #[test]
    fn selects_by_case_insensitive_name_part() {
        assert_eq!(select_display(&displays(), "oled"), Ok(1));
        assert_eq!(select_display(&displays(), "u2720"), Ok(2));
    }

    #[test]
    fn rejects_ambiguous_name() {
        let err = select_display(&displays(), "dell").unwrap_err();
        assert!(err.contains("matches several displays"), "{err}");
        assert!(err.contains("2 (DELL U2720Q), 3 (DELL P2419H)"), "{err}");
    }

    #[test]
    fn rejects_unknown_name_listing_all() {
        let err = select_display(&displays(), "lg").unwrap_err();
        assert!(
            err.contains("available: 1 (MPG321UX OLED), 2 (DELL U2720Q), 3 (DELL P2419H)"),
            "{err}"
        );
    }
}
