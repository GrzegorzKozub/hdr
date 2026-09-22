//! hdr-toggle — enable, disable, toggle, or report Windows "Advanced Color" (HDR)
//! state for every active display, from the command line.
//!
//! This reimplements, in Rust, the same public Windows Display Config
//! mechanism (DisplayConfigGetDeviceInfo / DisplayConfigSetDeviceInfo with
//! DISPLAYCONFIG_*_ADVANCED_COLOR_*) that HDR-toggle utilities such as
//! RankFTW/RHI's "HDR Auto-Toggle" feature use under the hood. It is an
//! original implementation against Microsoft's public API, not a port of
//! any GPL-licensed source.
//!
//! Usage:
//!   hdr-toggle status          # print HDR support/enabled state per display
//!   hdr-toggle on              # enable HDR on every display that supports it
//!   hdr-toggle off             # disable HDR on every display
//!   hdr-toggle toggle          # flip each display's current state
//!   hdr-toggle on  --display 1 # act on a single display (1-based, from `status`)

use std::env;
use std::process::ExitCode;

use windows_sys::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes,
    QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE,
    DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE, QDC_ONLY_ACTIVE_PATHS,
};
use windows_sys::Win32::Foundation::LUID;

const ERROR_SUCCESS: i32 = 0;

/// One active display path, identified the way the DisplayConfig API wants it.
struct Display {
    adapter_id: LUID,
    target_id: u32,
}

/// Snapshot of a display's HDR-relevant state, decoded from the bitfield
/// union that `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO` returns as a raw u32.
struct ColorInfo {
    advanced_color_supported: bool,
    advanced_color_enabled: bool,
    advanced_color_force_disabled: bool,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("status");

    let only_index: Option<usize> = args
        .iter()
        .position(|a| a == "--display")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<usize>().ok());

    let displays = match enumerate_displays() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if displays.is_empty() {
        eprintln!("no active displays found");
        return ExitCode::FAILURE;
    }

    let mut had_error = false;

    for (i, display) in displays.iter().enumerate() {
        let index = i + 1;
        if let Some(only) = only_index {
            if only != index {
                continue;
            }
        }

        let info = match get_advanced_color_info(display) {
            Ok(info) => info,
            Err(e) => {
                eprintln!("display {index}: failed to query state: {e}");
                had_error = true;
                continue;
            }
        };

        match command {
            "status" => {
                println!(
                    "display {index}: supported={} enabled={}{}",
                    info.advanced_color_supported,
                    info.advanced_color_enabled,
                    if info.advanced_color_force_disabled {
                        " (force-disabled by system)"
                    } else {
                        ""
                    }
                );
            }
            "on" | "off" | "toggle" => {
                if !info.advanced_color_supported {
                    println!("display {index}: HDR not supported, skipping");
                    continue;
                }
                let target_state = match command {
                    "on" => true,
                    "off" => false,
                    "toggle" => !info.advanced_color_enabled,
                    _ => unreachable!(),
                };
                if target_state == info.advanced_color_enabled {
                    println!(
                        "display {index}: already {}",
                        if target_state { "on" } else { "off" }
                    );
                    continue;
                }
                match set_advanced_color_state(display, target_state) {
                    Ok(()) => println!(
                        "display {index}: HDR {}",
                        if target_state { "enabled" } else { "disabled" }
                    ),
                    Err(e) => {
                        eprintln!("display {index}: failed to set state: {e}");
                        had_error = true;
                    }
                }
            }
            other => {
                eprintln!("unknown command '{other}'");
                eprintln!("usage: hdr-toggle [status|on|off|toggle] [--display N]");
                return ExitCode::FAILURE;
            }
        }
    }

    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Enumerate every active display path via QueryDisplayConfig and return
/// the (adapterId, targetId) pair each one needs for the advanced-color calls.
fn enumerate_displays() -> Result<Vec<Display>, String> {
    unsafe {
        let mut path_count: u32 = 0;
        let mut mode_count: u32 = 0;

        let rc = GetDisplayConfigBufferSizes(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            &mut mode_count,
        );
        if rc != ERROR_SUCCESS as u32 {
            return Err(format!("GetDisplayConfigBufferSizes failed (code {rc})"));
        }

        let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> =
            vec![std::mem::zeroed(); path_count as usize];
        let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> =
            vec![std::mem::zeroed(); mode_count as usize];

        let rc = QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            std::ptr::null_mut(),
        );
        if rc != ERROR_SUCCESS as u32 {
            return Err(format!("QueryDisplayConfig failed (code {rc})"));
        }

        paths.truncate(path_count as usize);

        Ok(paths
            .iter()
            .map(|p| Display {
                adapter_id: p.targetInfo.adapterId,
                target_id: p.targetInfo.id,
            })
            .collect())
    }
}

/// Query current HDR / advanced-color state for one display.
fn get_advanced_color_info(display: &Display) -> Result<ColorInfo, String> {
    unsafe {
        let mut req: DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO = std::mem::zeroed();
        req.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
            size: std::mem::size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32,
            adapterId: display.adapter_id,
            id: display.target_id,
        };

        let rc = DisplayConfigGetDeviceInfo(&mut req.header);
        if rc != ERROR_SUCCESS {
            return Err(format!("DisplayConfigGetDeviceInfo failed (code {rc})"));
        }

        // The bitfield union also exposes a plain `value: u32` member, which
        // is the stable way to read it without depending on bit ordering.
        let bits = req.Anonymous.value;
        Ok(ColorInfo {
            advanced_color_supported: bits & 0x1 != 0,
            advanced_color_enabled: bits & 0x2 != 0,
            advanced_color_force_disabled: bits & 0x8 != 0,
        })
    }
}

/// Set HDR / advanced-color state for one display.
fn set_advanced_color_state(display: &Display, enable: bool) -> Result<(), String> {
    unsafe {
        let mut req: DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE = std::mem::zeroed();
        req.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE,
            size: std::mem::size_of::<DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE>() as u32,
            adapterId: display.adapter_id,
            id: display.target_id,
        };
        // Bit 0 of the union is `enableAdvancedColor`.
        req.Anonymous.value = if enable { 1 } else { 0 };

        let rc = DisplayConfigSetDeviceInfo(&req.header);
        if rc != ERROR_SUCCESS {
            return Err(format!("DisplayConfigSetDeviceInfo failed (code {rc})"));
        }
        Ok(())
    }
}
