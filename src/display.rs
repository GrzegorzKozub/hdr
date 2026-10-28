//! Safe wrapper over the Windows Display Config API: enumerate active displays
//! and read or set their HDR state. All `unsafe` FFI lives in this module.
//!
//! On Windows 11 24H2+ it uses the HDR-specific GET_ADVANCED_COLOR_INFO_2 /
//! SET_HDR_STATE requests; on older builds it falls back to
//! GET_ADVANCED_COLOR_INFO / SET_ADVANCED_COLOR_STATE.

use windows_sys::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes,
    QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE, DISPLAYCONFIG_DEVICE_INFO_TYPE,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_TARGET_DEVICE_NAME,
    QDC_ONLY_ACTIVE_PATHS,
};
use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, LUID};

// Windows 11 24H2+ request types; not yet exposed by windows-sys 0.61.
const GET_ADVANCED_COLOR_INFO_2: DISPLAYCONFIG_DEVICE_INFO_TYPE = 15;
const SET_HDR_STATE: DISPLAYCONFIG_DEVICE_INFO_TYPE = 16;

/// Layout shared by DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO and its `_2`
/// successor (which appends `activeColorMode`).
#[repr(C)]
struct GetColorInfo {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    bits: u32,
    color_encoding: i32,
    bits_per_color_channel: u32,
    active_color_mode: i32,
}

/// Layout shared by DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE and
/// DISPLAYCONFIG_SET_HDR_STATE: bit 0 is the enable flag.
#[repr(C)]
struct SetState {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    value: u32,
}

pub struct Display {
    adapter_id: LUID,
    target_id: u32,
    pub name: String,
}

pub struct HdrInfo {
    pub supported: bool,
    pub enabled: bool,
    pub limited_by_policy: bool,
    /// Whether the 24H2+ API was used; selects the matching set request.
    v2: bool,
}

fn header(
    ty: DISPLAYCONFIG_DEVICE_INFO_TYPE,
    size: usize,
    d: &Display,
) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: ty,
        size: size as u32,
        adapterId: d.adapter_id,
        id: d.target_id,
    }
}

/// Enumerate active display paths. The path count can change between the two
/// calls (hot-plug, mode change), so retry on ERROR_INSUFFICIENT_BUFFER.
pub fn enumerate_displays() -> Result<Vec<Display>, String> {
    let paths = loop {
        let (mut path_count, mut mode_count) = (0u32, 0u32);
        let rc = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        if rc != ERROR_SUCCESS {
            return Err(format!("GetDisplayConfigBufferSizes failed (code {rc})"));
        }
        let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> =
            vec![unsafe { std::mem::zeroed() }; path_count as usize];
        let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> =
            vec![unsafe { std::mem::zeroed() }; mode_count as usize];
        let rc = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        };
        match rc {
            ERROR_SUCCESS => {
                paths.truncate(path_count as usize);
                break paths;
            }
            ERROR_INSUFFICIENT_BUFFER => continue,
            _ => return Err(format!("QueryDisplayConfig failed (code {rc})")),
        }
    };

    Ok(paths
        .iter()
        .map(|p| {
            let mut d = Display {
                adapter_id: p.targetInfo.adapterId,
                target_id: p.targetInfo.id,
                name: String::new(),
            };
            d.name = monitor_name(&d).unwrap_or_else(|| format!("target {}", d.target_id));
            d
        })
        .collect())
}

fn monitor_name(d: &Display) -> Option<String> {
    let mut req: DISPLAYCONFIG_TARGET_DEVICE_NAME = unsafe { std::mem::zeroed() };
    req.header = header(
        DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        std::mem::size_of_val(&req),
        d,
    );
    if unsafe { DisplayConfigGetDeviceInfo(&mut req.header) } != ERROR_SUCCESS as i32 {
        return None;
    }
    let raw = &req.monitorFriendlyDeviceName;
    let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    (len > 0).then(|| String::from_utf16_lossy(&raw[..len]))
}

pub fn get_hdr_info(d: &Display) -> Result<HdrInfo, String> {
    let mut req: GetColorInfo = unsafe { std::mem::zeroed() };

    // 24H2+: distinguishes HDR from ACM/WCG, which the legacy request lumps
    // together as "advanced color".
    req.header = header(
        GET_ADVANCED_COLOR_INFO_2,
        std::mem::size_of::<GetColorInfo>(),
        d,
    );
    if unsafe { DisplayConfigGetDeviceInfo(&mut req.header) } == ERROR_SUCCESS as i32 {
        return Ok(HdrInfo {
            supported: req.bits & (1 << 4) != 0, // highDynamicRangeSupported
            enabled: req.bits & (1 << 5) != 0,   // highDynamicRangeUserEnabled
            limited_by_policy: req.bits & (1 << 3) != 0, // advancedColorLimitedByPolicy
            v2: true,
        });
    }

    // Legacy struct has no trailing activeColorMode field.
    let size = std::mem::size_of::<GetColorInfo>() - std::mem::size_of::<i32>();
    req.header = header(DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, size, d);
    let rc = unsafe { DisplayConfigGetDeviceInfo(&mut req.header) };
    if rc != ERROR_SUCCESS as i32 {
        return Err(format!("failed to query HDR state (code {rc})"));
    }
    Ok(HdrInfo {
        supported: req.bits & (1 << 0) != 0, // advancedColorSupported
        enabled: req.bits & (1 << 1) != 0,   // advancedColorEnabled
        limited_by_policy: req.bits & (1 << 3) != 0, // advancedColorForceDisabled
        v2: false,
    })
}

pub fn set_hdr_state(d: &Display, info: &HdrInfo, enable: bool) -> Result<(), String> {
    let ty = if info.v2 {
        SET_HDR_STATE
    } else {
        DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE
    };
    let req = SetState {
        header: header(ty, std::mem::size_of::<SetState>(), d),
        value: enable as u32,
    };
    let rc = unsafe { DisplayConfigSetDeviceInfo(&req.header) };
    if rc != ERROR_SUCCESS as i32 {
        return Err(format!("failed to set HDR state (code {rc})"));
    }
    Ok(())
}
