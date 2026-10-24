//! Hardware misconfiguration checks that cost real performance and that the
//! usual tools never look at: RAM channel layout, PCIe link of the GPU and the
//! NVMe drives, processor power-plan limits left behind by tweaks, and the USB
//! controller layout of mouse and keyboard. Reading is side-effect free; only
//! the power-plan reset changes anything, journaled with undo.

use serde_json::{json, Value};

// ── RAM channels ─────────────────────────────────────────────────────────────

/// Channel letter of a module from its bank label / device locator, e.g.
/// "P0 CHANNEL A", "Controller0-ChannelB-DIMM1", "DIMM_A2", "DIMMB1".
pub fn channel_of(bank: &str, locator: &str) -> Option<char> {
    for s in [bank, locator] {
        let u = s.to_ascii_uppercase();
        if let Some(i) = u.find("CHANNEL") {
            let rest = u[i + 7..].trim_start_matches([' ', '-', '_']);
            if let Some(c) = rest.chars().next().filter(|c| c.is_ascii_alphabetic()) {
                return Some(c);
            }
        }
    }
    slot_of(locator).map(|(c, _)| c)
}

/// (channel letter, slot number) from locators like "DIMM_A2", "DIMMB1", "A1".
pub fn slot_of(locator: &str) -> Option<(char, u8)> {
    let u = locator.to_ascii_uppercase();
    let rest = u.strip_prefix("DIMM").unwrap_or(&u);
    let t: String = rest.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let mut ch = t.chars();
    let (l, d) = (ch.next()?, ch.next()?);
    (l.is_ascii_alphabetic() && l <= 'H' && d.is_ascii_digit() && ch.next().is_none())
        .then(|| (l, d.to_digit(10).unwrap_or(0) as u8))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub bank: String,
    pub locator: String,
    pub gb: u64,
    pub speed: u32,
    pub part: String,
}

/// Verdict and hints for the installed modules on a board with `slots`.
pub fn ram_verdict(mods: &[Module], slots: u32) -> Value {
    let mut chans: Vec<char> = mods
        .iter()
        .filter_map(|m| channel_of(&m.bank, &m.locator))
        .collect();
    chans.sort();
    chans.dedup();
    let known = mods
        .iter()
        .all(|m| channel_of(&m.bank, &m.locator).is_some());
    let mode = match (mods.len(), known, chans.len()) {
        (0, _, _) => "unknown",
        (1, _, _) => "single",
        (_, true, 1) => "singleWrongSlots",
        (_, true, _) => "multi",
        _ => "unknown",
    };
    let mut hints = Vec::new();
    if mode == "single" {
        hints.push("addModule");
    }
    if mode == "singleWrongSlots" {
        hints.push("wrongSlots");
    }
    // Two modules on a four-slot board in the first slot of each channel:
    // most boards want the second slot (A2/B2) for two modules.
    let slot_nums: Vec<u8> = mods
        .iter()
        .filter_map(|m| slot_of(&m.locator).map(|s| s.1))
        .collect();
    if slots == 4 && mods.len() == 2 && slot_nums.len() == 2 && slot_nums.iter().all(|&n| n == 1) {
        hints.push("checkA2B2");
    }
    let caps: std::collections::BTreeSet<u64> = mods.iter().map(|m| m.gb).collect();
    if caps.len() > 1 {
        hints.push("mixedCapacity");
    }
    let parts: std::collections::BTreeSet<&str> = mods
        .iter()
        .map(|m| m.part.as_str())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() > 1 {
        hints.push("mixedKits");
    }
    json!({
        "mode": mode,
        "channels": chans.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
        "slots": slots,
        "modules": mods.iter().map(|m| json!({
            "bank": m.bank, "locator": m.locator, "gb": m.gb, "speed": m.speed, "part": m.part,
        })).collect::<Vec<_>>(),
        "hints": hints,
    })
}

#[cfg(windows)]
const RAM_PS: &str = r#"
$slots = (Get-CimInstance Win32_PhysicalMemoryArray -ErrorAction SilentlyContinue | Measure-Object -Property MemoryDevices -Sum).Sum
$mods = @(Get-CimInstance Win32_PhysicalMemory -ErrorAction SilentlyContinue | ForEach-Object {
    @{ bank = [string]$_.BankLabel; locator = [string]$_.DeviceLocator; gb = [int64]([math]::Round($_.Capacity / 1GB));
       speed = [int]$_.ConfiguredClockSpeed; part = ([string]$_.PartNumber).Trim() } })
