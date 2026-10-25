//! Resizable BAR per game through the NVIDIA driver profile (DRS), the same
//! settings NVIDIA Profile Inspector edits. The driver only uses ReBAR for
//! games on NVIDIA's list; for others (e.g. Rainbow Six Siege) the profile
//! flags 0x000F00BA (enable) and 0x000F00BB (options) are 0 and can be set
//! to 1. The previous values are stored before anything is written, and the
//! undo restores them (or removes the app's value so the driver default
//! applies again).
//!
//! Function ids and struct layouts follow NVIDIA's published NVAPI headers
//! (github.com/NVIDIA/nvapi, nvapi_interface.h / nvapi.h, pack 8).

use serde_json::{json, Value};

pub const REBAR_ENABLE: u32 = 0x000F_00BA;
pub const REBAR_OPTIONS: u32 = 0x000F_00BB;
pub const REBAR_IDS: [u32; 2] = [REBAR_ENABLE, REBAR_OPTIONS];

/// Rainbow Six Siege executables (DX11, DX12 and the old Vulkan build).
pub const R6_EXES: &[&str] = &[
    "RainbowSix.exe",
    "RainbowSix_DX12.exe",
    "RainbowSix_Vulkan.exe",
];

/// A game executable name the user may target: a plain file name.
pub fn valid_exe(name: &str) -> bool {
    let n = name.trim();
    n.len() <= 64
        && n.to_ascii_lowercase().ends_with(".exe")
        && n.len() > 4
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || " _-.()".contains(c))
}

