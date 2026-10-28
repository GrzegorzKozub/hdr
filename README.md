# hdr

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

The binary will be at `target\release\hdr.exe`. No admin elevation
is required for this particular API.

## Usage

```powershell
hdr status              # show HDR support/state for every active display
hdr on                  # enable HDR on every display that supports it
hdr off                 # disable HDR on every display
hdr toggle              # flip each display's current state
hdr on --display 2      # act on just display #2 (index from `status`; -d also works)
hdr on -d "HDR 4K"      # or pick a display by monitor name (case-insensitive substring)
```

Example, to script an HDR-on/HDR-off wrapper around a game launch:

```powershell
hdr on
Start-Process -Wait "C:\Games\MyGame\game.exe"
hdr off
```

## How it works

1. `QueryDisplayConfig` enumerates active display paths and gives each one
   an `(adapterId, targetId)` pair (retried if the display set changes
   mid-query). `DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME` supplies the
   monitor name shown in the output.
2. On Windows 11 24H2+, `DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO_2`
   reports HDR support and the user's HDR setting, and
   `DISPLAYCONFIG_DEVICE_INFO_SET_HDR_STATE` turns HDR on or off — equivalent
   to "Use HDR" in Settings > System > Display.
3. On older builds the tool falls back to the legacy
   `GET_ADVANCED_COLOR_INFO` / `SET_ADVANCED_COLOR_STATE` requests.

The legacy requests treat "advanced color" as a single flag, so on 24H2+
with Auto Color Management (ACM) enabled they report HDR as on even when
it's off, and report SDR wide-color displays as HDR-capable. The newer
requests keep HDR and ACM separate.

## Notes

- Displays are numbered in `QueryDisplayConfig` path order, which can change
  when monitors are replugged; selecting by name is more robust for scripts.
  A name must match exactly one display — an ambiguous or unknown name is
  an error that lists the available displays.
- If a display reports `supported=false`, Windows doesn't think it's
  HDR-capable on the current connection (wrong port/cable, or the monitor
  genuinely doesn't support it) — the tool can't override that.
- `limited by system policy` means Windows itself is restricting advanced
  color on that display (e.g. battery saver). `on` is still attempted but
  may be rejected by the OS.
- Exit code is non-zero on bad arguments or if any display fails to query
  or switch.