@{ slots = [int]$slots; modules = $mods } | ConvertTo-Json -Depth 3 -Compress
"#;

#[cfg(windows)]
fn ram() -> Value {
    let v = crate::ps::run_json(RAM_PS).unwrap_or(Value::Null);
    let arr = match &v["modules"] {
        Value::Array(a) => a.clone(),
        o @ Value::Object(_) => vec![o.clone()],
        _ => vec![],
    };
    let s = |m: &Value, k: &str| m[k].as_str().unwrap_or_default().to_string();
    let mods: Vec<Module> = arr
        .iter()
        .map(|m| Module {
            bank: s(m, "bank"),
            locator: s(m, "locator"),
            gb: m["gb"].as_u64().unwrap_or(0),
            speed: m["speed"].as_u64().unwrap_or(0) as u32,
            part: s(m, "part"),
        })
        .collect();
    ram_verdict(&mods, v["slots"].as_u64().unwrap_or(0) as u32)
}

// ── PCIe links ───────────────────────────────────────────────────────────────

/// Link verdict: lanes missing is a real loss; a lower generation than the
/// device supports is either the board/slot limit, a riser or BIOS setting,
/// or (for GPUs at idle) normal power saving.
pub fn link_verdict(cur_gen: u32, max_gen: u32, cur_width: u32, max_width: u32) -> &'static str {
    if cur_width == 0 || max_width == 0 || cur_gen == 0 || max_gen == 0 {
        "unknown"
    } else if cur_width < max_width {
        "lanes"
    } else if cur_gen < max_gen {
        "gen"
    } else {
        "ok"
    }
}

#[cfg(windows)]
const LINKS_PS: &str = r#"
$keys = 'DEVPKEY_PciDevice_CurrentLinkSpeed','DEVPKEY_PciDevice_CurrentLinkWidth','DEVPKEY_PciDevice_MaxLinkSpeed','DEVPKEY_PciDevice_MaxLinkWidth'
function L($id) { $p = @{}; Get-PnpDeviceProperty -InstanceId $id -KeyName $keys -ErrorAction SilentlyContinue | ForEach-Object { $p[$_.KeyName] = $_.Data }
    @{ curGen = [int]$p[$keys[0]]; curWidth = [int]$p[$keys[1]]; maxGen = [int]$p[$keys[2]]; maxWidth = [int]$p[$keys[3]] } }
$gpus = @(Get-PnpDevice -PresentOnly -Class Display -ErrorAction SilentlyContinue | Where-Object { $_.InstanceId -like 'PCI\*' } | ForEach-Object {
    $l = L $_.InstanceId; $l.name = [string]$_.FriendlyName; $l })
$nvme = @(Get-PnpDevice -PresentOnly -Class DiskDrive -ErrorAction SilentlyContinue | ForEach-Object {
    $par = (Get-PnpDeviceProperty -InstanceId $_.InstanceId -KeyName 'DEVPKEY_Device_Parent' -ErrorAction SilentlyContinue).Data
    if ($par -like 'PCI\*') { $l = L $par; $l.name = [string]$_.FriendlyName; $l } })
@{ gpus = $gpus; nvme = $nvme } | ConvertTo-Json -Depth 3 -Compress
"#;

fn with_verdict(mut d: Value) -> Value {
    let n = |k: &str| d[k].as_u64().unwrap_or(0) as u32;
    let v = link_verdict(n("curGen"), n("maxGen"), n("curWidth"), n("maxWidth"));
    d["verdict"] = json!(v);
    d
}

#[cfg(windows)]
fn links() -> Value {
    let v = crate::ps::run_json(LINKS_PS).unwrap_or(Value::Null);
    let list = |k: &str| -> Vec<Value> {
        match &v[k] {
            Value::Array(a) => a.iter().cloned().map(with_verdict).collect(),
            o @ Value::Object(_) => vec![with_verdict(o.clone())],
            _ => vec![],
        }
    };
    json!({ "gpus": list("gpus"), "nvme": list("nvme") })
}

