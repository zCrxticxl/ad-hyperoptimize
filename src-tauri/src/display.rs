//! Display check: connection type, refresh rate (current vs. highest offered
//! at this resolution), HDR state and bit depth per active monitor, read via
//! the Windows display configuration API. Read-only.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Monitor {
    pub name: String,
    /// DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY.
    pub tech: u32,
    pub width: u32,
    pub height: u32,
    pub hz: f64,
    pub max_hz: u32,
    pub hdr_supported: bool,
    pub hdr_on: bool,
    pub bits_per_channel: u32,
}

pub fn connection(tech: u32) -> &'static str {
    match tech {
        0 => "VGA",
        4 => "DVI",
        5 => "HDMI",
        6 | 11 | 13 | 0x8000_0000 => "internal",
        10 => "DisplayPort",
        12 => "UDI",
        15 => "Miracast",
        16 => "indirect",
        _ => "other",
    }
}

/// Hint keys for one monitor (texts live in the UI).
pub fn hints(m: &Monitor) -> Vec<&'static str> {
    let mut h = Vec::new();
    if m.max_hz as f64 > m.hz.round() + 1.0 {
        h.push("hzBelowMax");
    }
    match connection(m.tech) {
        "HDMI" => h.push("hdmi"),
        "VGA" | "DVI" => h.push("legacyCable"),
        _ => {}
    }
    if m.hdr_on {
        h.push("hdrOn");
    }
    h
}

/// "VRROptimizeEnable=1;..." from DirectXUserGlobalSettings.
pub fn vrr_windowed(dx_settings: Option<&str>) -> Option<bool> {
    let s = dx_settings?;
    s.split(';')
        .find_map(|kv| kv.trim().strip_prefix("VRROptimizeEnable="))
        .map(|v| v.trim() == "1")
}

pub fn monitor_json(m: &Monitor) -> Value {
    json!({
        "name": m.name,
        "connection": connection(m.tech),
        "width": m.width,
        "height": m.height,
        "hz": (m.hz * 100.0).round() / 100.0,
        "maxHz": m.max_hz,
        "hdrSupported": m.hdr_supported,
        "hdrOn": m.hdr_on,
        "bitsPerChannel": m.bits_per_channel,
        "hints": hints(m),
    })
}

