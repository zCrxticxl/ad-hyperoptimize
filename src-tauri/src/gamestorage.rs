//! Game storage check: where installed games live (Steam, Ubisoft Connect,
//! Epic) and what those drives are (SSD/HDD, bus, free space), plus whether
//! TRIM is enabled. Read-only.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub name: String,
    pub launcher: &'static str,
    pub path: String,
}

/// Library paths from Steam's libraryfolders.vdf (`"path"  "D:\\SteamLibrary"`).
pub fn parse_vdf_paths(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix("\"path\"")?.trim();
            let v = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(v.replace("\\\\", "\\"))
        })
        .collect()
}

/// A `"key"  "value"` field from an .acf/.vdf file.
pub fn vdf_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    text.lines().find_map(|l| {
        let rest = l.trim().strip_prefix(&needle)?.trim();
        Some(
            rest.strip_prefix('"')?
                .strip_suffix('"')?
                .replace("\\\\", "\\"),
        )
    })
}

/// Drive letter of a path ("D:\\Games\\x" -> 'D').
pub fn drive_letter(path: &str) -> Option<char> {
    let mut c = path.chars();
    let l = c.next()?.to_ascii_uppercase();
    (l.is_ascii_alphabetic() && c.next() == Some(':')).then_some(l)
}

#[cfg(windows)]
fn steam_games() -> Vec<Game> {
    let Ok(Some(crate::safety::RegVal::Str(steam))) =
        crate::tweaks::reg_read("HKCU", r"Software\Valve\Steam", "SteamPath")
    else {
        return vec![];
    };
    let steam = steam.replace('/', "\\");
    let mut libs = vec![steam.clone()];
    if let Ok(vdf) = std::fs::read_to_string(format!(r"{steam}\steamapps\libraryfolders.vdf")) {
        for p in parse_vdf_paths(&vdf) {
            if !libs.iter().any(|l| l.eq_ignore_ascii_case(&p)) {
                libs.push(p);
            }
        }
    }
    let mut out = Vec::new();
    for lib in libs {
        let apps = format!(r"{lib}\steamapps");
        for e in std::fs::read_dir(&apps).into_iter().flatten().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !(n.starts_with("appmanifest_") && n.ends_with(".acf")) {
                continue;
            }
            let Ok(acf) = std::fs::read_to_string(e.path()) else {
                continue;
            };
            if let (Some(name), Some(dir)) =
                (vdf_field(&acf, "name"), vdf_field(&acf, "installdir"))
            {
                out.push(Game {
                    name,
                    launcher: "Steam",
                    path: format!(r"{apps}\common\{dir}"),
                });
            }
        }
    }
    out
}

#[cfg(windows)]
fn ubisoft_games() -> Vec<Game> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    let Ok(installs) = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs", KEY_READ)
    else {
        return vec![];
    };
    installs
        .enum_keys()
        .flatten()
        .filter_map(|id| {
            let k = installs.open_subkey_with_flags(&id, KEY_READ).ok()?;
            let dir: String = k.get_value("InstallDir").ok()?;
            let dir = dir.replace('/', "\\");
            let name = dir
                .trim_end_matches('\\')
                .rsplit('\\')
                .next()
                .unwrap_or(&id)
                .to_string();
            Some(Game {
                name,
                launcher: "Ubisoft Connect",
                path: dir,
            })
        })
        .collect()
}

#[cfg(windows)]
fn epic_games() -> Vec<Game> {
    let dir = format!(
        r"{}\Epic\EpicGamesLauncher\Data\Manifests",
        std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into())
    );
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "item"))
        .filter_map(|e| {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            Some(Game {
                name: v["DisplayName"].as_str()?.to_string(),
                launcher: "Epic",
                path: v["InstallLocation"].as_str()?.to_string(),
            })
        })
        .collect()
}

#[cfg(windows)]
const DRIVES_PS: &str = r#"
@(Get-Partition -ErrorAction SilentlyContinue | Where-Object { $_.DriveLetter } | ForEach-Object {
    $p = $_
    $pd = Get-PhysicalDisk -ErrorAction SilentlyContinue | Where-Object { [string]$_.DeviceId -eq [string]$p.DiskNumber } | Select-Object -First 1
    $v = Get-Volume -DriveLetter $p.DriveLetter -ErrorAction SilentlyContinue
    [pscustomobject]@{ letter = [string]$p.DriveLetter; media = [string]$pd.MediaType; bus = [string]$pd.BusType;
        size = [int64]$v.Size; free = [int64]$v.SizeRemaining }
})
"#;

/// "NTFS DisableDeleteNotify = 0  (...)" -> Some(true) when TRIM is on.
pub fn parse_trim(out: &str) -> Option<bool> {
    out.lines().find_map(|l| {
        let rest = l.split("DisableDeleteNotify").nth(1)?;
        let digit = rest.trim_start_matches([' ', '=']).chars().next()?;
        l.contains("NTFS").then_some(digit == '0')
    })
}

pub fn check() -> Value {
    #[cfg(windows)]
    {
        let mut games = steam_games();
        games.extend(ubisoft_games());
        games.extend(epic_games());
        games.sort_by_key(|g| g.name.to_lowercase());
        games.truncate(300);
        let drives = match crate::ps::run_json(DRIVES_PS) {
            Ok(Value::Array(a)) => a,
            Ok(o @ Value::Object(_)) => vec![o],
            _ => vec![],
        };
        let trim = crate::ps::exec(
            &format!(r"{}\fsutil.exe", crate::ps::system_dir()),
            &["behavior", "query", "DisableDeleteNotify"],
        )
        .ok()
        .and_then(|o| parse_trim(&o));
        json!({
            "games": games.iter().map(|g| json!({
                "name": g.name, "launcher": g.launcher, "path": g.path,
                "drive": drive_letter(&g.path).map(String::from),
            })).collect::<Vec<_>>(),
            "drives": drives,
            "trimEnabled": trim,
        })
    }
    #[cfg(not(windows))]
    json!({ "games": [], "drives": [], "error": "Windows only" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_steam_library_files() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}";
        assert_eq!(
            parse_vdf_paths(vdf),
            vec![r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]
        );
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"359550\"\n\t\"name\"\t\t\"Tom Clancy's Rainbow Six Siege\"\n\t\"installdir\"\t\t\"Tom Clancy's Rainbow Six Siege\"\n}";
        assert_eq!(
            vdf_field(acf, "name").as_deref(),
            Some("Tom Clancy's Rainbow Six Siege")
        );
        assert_eq!(drive_letter(r"d:\Games"), Some('D'));
        assert_eq!(drive_letter(r"\\server\share"), None);
    }

    #[test]
    fn parses_trim_state() {
        assert_eq!(parse_trim("NTFS DisableDeleteNotify = 0  (Allows TRIM operations to be sent to the storage device)\nReFS DisableDeleteNotify = 0"), Some(true));
        assert_eq!(
            parse_trim("NTFS DisableDeleteNotify = 1  (Deaktiviert)"),
            Some(false)
        );
        assert_eq!(parse_trim("garbage"), None);
    }
}