/// Live GPU link from nvidia-smi (gen.max already accounts for the board).
/// Sampled by the UI while it puts load on the GPU, since an idle GPU drops
/// its link generation on purpose.
#[cfg(windows)]
pub fn gpu_link_live() -> Value {
    let smi = format!(r"{}\nvidia-smi.exe", crate::ps::system_dir());
    if !std::path::Path::new(&smi).is_file() {
        return Value::Null;
    }
    let q = "--query-gpu=pcie.link.gen.current,pcie.link.gen.max,pcie.link.width.current,pcie.link.width.max,utilization.gpu";
    match crate::ps::exec_capture(&smi, &[q, "--format=csv,noheader,nounits"]) {
        Ok((st, out, _)) if st.success() => parse_smi_link(out.lines().next().unwrap_or_default()),
        _ => Value::Null,
    }
}

#[cfg(not(windows))]
pub fn gpu_link_live() -> Value {
    Value::Null
}

pub fn parse_smi_link(line: &str) -> Value {
    let f: Vec<u32> = line
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if f.len() != 5 {
        return Value::Null;
    }
    with_verdict(
        json!({ "curGen": f[0], "maxGen": f[1], "curWidth": f[2], "maxWidth": f[3], "util": f[4] }),
    )
}

// ── Power plan processor limits ──────────────────────────────────────────────

const BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

/// (AC, DC) from `powercfg /query <scheme> SUB_PROCESSOR <alias>`: the last two
/// hex values of the output (labels are localized, the numbers are not).
pub fn parse_powercfg_values(out: &str) -> Option<(u32, u32)> {
    let hex: Vec<u32> = out
        .split_whitespace()
        .filter_map(|t| {
            t.strip_prefix("0x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
        })
        .collect();
    (hex.len() >= 2).then(|| (hex[hex.len() - 2], hex[hex.len() - 1]))
}

/// Issue key for a setting value, given whether the plan is Balanced.
pub fn power_issue(alias: &str, value: u32, balanced: bool) -> Option<&'static str> {
    match alias {
        "PROCTHROTTLEMAX" if value < 100 => Some("maxBelow100"),
        "PROCTHROTTLEMIN" if balanced && value >= 50 => Some("minHigh"),
        "PERFBOOSTMODE" if value == 0 => Some("boostOff"),
        _ => None,
    }
}

/// Windows default to reset an alias to.
fn power_default(alias: &str) -> u32 {
    match alias {
        "PROCTHROTTLEMAX" => 100,
        "PROCTHROTTLEMIN" => 5,
        _ => 2, // PERFBOOSTMODE: Aggressive
    }
}

const POWER_ALIASES: &[&str] = &["PROCTHROTTLEMAX", "PROCTHROTTLEMIN", "PERFBOOSTMODE"];

#[cfg(windows)]
fn active_scheme() -> Option<String> {
    let out = crate::ps::exec("powercfg.exe", &["/getactivescheme"]).ok()?;
    out.split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .find(|t| crate::ps::is_guid(t))
        .map(|t| t.to_ascii_lowercase())
}

#[cfg(windows)]
fn power() -> Value {
    let scheme = active_scheme();
    let balanced = scheme.as_deref() == Some(BALANCED);
    let settings: Vec<Value> = POWER_ALIASES
        .iter()
        .filter_map(|a| {
            let out = crate::ps::exec(
                "powercfg.exe",
                &["/query", "SCHEME_CURRENT", "SUB_PROCESSOR", a],
            )
            .ok()?;
            let (ac, dc) = parse_powercfg_values(&out)?;
            Some(json!({
                "alias": a, "ac": ac, "dc": dc,
                "acIssue": power_issue(a, ac, balanced),
                "dcIssue": power_issue(a, dc, balanced),
                "open": crate::perftweaks::open_entry(&format!("hwcheck:power:{a}")).map(|e| e.id),
            }))
        })
        .collect();
    json!({ "scheme": scheme, "balanced": balanced, "settings": settings })
}

/// Resets one processor power setting (AC and DC) to its Windows default,
/// journaled so it can be undone.
#[cfg(windows)]
pub fn power_fix(alias: String) -> Result<Value, String> {
    if !POWER_ALIASES.contains(&alias.as_str()) {
        return Err("Unknown setting".into());
    }
    let out = crate::ps::exec(
        "powercfg.exe",
        &["/query", "SCHEME_CURRENT", "SUB_PROCESSOR", &alias],
    )?;
    let (ac, dc) = parse_powercfg_values(&out).ok_or("cannot read the current value")?;
    let want = power_default(&alias);
    let set = |which: &str, v: u32| {
        crate::perftweaks::ps_line(&format!(
            "powercfg /set{which}valueindex SCHEME_CURRENT SUB_PROCESSOR {alias} {v}; if ($LASTEXITCODE -ne 0) {{ exit 1 }}; powercfg /setactive SCHEME_CURRENT"
        ))
    };
    let _engine = crate::tweaks::engine_guard()?;
    let id = crate::perftweaks::apply_journaled(
        &format!("hwcheck:power:{alias}"),
        &format!("Power plan {alias} to Windows default"),
        vec![
            crate::safety::ChangeItem::Command {
                applied: set("ac", want),
                revert: set("ac", ac),
            },
            crate::safety::ChangeItem::Command {
                applied: set("dc", want),
                revert: set("dc", dc),
            },
        ],
    )?;
    Ok(json!({ "ok": true, "entryId": id }))
}

