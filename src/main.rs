//! hdr — enable, disable, toggle, or report Windows HDR state for every
//! active display, from the command line.
//!
//! Display enumeration and the Windows API calls live in `display.rs`.
//!
//! Usage:
//!   hdr status          # print HDR state per display
//!   hdr on              # enable HDR on every display that supports it
//!   hdr off             # disable HDR on every display
//!   hdr toggle          # flip each display's current state
//!   hdr on --display 1  # act on a single display (1-based, from `status`)
//!   hdr on -d "HDR 4K"  # ...or by case-insensitive monitor-name substring

mod display;

use std::process::ExitCode;

use display::{enumerate_displays, get_hdr_info, set_hdr_state, Display};

#[derive(Clone, Copy, PartialEq)]
enum Command {
    Status,
    On,
    Off,
    Toggle,
}

const USAGE: &str = "usage: hdr [status|on|off|toggle] [--display N|NAME]";

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args() -> Result<(Command, Option<String>), String> {
    let mut command = None;
    let mut only = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--display" | "-d" => {
                let v = args
                    .next()
                    .filter(|v| !v.is_empty())
                    .ok_or("--display needs a value")?;
                only = Some(v);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            _ if command.is_some() => return Err(format!("unexpected argument '{arg}'\n{USAGE}")),
            "status" => command = Some(Command::Status),
            "on" => command = Some(Command::On),
            "off" => command = Some(Command::Off),
            "toggle" => command = Some(Command::Toggle),
            _ => return Err(format!("unknown command '{arg}'\n{USAGE}")),
        }
    }
    Ok((command.unwrap_or(Command::Status), only))
}

/// Returns Ok(false) if any per-display operation failed.
fn run() -> Result<bool, String> {
    let (command, selector) = parse_args()?;
    let displays = enumerate_displays()?;
    if displays.is_empty() {
        return Err("no active displays found".into());
    }
    let only = selector
        .map(|s| select_display(&displays, &s))
        .transpose()?;

    let mut ok = true;
    for (i, display) in displays.iter().enumerate() {
        let index = i + 1;
        if only.is_some_and(|n| n != index) {
            continue;
        }
        let label = format!("display {index} ({})", display.name);

        let info = match get_hdr_info(display) {
            Ok(info) => info,
            Err(e) => {
                eprintln!("{label}: {e}");
                ok = false;
                continue;
            }
        };

        if command == Command::Status {
            println!(
                "{label}: supported={} enabled={}{}",
                info.supported,
                info.enabled,
                if info.limited_by_policy {
                    " (limited by system policy)"
                } else {
                    ""
                }
            );
            continue;
        }

        if !info.supported {
            println!("{label}: HDR not supported, skipping");
            continue;
        }
        let want = match command {
            Command::On => true,
            Command::Off => false,
            _ => !info.enabled,
        };
        if want == info.enabled {
            println!("{label}: already {}", if want { "on" } else { "off" });
            continue;
        }
        match set_hdr_state(display, &info, want) {
            Ok(()) => println!("{label}: HDR {}", if want { "enabled" } else { "disabled" }),
            Err(e) => {
                eprintln!("{label}: {e}");
                ok = false;
            }
        }
    }
    Ok(ok)
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
