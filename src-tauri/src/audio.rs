//! Audio check: active playback devices with the format Windows mixes to
//! (sample rate, bit depth, channels), and virtual audio drivers. Read-only.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Device {
    pub name: String,
    pub interface: String,
    pub sample_rate: Option<u32>,
    pub bits: Option<u16>,
    pub channels: Option<u16>,
}

/// Mix format from PKEY_AudioEngine_DeviceFormat (WAVEFORMATEX or
/// WAVEFORMATEXTENSIBLE): (channels, sample rate, valid bits).
pub fn parse_wave_format(b: &[u8]) -> Option<(u16, u32, u16)> {
    if b.len() < 16 {
        return None;
    }
    let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    let tag = u16_at(0);
    let channels = u16_at(2);
    let rate = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    let mut bits = u16_at(14);
    // WAVE_FORMAT_EXTENSIBLE: the valid bits follow cbSize.
    if tag == 0xFFFE && b.len() >= 20 {
        let valid = u16_at(18);
        if valid > 0 {
            bits = valid;
        }
    }
    (channels > 0 && rate > 0).then_some((channels, rate, bits))
}

/// Virtual audio layers that sit in the audio path of every game.
pub fn is_virtual(d: &Device) -> bool {
    let s = format!("{} {}", d.name, d.interface).to_ascii_lowercase();
    [
        "nahimic",
        "sonic studio",
        "sonar",
        "virtual",
        "voicemeeter",
        "vb-audio",
        "dolby atmos for headphones",
        "nvidia broadcast",
    ]
    .iter()
    .any(|k| s.contains(k))
}

pub fn hints(d: &Device) -> Vec<&'static str> {
    let mut h = Vec::new();
    if d.sample_rate.is_some_and(|r| r > 48_000) {
        h.push("highRate");
    }
    if is_virtual(d) {
        h.push("virtual");
    }
    h
}

fn device_json(d: &Device) -> Value {
    json!({
        "name": d.name,
        "interface": d.interface,
        "sampleRate": d.sample_rate,
        "bits": d.bits,
        "channels": d.channels,
        "hints": hints(d),
    })
}

#[cfg(windows)]
fn devices() -> Result<Vec<Device>, String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;
    const RENDER: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render";
    const DESC: &str = "{a45c254e-df1c-4efd-8020-67d146a850e0},2";
    const IFACE: &str = "{b3f8fa53-0004-438e-9003-51a46e139bfc},6";
    const FORMAT: &str = "{f19f064d-082c-4e27-bc73-6882a1bb8e4c},0";
    let root = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(RENDER, KEY_READ)
        .map_err(|e| format!("open audio devices: {e}"))?;
    let mut out = Vec::new();
    for id in root.enum_keys().flatten() {
        let Ok(dev) = root.open_subkey_with_flags(&id, KEY_READ) else {
            continue;
        };
        // DEVICE_STATE_ACTIVE = 1.
        if dev.get_value::<u32, _>("DeviceState").ok() != Some(1) {
            continue;
        }
        let Ok(props) = dev.open_subkey_with_flags("Properties", KEY_READ) else {
            continue;
        };
        let fmt = props
            .get_raw_value(FORMAT)
            .ok()
            .and_then(|v| parse_wave_format(&v.bytes));
        out.push(Device {
            name: props.get_value::<String, _>(DESC).unwrap_or_default(),
            interface: props.get_value::<String, _>(IFACE).unwrap_or_default(),
            channels: fmt.map(|f| f.0),
            sample_rate: fmt.map(|f| f.1),
            bits: fmt.map(|f| f.2),
        });
    }
    Ok(out)
}

#[cfg(not(windows))]
fn devices() -> Result<Vec<Device>, String> {
    Err("Windows only".into())
}

pub fn check() -> Value {
    match devices() {
        Ok(d) => json!({ "devices": d.iter().map(device_json).collect::<Vec<_>>() }),
        Err(e) => json!({ "devices": [], "error": e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_extensible_format() {
        // WAVEFORMATEXTENSIBLE, 2 ch, 192 kHz, 32-bit container, 24 valid bits.
        let mut b = vec![0u8; 40];
        b[0..2].copy_from_slice(&0xFFFEu16.to_le_bytes());
        b[2..4].copy_from_slice(&2u16.to_le_bytes());
        b[4..8].copy_from_slice(&192_000u32.to_le_bytes());
        b[14..16].copy_from_slice(&32u16.to_le_bytes());
        b[16..18].copy_from_slice(&22u16.to_le_bytes());
        b[18..20].copy_from_slice(&24u16.to_le_bytes());
        assert_eq!(parse_wave_format(&b), Some((2, 192_000, 24)));
        assert_eq!(parse_wave_format(&[0; 8]), None);
    }

    #[test]
    fn hints() {
        let d = Device {
            name: "Speakers".into(),
            interface: "Nahimic mirroring Device".into(),
            sample_rate: Some(96_000),
            ..Default::default()
        };
        assert_eq!(super::hints(&d), vec!["highRate", "virtual"]);
        let ok = Device {
            name: "Headphones".into(),
            interface: "USB Audio".into(),
            sample_rate: Some(48_000),
            ..Default::default()
        };
        assert!(super::hints(&ok).is_empty());
    }
}