#[cfg(not(windows))]
pub fn power_fix(_alias: String) -> Result<Value, String> {
    Err("Windows only".into())
}

// ── USB topology ─────────────────────────────────────────────────────────────

/// PCI vendor IDs of add-on USB controllers (not CPU/chipset).
pub fn third_party_controller(instance: &str) -> bool {
    let u = instance.to_ascii_uppercase();
    ["VEN_1B21", "VEN_1106", "VEN_1912", "VEN_1B73", "VEN_1B6F"]
        .iter()
        .any(|v| u.contains(v))
}

#[cfg(windows)]
const USB_PS: &str = r#"
$all = @{}; Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue | ForEach-Object { $all[$_.InstanceId] = $_ }
$parent = @{}
function P($id) { if (-not $parent.ContainsKey($id)) { $parent[$id] = (Get-PnpDeviceProperty -InstanceId $id -KeyName 'DEVPKEY_Device_Parent' -ErrorAction SilentlyContinue).Data }; $parent[$id] }
$kinds = @{ Mouse = 'mouse'; Keyboard = 'keyboard'; Camera = 'camera'; Image = 'camera'; MEDIA = 'audio' }
@($all.Values | Where-Object { $kinds.ContainsKey([string]$_.Class) } | ForEach-Object {
    $d = $_; $id = $d.InstanceId; $hub = $false; $ctrl = $null; $usb = $false
    for ($i = 0; $i -lt 12 -and $id; $i++) {
        $id = P $id
        if (-not $id) { break }
        if ($id -like 'USB\*') { $usb = $true; if ($id -notlike 'USB\ROOT_HUB*' -and [string]$all[$id].Class -eq 'USB' -and ([string]$all[$id].FriendlyName) -match 'hub') { $hub = $true } }
        if ($id -like 'PCI\*') { $ctrl = $id; break }
    }
    if ($usb -and $ctrl) { @{ name = [string]$d.FriendlyName; kind = $kinds[[string]$d.Class]; hub = $hub; controller = $ctrl; controllerName = [string]$all[$ctrl].FriendlyName } }
}) | ConvertTo-Json -Depth 3 -Compress
"#;

/// Groups devices by controller and adds hints per controller.
pub fn usb_layout(devices: &[Value]) -> Value {
    let mut ctrls: Vec<String> = Vec::new();
    for d in devices {
        if let Some(c) = d["controller"].as_str() {
            if !ctrls.iter().any(|x| x == c) {
                ctrls.push(c.to_string());
            }
        }
    }
    let out: Vec<Value> = ctrls
        .iter()
        .map(|c| {
            let devs: Vec<&Value> = devices.iter().filter(|d| d["controller"].as_str() == Some(c)).collect();
            let has = |k: &str| devs.iter().any(|d| d["kind"] == k);
            let input = has("mouse") || has("keyboard");
            let mut hints = Vec::new();
            if input && (has("camera") || has("audio")) {
                hints.push("sharedWithStreaming");
            }
            if input && third_party_controller(c) {
                hints.push("thirdParty");
            }
            if devs.iter().any(|d| (d["kind"] == "mouse" || d["kind"] == "keyboard") && d["hub"] == true) {
                hints.push("viaHub");
            }
            json!({
                "controller": c,
                "name": devs.first().and_then(|d| d["controllerName"].as_str()).unwrap_or(""),
                "thirdParty": third_party_controller(c),
                "devices": devs.iter().map(|d| json!({ "name": d["name"], "kind": d["kind"], "hub": d["hub"] })).collect::<Vec<_>>(),
                "hints": hints,
            })
        })
        .collect();
    json!({ "controllers": out })
}