/// System-level ReBAR is on when BAR1 covers (nearly) the whole VRAM.
pub fn system_rebar_on(bar: Option<(u64, u64)>) -> Option<bool> {
    bar.map(|(bar1, fb)| bar1 * 10 >= fb * 9)
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Prev {
    /// The value lived in the game's own profile (user or predefined).
    pub in_profile: bool,
    pub value: u32,
}

fn backup_path() -> std::path::PathBuf {
    crate::safety::app_data_dir().join("nv_rebar_backup.json")
}

fn load_backup() -> serde_json::Map<String, Value> {
    std::fs::read_to_string(backup_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn save_backup(m: &serde_json::Map<String, Value>) -> Result<(), String> {
    std::fs::create_dir_all(crate::safety::app_data_dir()).map_err(|e| e.to_string())?;
    crate::safety::write_user_file(
        &backup_path(),
        Value::Object(m.clone()).to_string().as_bytes(),
    )
}

#[cfg(windows)]
mod drs {
    use super::Prev;
    use std::ffi::c_void;

    const ID_INITIALIZE: u32 = 0x0150_e828;
    const ID_CREATE_SESSION: u32 = 0x0694_d52e;
    const ID_DESTROY_SESSION: u32 = 0xdad9_cff8;
    const ID_LOAD_SETTINGS: u32 = 0x375d_bd6b;
    const ID_SAVE_SETTINGS: u32 = 0xfcbc_7e14;
    const ID_FIND_APP: u32 = 0xeee5_66b2;
    const ID_SET_SETTING: u32 = 0x577d_d202;
    const ID_GET_SETTING: u32 = 0x73bf_8338;
    const ID_DELETE_SETTING: u32 = 0xe4a2_6362;

    const OK: i32 = 0;
    const NO_NVIDIA: i32 = -6;
    const INCOMPATIBLE_VERSION: i32 = -9;
    const SETTING_NOT_FOUND: i32 = -160;
    const EXE_NOT_FOUND: i32 = -166;

    const UNICODE_MAX: usize = 2048;
    /// NVDRS_APPLICATION_V1: version, isPredefined, 3 × NvAPI_UnicodeString.
    const APP_V1_SIZE: usize = 8 + 3 * UNICODE_MAX * 2;
    /// NVDRS_SETTING_V1 offsets. Current headers add a NvU64 to the value
    /// unions, which aligns them to 8 (size 12328, current value at 8224);
    /// drivers built before that use 12320 / 8220. Both are tried.
    const OFF_ID: usize = 4100;
    const OFF_TYPE: usize = 4104;
    const OFF_LOCATION: usize = 4108;
    const LAYOUTS: [(usize, usize); 2] = [(12328, 8224), (12320, 8220)];
    const NVDRS_DWORD_TYPE: u32 = 0;
    const NVDRS_CURRENT_PROFILE_LOCATION: u32 = 0;

    type Handle = *mut c_void;
    type QueryInterface = unsafe extern "C" fn(u32) -> *const c_void;
    type FnInit = unsafe extern "C" fn() -> i32;
    type FnSession = unsafe extern "C" fn(*mut Handle) -> i32;
    type FnHandle = unsafe extern "C" fn(Handle) -> i32;
    type FnFindApp = unsafe extern "C" fn(Handle, *const u16, *mut Handle, *mut u8) -> i32;
    type FnGet = unsafe extern "C" fn(Handle, Handle, u32, *mut u8) -> i32;
    type FnSet = unsafe extern "C" fn(Handle, Handle, *mut u8) -> i32;
    type FnDelete = unsafe extern "C" fn(Handle, Handle, u32) -> i32;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const std::ffi::c_char) -> *const c_void;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub struct Drs {
        session: Handle,
        destroy: FnHandle,
        save: FnHandle,
        find_app: FnFindApp,
        get: FnGet,
        set: FnSet,
        delete: FnDelete,
        layout: std::cell::Cell<usize>,
    }

    fn err(rc: i32, what: &str) -> String {
        match rc {
            NO_NVIDIA => "No NVIDIA driver found.".into(),
            EXE_NOT_FOUND => "The NVIDIA driver has no profile for this game (start it once, or check the file name).".into(),
            _ => format!("NVIDIA driver call failed: {what} ({rc})"),
        }
    }

    impl Drs {
        pub fn open() -> Result<Self, String> {
            unsafe {
                let lib = LoadLibraryW(wide("nvapi64.dll").as_ptr());
                if lib.is_null() {
                    return Err("No NVIDIA driver found (nvapi64.dll).".into());
                }
                let qi = GetProcAddress(lib, c"nvapi_QueryInterface".as_ptr());
                if qi.is_null() {
                    return Err("nvapi64.dll has no nvapi_QueryInterface.".into());
                }
                let qi: QueryInterface = std::mem::transmute(qi);
                let f = |id: u32| -> Result<*const c_void, String> {
                    let p = qi(id);
                    if p.is_null() {
                        Err(format!(
                            "This NVIDIA driver does not provide function 0x{id:08x}."
                        ))
                    } else {
                        Ok(p)
                    }
                };
                let init: FnInit = std::mem::transmute(f(ID_INITIALIZE)?);
                let rc = init();
                if rc != OK {
                    return Err(err(rc, "initialize"));
                }
                let create: FnSession = std::mem::transmute(f(ID_CREATE_SESSION)?);
                let load: FnHandle = std::mem::transmute(f(ID_LOAD_SETTINGS)?);
                let destroy: FnHandle = std::mem::transmute(f(ID_DESTROY_SESSION)?);
                let save: FnHandle = std::mem::transmute(f(ID_SAVE_SETTINGS)?);
                let find_app: FnFindApp = std::mem::transmute(f(ID_FIND_APP)?);
                let get: FnGet = std::mem::transmute(f(ID_GET_SETTING)?);
                let set: FnSet = std::mem::transmute(f(ID_SET_SETTING)?);
                let delete: FnDelete = std::mem::transmute(f(ID_DELETE_SETTING)?);
                let mut session: Handle = std::ptr::null_mut();
                let rc = create(&mut session);
                if rc != OK {
                    return Err(err(rc, "create session"));
                }
                // From here the session is owned by `drs` and destroyed on drop.
                let drs = Drs {
                    session,
                    destroy,
                    save,
                    find_app,
                    get,
                    set,
                    delete,
                    layout: std::cell::Cell::new(0),
                };
                let rc = load(drs.session);
                if rc != OK {
                    return Err(err(rc, "load settings"));
                }
                Ok(drs)
            }
        }

        /// Profile handle of the profile that contains `exe`.
        pub fn profile_of(&self, exe: &str) -> Result<Handle, String> {
            let mut name = vec![0u16; UNICODE_MAX];
            for (i, c) in exe.encode_utf16().take(UNICODE_MAX - 1).enumerate() {
                name[i] = c;
            }
            let mut app = vec![0u8; APP_V1_SIZE];
            app[0..4].copy_from_slice(&((APP_V1_SIZE as u32) | (1 << 16)).to_le_bytes());
            let mut profile: Handle = std::ptr::null_mut();
            let rc = unsafe {
                (self.find_app)(self.session, name.as_ptr(), &mut profile, app.as_mut_ptr())
            };
            if rc != OK {
                return Err(err(rc, "find application"));
            }
            Ok(profile)
        }

        /// Runs `call` with a zeroed NVDRS_SETTING buffer in the current
        /// layout; on a version mismatch switches to the older layout once.
        fn with_setting<T>(&self, mut call: impl FnMut(&mut [u8], usize) -> (i32, T)) -> (i32, T) {
            loop {
                let (size, cur_off) = LAYOUTS[self.layout.get()];
                // u64 backing keeps the buffer 8-aligned like the C struct.
                let mut backing = vec![0u64; size.div_ceil(8)];
                let buf = unsafe {
                    std::slice::from_raw_parts_mut(backing.as_mut_ptr() as *mut u8, size)
                };
                buf[0..4].copy_from_slice(&((size as u32) | (1 << 16)).to_le_bytes());
                let (rc, out) = call(buf, cur_off);
                if rc == INCOMPATIBLE_VERSION && self.layout.get() + 1 < LAYOUTS.len() {
                    self.layout.set(self.layout.get() + 1);
                    continue;
                }
                return (rc, out);
            }
        }

        /// Current value of a DWORD setting and whether it is set in this
        /// profile; None when the setting does not exist for this profile.
        pub fn get(&self, profile: Handle, id: u32) -> Result<Option<Prev>, String> {
            let (rc, out) = self.with_setting(|buf, cur| {
                let rc = unsafe { (self.get)(self.session, profile, id, buf.as_mut_ptr()) };
                let read =
                    |o: usize| u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
                (rc, (read(OFF_LOCATION), read(OFF_TYPE), read(cur)))
            });
            match rc {
                OK if out.1 == NVDRS_DWORD_TYPE => Ok(Some(Prev {
                    in_profile: out.0 == NVDRS_CURRENT_PROFILE_LOCATION,
                    value: out.2,
                })),
                OK => Err(format!("Setting 0x{id:08x} is not a DWORD")),
                SETTING_NOT_FOUND => Ok(None),
                rc => Err(err(rc, "get setting")),
            }
        }

        pub fn set(&self, profile: Handle, id: u32, value: u32) -> Result<(), String> {
            let (rc, ()) = self.with_setting(|buf, cur| {
                buf[OFF_ID..OFF_ID + 4].copy_from_slice(&id.to_le_bytes());
                buf[OFF_TYPE..OFF_TYPE + 4].copy_from_slice(&NVDRS_DWORD_TYPE.to_le_bytes());
                buf[cur..cur + 4].copy_from_slice(&value.to_le_bytes());
                (
                    unsafe { (self.set)(self.session, profile, buf.as_mut_ptr()) },
                    (),
                )
            });
            if rc == OK {
                Ok(())
            } else {
                Err(err(rc, "set setting"))
            }
        }

        pub fn delete(&self, profile: Handle, id: u32) -> Result<(), String> {
            match unsafe { (self.delete)(self.session, profile, id) } {
                OK | SETTING_NOT_FOUND => Ok(()),
                rc => Err(err(rc, "delete setting")),
            }
        }

        pub fn save(&self) -> Result<(), String> {
            match unsafe { (self.save)(self.session) } {
                OK => Ok(()),
                rc => Err(err(rc, "save settings")),
            }
        }
    }

    impl Drop for Drs {
        fn drop(&mut self) {
            if !self.session.is_null() {
                unsafe { (self.destroy)(self.session) };
            }
        }
    }
}

/// Is ReBAR forced for this exe? (both flags 1)
fn forced(vals: &[Option<Prev>]) -> bool {
    vals.iter().all(|v| v.is_some_and(|p| p.value == 1))
}

#[cfg(windows)]
fn system_bar() -> Option<(u64, u64)> {
    let smi = format!(r"{}\nvidia-smi.exe", crate::ps::system_dir());
    let (_, out, _) = crate::ps::exec_capture(&smi, &["-q", "-d", "MEMORY"]).ok()?;
    crate::optimizer::parse_nvidia_bar(&out)
}

#[cfg(windows)]
pub fn status(extra: Option<String>) -> Value {
    let bar = system_bar();
    let backup = load_backup();
    let mut exes: Vec<String> = R6_EXES.iter().map(|s| s.to_string()).collect();
    if let Some(e) = extra.filter(|e| valid_exe(e)) {
        exes.push(e);
    }
    for k in backup.keys() {
        if !exes.iter().any(|e| e.eq_ignore_ascii_case(k)) {
            exes.push(k.clone());
        }
    }
    let drs = match drs::Drs::open() {
        Ok(d) => d,
        Err(e) => return json!({ "error": e, "systemRebar": system_rebar_on(bar) }),
    };
    let games: Vec<Value> = exes
        .iter()
        .map(|exe| match drs.profile_of(exe) {
            Err(e) => json!({ "exe": exe, "profile": false, "note": e }),
            Ok(p) => {
                let vals: Vec<Option<Prev>> = REBAR_IDS
                    .iter()
                    .map(|&id| drs.get(p, id).ok().flatten())
                    .collect();
                json!({
                    "exe": exe, "profile": true, "forced": forced(&vals),
                    "values": vals.iter().map(|v| v.map(|p| p.value)).collect::<Vec<_>>(),
                    "byApp": backup.contains_key(&exe.to_ascii_lowercase()),
                })
            }
        })
        .collect();
    json!({ "systemRebar": system_rebar_on(bar), "bar1Mib": bar.map(|b| b.0), "games": games })
}

/// Forces ReBAR on for `exe` (enable) or restores the stored previous values.
#[cfg(windows)]
pub fn set(exe: String, enable: bool) -> Result<Value, String> {
    if !valid_exe(&exe) {
        return Err("Invalid executable name".into());
    }
    let key = exe.to_ascii_lowercase();
    let mut backup = load_backup();
    let drs = drs::Drs::open()?;
    let profile = drs.profile_of(&exe)?;
    if enable {
        if system_rebar_on(system_bar()) == Some(false) {
            return Err("Resizable BAR is off in the BIOS (BAR1 is small). Enable \"Above 4G Decoding\" and \"Re-Size BAR Support\" (UEFI boot, CSM off) first.".into());
        }
        if backup.contains_key(&key) {
            return Err("Already forced by this app; undo it first.".into());
        }
        // Write-ahead: previous values are stored before anything changes.
        let prev: Vec<Option<Prev>> = REBAR_IDS
            .iter()
            .map(|&id| drs.get(profile, id))
            .collect::<Result<_, _>>()?;
        backup.insert(key.clone(), json!(prev));
        save_backup(&backup)?;
        let applied = REBAR_IDS
            .iter()
            .try_for_each(|&id| drs.set(profile, id, 1))
            .and_then(|_| drs.save());
        if let Err(e) = applied {
            backup.remove(&key);
            let _ = save_backup(&backup);
            return Err(format!("{e}; nothing was saved to the driver profile"));
        }
        let now: Vec<Option<Prev>> = REBAR_IDS
            .iter()
            .map(|&id| drs.get(profile, id).ok().flatten())
            .collect();
        if !forced(&now) {
            return Err("The driver did not keep the values; undo and try again.".into());
        }
        Ok(json!({ "ok": true, "forced": true }))
    } else {
        let prev: Vec<Option<Prev>> = backup
            .get(&key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or("No change of this app to undo for this game")?;
        for (&id, p) in REBAR_IDS.iter().zip(prev.iter()) {
            match p {
                Some(p) if p.in_profile => drs.set(profile, id, p.value)?,
                // Not in the game's own profile before: remove the app's value
                // so the global/default value applies again.
                _ => drs.delete(profile, id)?,
            }
        }
        drs.save()?;
        backup.remove(&key);
        save_backup(&backup)?;
        Ok(json!({ "ok": true, "forced": false }))
    }
}

#[cfg(not(windows))]
pub fn status(_extra: Option<String>) -> Value {
    json!({ "error": "Windows only" })
}

#[cfg(not(windows))]
pub fn set(_exe: String, _enable: bool) -> Result<Value, String> {
    Err("Windows only".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_names() {
        assert!(valid_exe("RainbowSix.exe"));
        assert!(valid_exe("Game (x64).exe"));
        assert!(!valid_exe("..\\evil.exe"));
        assert!(!valid_exe("a.dll"));
        assert!(!valid_exe(".exe"));
    }

    #[test]
    fn rebar_state() {
        assert_eq!(system_rebar_on(Some((16384, 16376))), Some(true));
        assert_eq!(system_rebar_on(Some((256, 16376))), Some(false));
        assert!(forced(&[
            Some(Prev {
                in_profile: true,
                value: 1
            }),
            Some(Prev {
                in_profile: true,
                value: 1
            })
        ]));
        assert!(!forced(&[
            Some(Prev {
                in_profile: true,
                value: 1
            }),
            None
        ]));
    }
}
