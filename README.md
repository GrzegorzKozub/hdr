# hdr-toggle

A small Windows CLI, written in Rust, that enables, disables, toggles, or
reports the "Advanced Color" (HDR) state of your displays.

It talks to the same public Windows API — `DisplayConfigGetDeviceInfo` /
`DisplayConfigSetDeviceInfo` with `DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO`
/ `DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE` — that the "HDR
Auto-Toggle" feature in tools like RankFTW/RHI uses to flip HDR on when a
game launches and off when it closes. This is an independent, original
implementation against Microsoft's documented API, not a port of RHI's
(GPL-3.0) source.

## Build

Requires the Rust toolchain (https://rustup.rs) on Windows, targeting MSVC:

```powershell
cargo build --release
```

The binary will be at `target\release\hdr-toggle.exe`. No admin elevation
is required for this particular API.

## Usage

```powershell
hdr-toggle status              # show HDR support/state for every active display
hdr-toggle on                  # enable HDR on every display that supports it
hdr-toggle off                 # disable HDR on every display
hdr-toggle toggle              # flip each display's current state
hdr-toggle on --display 2      # act on just display #2 (index from `status`)
```

Example, to script an HDR-on/HDR-off wrapper around a game launch:

```powershell
hdr-toggle on
Start-Process -Wait "C:\Games\MyGame\game.exe"
hdr-toggle off
```

## How it works

1. `QueryDisplayConfig` enumerates active display paths and gives each one
   an `(adapterId, targetId)` pair.
2. For each display, `DisplayConfigGetDeviceInfo` with
   `DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO` reads a bitfield
   reporting whether advanced color (HDR) is supported, currently enabled,
   or force-disabled by Windows (e.g. no HDR-capable cable/port detected).
3. `DisplayConfigSetDeviceInfo` with
   `DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE` flips the
   `enableAdvancedColor` bit to turn HDR on or off — equivalent to toggling
   "Use HDR" in Settings > System > Display, but scriptable.

## Notes

- If a display reports `supported=false`, Windows doesn't think it's
  HDR-capable on the current connection (wrong port/cable, or the monitor
  genuinely doesn't support it) — the tool can't override that.
- `force-disabled by system` means Windows itself has blocked HDR for that
  display (this flag is informational; `on` will still be attempted but may
  be rejected by the OS).