#[cfg(windows)]
fn usb() -> Value {
    let devices = match crate::ps::run_json(USB_PS) {
        Ok(Value::Array(a)) => a,
        Ok(o @ Value::Object(_)) => vec![o],
        _ => vec![],
    };
    usb_layout(&devices)
}

pub fn check() -> Value {
    #[cfg(windows)]
    {
        json!({ "ram": ram(), "links": links(), "power": power(), "usb": usb() })
    }
    #[cfg(not(windows))]
    json!({ "error": "Windows only" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(bank: &str, loc: &str, gb: u64) -> Module {
        Module {
            bank: bank.into(),
            locator: loc.into(),
            gb,
            speed: 3600,
            part: "X".into(),
        }
    }

    #[test]
    fn channel_and_slot_parsing() {
        assert_eq!(channel_of("P0 CHANNEL A", "DIMM 0"), Some('A'));
        assert_eq!(
            channel_of("BANK 0", "Controller0-ChannelB-DIMM1"),
            Some('B')
        );
        assert_eq!(channel_of("BANK 1", "DIMM_A2"), Some('A'));
        assert_eq!(slot_of("DIMMB1"), Some(('B', 1)));
        assert_eq!(slot_of("DIMM 0"), None);
    }

    #[test]
    fn ram_modes() {
        assert_eq!(
            ram_verdict(&[m("P0 CHANNEL A", "DIMM 0", 16)], 4)["mode"],
            "single"
        );
        let wrong = ram_verdict(&[m("BANK 0", "DIMM_A1", 16), m("BANK 1", "DIMM_A2", 16)], 4);
        assert_eq!(wrong["mode"], "singleWrongSlots");
        let a1b1 = ram_verdict(&[m("BANK 0", "DIMM_A1", 16), m("BANK 2", "DIMM_B1", 16)], 4);
        assert_eq!(a1b1["mode"], "multi");
        assert_eq!(a1b1["hints"], json!(["checkA2B2"]));
        let mixed = ram_verdict(&[m("BANK 1", "DIMM_A2", 16), m("BANK 3", "DIMM_B2", 8)], 4);
        assert_eq!(mixed["hints"], json!(["mixedCapacity"]));
    }

    #[test]
    fn link_verdicts() {
        assert_eq!(link_verdict(4, 4, 16, 16), "ok");
        assert_eq!(link_verdict(4, 4, 4, 16), "lanes");
        assert_eq!(link_verdict(3, 4, 16, 16), "gen");
        assert_eq!(link_verdict(0, 4, 16, 16), "unknown");
        assert_eq!(parse_smi_link("4, 4, 16, 16, 97")["verdict"], "ok");
        assert_eq!(parse_smi_link("1, 4, 8, 16, 0")["verdict"], "lanes");
    }

    #[test]
    fn powercfg_parsing_and_issues() {
        let out = "Power Setting GUID: bc5038f7-23e0-4960-96da-33abaf5935ec  (Maximum processor state)\n  Minimum Possible Setting: 0x00000000\n  Maximum Possible Setting: 0x00000064\n  Possible Settings increment: 0x00000001\n  Possible Settings units: %\nCurrent AC Power Setting Index: 0x0000005a\nCurrent DC Power Setting Index: 0x00000064\n";
        assert_eq!(parse_powercfg_values(out), Some((90, 100)));
        assert_eq!(
            power_issue("PROCTHROTTLEMAX", 90, true),
            Some("maxBelow100")
        );
        assert_eq!(
            power_issue("PROCTHROTTLEMIN", 100, false),
            None,
            "High performance default"
        );
        assert_eq!(power_issue("PROCTHROTTLEMIN", 100, true), Some("minHigh"));
        assert_eq!(power_issue("PERFBOOSTMODE", 0, true), Some("boostOff"));
    }

    #[test]
    fn usb_hints() {
        let d = |name: &str, kind: &str, ctrl: &str, hub: bool| json!({ "name": name, "kind": kind, "controller": ctrl, "controllerName": "x", "hub": hub });
        let l = usb_layout(&[
            d("Mouse", "mouse", r"PCI\VEN_1022&DEV_149C\1", false),
            d("Cam", "camera", r"PCI\VEN_1022&DEV_149C\1", false),
            d("Keyboard", "keyboard", r"PCI\VEN_1B21&DEV_2142\2", true),
        ]);
        assert_eq!(l["controllers"][0]["hints"], json!(["sharedWithStreaming"]));
        assert_eq!(
            l["controllers"][1]["hints"],
            json!(["thirdParty", "viaHub"])
        );
    }
}