#[cfg(windows)]
mod ffi {
    use super::Monitor;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Luid {
        low: u32,
        high: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rational {
        num: u32,
        den: u32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct SourceInfo {
        adapter: Luid,
        id: u32,
        mode_idx: u32,
        status: u32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct TargetInfo {
        adapter: Luid,
        id: u32,
        mode_idx: u32,
        tech: u32,
        rotation: u32,
        scaling: u32,
        refresh: Rational,
        scanline: u32,
        available: i32,
        status: u32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct PathInfo {
        source: SourceInfo,
        target: TargetInfo,
        flags: u32,
    }
    #[repr(C, align(8))]
    #[derive(Clone, Copy)]
    struct ModeInfo([u8; 64]);
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Header {
        kind: u32,
        size: u32,
        adapter: Luid,
        id: u32,
    }
    #[repr(C)]
    struct SourceName {
        header: Header,
        gdi: [u16; 32],
    }
    #[repr(C)]
    struct TargetName {
        header: Header,
        flags: u32,
        tech: u32,
        edid_mfr: u16,
        edid_prod: u16,
        connector: u32,
        friendly: [u16; 64],
        path: [u16; 128],
    }
    #[repr(C)]
    struct AdvancedColor {
        header: Header,
        value: u32,
        encoding: u32,
        bpc: u32,
    }

    // Layouts must match the Windows SDK exactly.
    const _: () = assert!(std::mem::size_of::<PathInfo>() == 72);
    const _: () = assert!(std::mem::size_of::<ModeInfo>() == 64);
    const _: () = assert!(std::mem::size_of::<SourceName>() == 84);
    const _: () = assert!(std::mem::size_of::<TargetName>() == 420);
    const _: () = assert!(std::mem::size_of::<AdvancedColor>() == 32);

    const QDC_ONLY_ACTIVE_PATHS: u32 = 2;
    const GET_SOURCE_NAME: u32 = 1;
    const GET_TARGET_NAME: u32 = 2;
    const GET_ADVANCED_COLOR_INFO: u32 = 9;
    const ENUM_CURRENT_SETTINGS: u32 = 0xFFFF_FFFF;
    /// sizeof(DEVMODEW) and the offsets of the fields read here.
    const DEVMODE_SIZE: usize = 220;
    const DM_SIZE: usize = 68;
    const DM_WIDTH: usize = 172;
    const DM_HEIGHT: usize = 176;
    const DM_FREQ: usize = 184;

    #[link(name = "user32")]
    extern "system" {
        fn GetDisplayConfigBufferSizes(flags: u32, paths: *mut u32, modes: *mut u32) -> i32;
        fn QueryDisplayConfig(
            flags: u32,
            paths: *mut u32,
            path_arr: *mut PathInfo,
            modes: *mut u32,
            mode_arr: *mut ModeInfo,
            topology: *mut c_void,
        ) -> i32;
        fn DisplayConfigGetDeviceInfo(packet: *mut Header) -> i32;
        fn EnumDisplaySettingsW(device: *const u16, mode: u32, devmode: *mut u8) -> i32;
    }

    fn wstr(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    fn u32_at(b: &[u8], off: usize) -> u32 {
        u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
    }

    fn devmode(gdi: &[u16], mode: u32) -> Option<(u32, u32, u32)> {
        let mut dm = [0u8; DEVMODE_SIZE];
        dm[DM_SIZE..DM_SIZE + 2].copy_from_slice(&(DEVMODE_SIZE as u16).to_le_bytes());
        let ok = unsafe { EnumDisplaySettingsW(gdi.as_ptr(), mode, dm.as_mut_ptr()) };
        (ok != 0).then(|| {
            (
                u32_at(&dm, DM_WIDTH),
                u32_at(&dm, DM_HEIGHT),
                u32_at(&dm, DM_FREQ),
            )
        })
    }

    pub fn monitors() -> Result<Vec<Monitor>, String> {
        let (mut np, mut nm) = (0u32, 0u32);
        let rc = unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) };
        if rc != 0 {
            return Err(format!("GetDisplayConfigBufferSizes failed ({rc})"));
        }
        let mut paths = vec![PathInfo::default(); np as usize];
        let mut modes = vec![ModeInfo([0; 64]); nm as usize];
        let rc = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut np,
                paths.as_mut_ptr(),
                &mut nm,
                modes.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        };
        if rc != 0 {
            return Err(format!("QueryDisplayConfig failed ({rc})"));
        }
        paths.truncate(np as usize);
        let mut out = Vec::new();
        for p in &paths {
            let mut tn = TargetName {
                header: Header {
                    kind: GET_TARGET_NAME,
                    size: std::mem::size_of::<TargetName>() as u32,
                    adapter: p.target.adapter,
                    id: p.target.id,
                },
                flags: 0,
                tech: 0,
                edid_mfr: 0,
                edid_prod: 0,
                connector: 0,
                friendly: [0; 64],
                path: [0; 128],
            };
            let name_ok = unsafe { DisplayConfigGetDeviceInfo(&mut tn.header) } == 0;
            let mut ac = AdvancedColor {
                header: Header {
                    kind: GET_ADVANCED_COLOR_INFO,
                    size: std::mem::size_of::<AdvancedColor>() as u32,
                    adapter: p.target.adapter,
                    id: p.target.id,
                },
                value: 0,
                encoding: 0,
                bpc: 0,
            };
            let color_ok = unsafe { DisplayConfigGetDeviceInfo(&mut ac.header) } == 0;
            let mut sn = SourceName {
                header: Header {
                    kind: GET_SOURCE_NAME,
                    size: std::mem::size_of::<SourceName>() as u32,
                    adapter: p.source.adapter,
                    id: p.source.id,
                },
                gdi: [0; 32],
            };
            let (mut width, mut height, mut max_hz) = (0, 0, 0);
            if unsafe { DisplayConfigGetDeviceInfo(&mut sn.header) } == 0 {
                if let Some((w, h, f)) = devmode(&sn.gdi, ENUM_CURRENT_SETTINGS) {
                    (width, height, max_hz) = (w, h, f);
                    let mut i = 0;
                    while let Some((mw, mh, mf)) = devmode(&sn.gdi, i) {
                        if mw == w && mh == h && mf > max_hz {
                            max_hz = mf;
                        }
                        i += 1;
                    }
                }
            }
            let r = p.target.refresh;
            let name = if name_ok {
                wstr(&tn.friendly)
            } else {
                String::new()
            };
            out.push(Monitor {
                name: if name.is_empty() {
                    "Display".into()
                } else {
                    name
                },
                tech: if name_ok { tn.tech } else { p.target.tech },
                width,
                height,
                hz: if r.den > 0 {
                    r.num as f64 / r.den as f64
                } else {
                    0.0
                },
                max_hz,
                hdr_supported: color_ok && ac.value & 1 != 0,
                hdr_on: color_ok && ac.value & 2 != 0,
                bits_per_channel: if color_ok { ac.bpc } else { 0 },
            });
        }
        Ok(out)
    }
}

pub fn check() -> Value {
    #[cfg(windows)]
    let monitors = ffi::monitors();
    #[cfg(not(windows))]
    let monitors: Result<Vec<Monitor>, String> = Err("Windows only".into());
    #[cfg(windows)]
    let dx = match crate::tweaks::reg_read(
        "HKCU",
        r"Software\Microsoft\DirectX\UserGpuPreferences",
        "DirectXUserGlobalSettings",
    ) {
        Ok(Some(crate::safety::RegVal::Str(v))) => Some(v),
        _ => None,
    };
    #[cfg(not(windows))]
    let dx: Option<String> = None;
    match monitors {
        Ok(ms) => json!({
            "monitors": ms.iter().map(monitor_json).collect::<Vec<_>>(),
            "vrrWindowed": vrr_windowed(dx.as_deref()),
        }),
        Err(e) => json!({ "monitors": [], "error": e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_for_typical_setups() {
        let m = Monitor {
            tech: 5,
            hz: 59.94,
            max_hz: 144,
            hdr_on: true,
            ..Default::default()
        };
        assert_eq!(hints(&m), vec!["hzBelowMax", "hdmi", "hdrOn"]);
        let dp = Monitor {
            tech: 10,
            hz: 239.76,
            max_hz: 240,
            ..Default::default()
        };
        assert!(hints(&dp).is_empty(), "239.76 Hz is the 240 Hz mode");
        assert_eq!(
            hints(&Monitor {
                tech: 4,
                hz: 60.0,
                max_hz: 60,
                ..Default::default()
            }),
            vec!["legacyCable"]
        );
    }

    #[test]
    fn reads_windowed_vrr_flag() {
        assert_eq!(
            vrr_windowed(Some("SwapEffectUpgradeEnable=1;VRROptimizeEnable=1;")),
            Some(true)
        );
        assert_eq!(vrr_windowed(Some("VRROptimizeEnable=0;")), Some(false));
        assert_eq!(vrr_windowed(Some("SwapEffectUpgradeEnable=1;")), None);
        assert_eq!(vrr_windowed(None), None);
    }
}
