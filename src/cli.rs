use anstyle::{AnsiColor, Effects, Style};
use clap::builder::{NonEmptyStringValueParser, Styles};
use clap::{Parser, Subcommand};

// Help colors as in cargo, uv and hyperfine, in the normal (not bright) shades.
const HEADER: Style = AnsiColor::Green.on_default().effects(Effects::BOLD);
const LITERAL: Style = AnsiColor::Cyan.on_default().effects(Effects::BOLD);
const PLACEHOLDER: Style = AnsiColor::Cyan.on_default();
const STYLES: Styles = Styles::styled()
    .header(HEADER)
    .usage(HEADER)
    .literal(LITERAL)
    .placeholder(PLACEHOLDER)
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(LITERAL)
    .invalid(AnsiColor::Yellow.on_default().effects(Effects::BOLD));

/// Turn Windows HDR on or off from the command line.
///
/// Changes the same setting as "Use HDR" in Settings > System > Display, on
/// every display that supports HDR or just the one picked with --display.
///
/// Without a command, shows the HDR state of each display.
#[derive(Parser, Debug)]
#[command(
    name = "hdr",
    bin_name = "hdr",
    version,
    about,
    long_about,
    styles = STYLES,
    after_help = examples(),
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Only this display: its number from 'hdr status', or part of its name
    #[arg(
        short,
        long,
        global = true,
        value_name = "N|NAME",
        value_parser = NonEmptyStringValueParser::new()
    )]
    pub display: Option<String>,
}

#[derive(Subcommand, Debug, Clone, Copy, PartialEq)]
pub enum Command {
    /// Show the HDR state of each display
    Status,

    /// Turn HDR on
    On,

    /// Turn HDR off
    Off,

    /// Turn HDR on where it is off, and off where it is on
    Toggle,
}

/// The examples section, styled like clap's own headings and literals.
fn examples() -> String {
    const EXAMPLES: [(&str, &str); 6] = [
        ("hdr", "show the HDR state of each display"),
        ("hdr on", "turn HDR on for every display that supports it"),
        ("hdr off", "turn HDR off everywhere"),
        ("hdr toggle", "switch each display to the other state"),
        ("hdr on -d 2", "turn HDR on for display 2 only"),
        ("hdr on -d oled", "pick the display by part of its name"),
    ];
    let mut out = format!("{HEADER}Examples:{HEADER:#}");
    for (cmd, what) in EXAMPLES {
        out.push_str(&format!("\n  {LITERAL}{cmd:<22}{LITERAL:#} {what}"));
    }
    out
}
