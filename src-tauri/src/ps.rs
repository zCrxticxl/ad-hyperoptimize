//! PowerShell / external command bridge. All WMI, service, powercfg and
//! diagnostic queries flow through here so output handling and window
//! suppression are uniform.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Upper bound for every external command, a hung powershell/WMI call must
/// not block a command worker forever. On timeout the process is killed.
const CMD_TIMEOUT: Duration = Duration::from_secs(30);

/// Upper bound for long-running operations (winget install/upgrade, WU driver
/// download+install, DISM/SFC checks). These legitimately take minutes; the
/// short timeout must not kill them mid-operation.
const CMD_TIMEOUT_LONG: Duration = Duration::from_secs(20 * 60);

#[cfg_attr(not(windows), allow(unused_variables))]
fn configure(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
        // The elevated process inherits the environment of whoever started
        // it. Children get a fixed one for every variable that decides which
        // program or module runs (PATH, PSModulePath, SystemRoot, ...), so a
        // tampered environment cannot make an admin child load foreign code.
        cmd.envs(trusted_env(&windows_dir()));
    }
}

/// Windows directory from the API, never from `%SystemRoot%`/`%windir%`.
#[cfg(windows)]
pub fn windows_dir() -> String {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetWindowsDirectoryW(buf: *mut u16, size: u32) -> u32;
    }
    static DIR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let mut buf = [0u16; 260];
        // SAFETY: the buffer length is passed; the return value is the number
        // of characters written (0 on failure, > len if too small).
        let n = unsafe { GetWindowsDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n == 0 || n >= buf.len() {
            r"C:\Windows".to_string()
        } else {
            String::from_utf16_lossy(&buf[..n])
        }
    })
    .clone()
}

#[cfg(not(windows))]
pub fn windows_dir() -> String {
    r"C:\Windows".to_string()
}

/// `<windows>\System32` (64-bit process: the native system directory).
pub fn system_dir() -> String {
    format!(r"{}\System32", windows_dir())
}

/// Absolute path of Windows PowerShell 5.1.
pub fn powershell_exe() -> String {
    format!(r"{}\WindowsPowerShell\v1.0\powershell.exe", system_dir())
}

/// The environment variables every child process gets, derived only from
/// the real Windows directory.
#[cfg_attr(not(windows), allow(dead_code))]
fn trusted_env(windows: &str) -> Vec<(&'static str, String)> {
    let sys = format!(r"{windows}\System32");
    let drive = windows.get(..2).unwrap_or("C:").to_string();
    vec![
        ("SystemRoot", windows.to_string()),
        ("windir", windows.to_string()),
        ("SystemDrive", drive),
        ("ComSpec", format!(r"{sys}\cmd.exe")),
        (
            "PATH",
            format!(r"{sys};{windows};{sys}\Wbem;{sys}\WindowsPowerShell\v1.0"),
        ),
        // Only the modules shipped with Windows; user and third-party module
        // folders are not searched.
        (
            "PSModulePath",
            format!(r"{sys}\WindowsPowerShell\v1.0\Modules"),
        ),
    ]
}

/// Wait for `child` until `timeout` elapses; kill it (with its whole process
/// tree on Windows) and return a timeout error when it does. stdout/stderr are
/// drained on side threads so a chatty child can never fill a pipe and
/// deadlock.
fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>), String> {
    let t_out = child.stdout.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = s.read_to_end(&mut v);
            v
        })
    });
    let t_err = child.stderr.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = s.read_to_end(&mut v);
            v
        })
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait(); // reap so no zombie is left behind
                return Err(format!("waiting for child failed: {e}"));
            }
        }
        if Instant::now() >= deadline {
            // Kill the whole tree, not just the parent: a killed powershell
            // leaves its children (winget, DISM, …) alive and holding the pipe
            // handles, which would leak the reader threads below. taskkill /T
            // terminates the tree so EOF arrives and every thread exits.
            let _ = kill_tree(child);
            let _ = child.wait();
            return Err(format!(
                "command timed out after {}s and was killed",
                timeout.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let out = t_out.and_then(|h| h.join().ok()).unwrap_or_default();
    let err = t_err.and_then(|h| h.join().ok()).unwrap_or_default();
    Ok((status, out, err))
}

/// Kill `child` and, on Windows, its entire descendant tree so no orphaned
/// grandchild survives a timeout.
fn kill_tree(child: &mut Child) -> std::io::Result<()> {
    let _ = child.kill();
    #[cfg(windows)]
    {
        // taskkill /T /F terminates the whole tree; child.id() is the PID.
        // configure() suppresses its console window (same "no flash" rule as
        // every other external command).
        let mut k = std::process::Command::new("taskkill");
        k.args(["/PID", &child.id().to_string(), "/T", "/F"]);
        configure(&mut k);
        let _ = k.stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    Ok(())
}

/// Forces UTF-8 stdout/stderr for every PowerShell child: localized Windows
/// uses legacy code pages (CP1252, CP936, ...), which would turn every
/// non-ASCII byte into U+FFFD in from_utf8_lossy decoding.
const PS_UTF8_PROLOGUE: &str = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; ";

/// Spawn `cmd` with piped output, enforce `timeout`, return (stdout, stderr).
fn run_checked(
    cmd: &mut Command,
    what: &str,
    timeout: Duration,
) -> Result<(String, String), String> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{what} spawn: {e}"))?;
    let (status, out, err) = wait_with_timeout(&mut child, timeout)?;
    if status.success() {
        Ok((
            String::from_utf8_lossy(&out).into_owned(),
            String::from_utf8_lossy(&err).into_owned(),
        ))
    } else {
        let err = String::from_utf8_lossy(&err).into_owned();
        Err(if err.trim().is_empty() {
            format!("{what} exited with {status}")
        } else {
            err
        })
    }
}

/// Run a PowerShell script, return stdout. Errors carry stderr.
pub fn run(script: &str) -> Result<String, String> {
    let mut cmd = Command::new(powershell_exe());
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &format!("{PS_UTF8_PROLOGUE}{script}"),
    ]);
    configure(&mut cmd);
    run_checked(&mut cmd, "powershell", CMD_TIMEOUT).map(|(out, _)| out)
}

/// Like [`run`], but the status of the script's last command does not
/// decide success. `powershell -Command` exits 1 whenever its last command
/// set `$?` to false, even under `-ErrorAction SilentlyContinue` (e.g.
/// removing a value that is already absent). Terminating errors still fail.
/// Callers must read the result back from Windows afterwards.
pub fn run_settled(script: &str) -> Result<String, String> {
    run(&settled(script))
}

fn settled(script: &str) -> String {
    format!("{script}\n$null = $null")
}

/// Like [`run`], but with the long timeout for operations that legitimately
/// take minutes (winget install/upgrade, WU driver install, DISM/SFC checks).
pub fn run_long(script: &str) -> Result<String, String> {
    let mut cmd = Command::new(powershell_exe());
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &format!("{PS_UTF8_PROLOGUE}{script}"),
    ]);
    configure(&mut cmd);
    run_checked(&mut cmd, "powershell", CMD_TIMEOUT_LONG).map(|(out, _)| out)
}

/// Run a PowerShell pipeline and parse `ConvertTo-Json` output.
/// The script is wrapped in `$( … )` so multi-line scripts (try/catch blocks,
/// loops) pipe their collected output into ConvertTo-Json without producing
/// an EmptyPipeElement parse error.
pub fn run_json(script: &str) -> Result<serde_json::Value, String> {
    run_json_with(script, CMD_TIMEOUT)
}

/// Like [`run_json`], but with the long timeout for minutes-long operations.
pub fn run_json_long(script: &str) -> Result<serde_json::Value, String> {
    run_json_with(script, CMD_TIMEOUT_LONG)
}

fn run_json_with(script: &str, timeout: Duration) -> Result<serde_json::Value, String> {
    // Force invariant culture: German/French locales emit "15,625" → invalid JSON.
    // The wrapper pipes through ConvertTo-Json for scripts that output bare PS objects.
    // Scripts that call ConvertTo-Json *themselves* get double-wrapped into a JSON string;
    // we detect that case and transparently unwrap it.
    let s = run_with(
        &format!(
            "[System.Threading.Thread]::CurrentThread.CurrentCulture = \
             [System.Globalization.CultureInfo]::InvariantCulture; \
             $ProgressPreference='SilentlyContinue'; $(\n{script}\n) | ConvertTo-Json -Depth 6 -Compress"
        ),
        timeout,
    )?;
    let t = s.trim();
    if t.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    let v = serde_json::from_str::<serde_json::Value>(t).map_err(|e| format!("json parse: {e}"))?;
    // If the inner script already called ConvertTo-Json, the outer wrapper re-encoded it
    // as a JSON string (e.g. `"{ ... }"`). Unwrap that extra layer.
    if let Some(inner_str) = v.as_str() {
        serde_json::from_str(inner_str).map_err(|e| format!("json parse inner: {e}"))
    } else {
        Ok(v)
    }
}

fn run_with(script: &str, timeout: Duration) -> Result<String, String> {
    let mut cmd = Command::new(powershell_exe());
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &format!("{PS_UTF8_PROLOGUE}{script}"),
    ]);
    configure(&mut cmd);
    run_checked(&mut cmd, "powershell", timeout).map(|(out, _)| out)
}

/// Run an arbitrary executable (reg.exe, powercfg, driverquery, schtasks...).
pub fn exec(exe: &str, args: &[&str]) -> Result<String, String> {
    exec_with(exe, args, CMD_TIMEOUT)
}

/// Like [`exec`], but with the long timeout for minutes-long operations.
pub fn exec_long(exe: &str, args: &[&str]) -> Result<String, String> {
    exec_with(exe, args, CMD_TIMEOUT_LONG)
}

fn exec_with(exe: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    configure(&mut cmd);
    run_checked(&mut cmd, exe, timeout).map(|(out, _)| out)
}

/// Run an executable with the long timeout, returning the exit status plus
/// stdout/stderr regardless of success. Used by callers that must inspect
/// output even on failure (winget/pnputil write errors to stdout, not
/// stderr). On timeout the whole process tree is killed.
pub fn exec_capture(
    exe: &str,
    args: &[&str],
) -> Result<(std::process::ExitStatus, String, String), String> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    configure(&mut cmd);
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{exe} spawn: {e}"))?;
    let (status, out, err) = wait_with_timeout(&mut child, CMD_TIMEOUT_LONG)?;
    Ok((
        status,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    ))
}

/// True when the process runs elevated.
pub fn is_admin() -> bool {
    run("([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)")
        .map(|s| s.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Validate an identifier/name/path that will be interpolated into a
/// PowerShell script. Rejects every character that has meaning inside PS
/// single or double quotes (`'`, `"`, `$`, backtick), plus control chars,
/// so a validated value is safe to embed in either quoting style. All other
/// Unicode (umlauts, CJK, …) is allowed, PS strings are UTF-16.
///
/// PowerShell also treats the typographic quotes U+2018..=U+201B as single
/// quotes and U+201C..=U+201E as double quotes, so the whole U+2018..=U+201F
/// block is rejected too (`'a’; calc; ‘b'` would otherwise break out of a
/// single-quoted literal).
pub fn is_safe_ident(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.chars().all(|c| {
            !c.is_control() && !matches!(c, '\'' | '"' | '$' | '`' | '\u{2018}'..='\u{201F}')
        })
}

/// True for every code point PowerShell's tokenizer accepts as a single-quote
/// delimiter: ASCII `'` plus U+2018, U+2019, U+201A and U+201B.
fn is_ps_single_quote(c: char) -> bool {
    matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}')
}

/// Quote `s` as a PowerShell single-quoted string literal. Inside a
/// single-quoted literal the only special characters are the quote
/// delimiters; each one (ASCII or typographic) is doubled, which PowerShell
/// reads back as that one literal character. Nothing inside a single-quoted
/// literal is expanded (`$`, backtick and `$(…)` stay inert).
pub fn quote_single(s: &str) -> String {
    format!("'{}'", escape_single(s))
}

/// The inside of a PowerShell single-quoted string: every character PowerShell
/// accepts as a single quote (ASCII and U+2018..U+201B) is doubled.
pub fn escape_single(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_ps_single_quote(c) {
            out.push(c);
        }
        out.push(c);
    }
    out
}

/// Like [`is_safe_ident`], but additionally rejects `\`/`/` so the value can
/// also be used as a single registry subkey name without escaping the key
/// hierarchy.
pub fn is_safe_regkey_name(s: &str) -> bool {
    is_safe_ident(s) && !s.contains(['\\', '/'])
}

/// Absolute path of winget.exe for elevated use. Bare `winget.exe` resolves
/// through PATH to the App Execution Alias in the user-writable
/// `%LOCALAPPDATA%\Microsoft\WindowsApps`, so the elevated process could be
/// made to run a planted binary. The DesktopAppInstaller package's own
/// install folder under `%ProgramFiles%\WindowsApps` is TrustedInstaller
/// owned. Resolved once per process; falls back to the bare name only when
/// the package location cannot be determined (winget then behaves exactly
/// as before).
pub fn winget_exe() -> String {
    static WINGET: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    WINGET
        .get_or_init(|| resolve_winget().unwrap_or_else(|| "winget.exe".into()))
        .clone()
}

#[cfg(windows)]
fn resolve_winget() -> Option<String> {
    let pf = std::env::var("ProgramFiles").ok()?;
    for all in ["", " -AllUsers"] {
        let Ok(out) = run(&format!(
            "(Get-AppxPackage{all} -Name Microsoft.DesktopAppInstaller -ErrorAction SilentlyContinue | \
             Sort-Object Version -Descending | Select-Object -First 1).InstallLocation"
        )) else {
            continue;
        };
        if let Some(exe) = winget_exe_in(out.trim(), &pf) {
            if std::path::Path::new(&exe).is_file() {
                return Some(exe);
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn resolve_winget() -> Option<String> {
    None
}

/// `<location>\winget.exe` when `location` is a package folder directly
/// below `<program_files>\WindowsApps` (single line, no traversal).
fn winget_exe_in(location: &str, program_files: &str) -> Option<String> {
    let loc = location.trim_end_matches('\\');
    let base = format!("{}\\WindowsApps\\", program_files.trim_end_matches('\\'));
    let pkg = loc
        .get(..base.len())
        .filter(|p| p.eq_ignore_ascii_case(&base))
        .map(|_| &loc[base.len()..])?;
    let valid = !program_files.is_empty()
        && !pkg.is_empty()
        && !pkg.contains(['\\', '/', ':'])
        && pkg != ".."
        && pkg != "."
        && !loc.chars().any(char::is_control);
    valid.then(|| format!("{loc}\\winget.exe"))
}

/// True for a canonical 8-4-4-4-12 GUID string. Used to extract GUIDs from
/// localized command output (powercfg /getactivescheme, …) where the header
/// text is language-dependent but the GUID itself is not.
pub fn is_guid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b[8] == b'-'
        && b[13] == b'-'
        && b[18] == b'-'
        && b[23] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, &c)| i == 8 || i == 13 || i == 18 || i == 23 || c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    #[test]
    fn settled_script_ends_with_a_succeeding_statement() {
        assert_eq!(
            super::settled("Remove-ItemProperty x y -EA SilentlyContinue"),
            "Remove-ItemProperty x y -EA SilentlyContinue\n$null = $null"
        );
    }

    use super::{is_safe_ident, is_safe_regkey_name, quote_single, trusted_env, winget_exe_in};

    #[test]
    fn children_get_an_environment_from_the_windows_dir_only() {
        let env = trusted_env(r"D:\WINDOWS");
        let get = |k: &str| {
            env.iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.clone())
                .unwrap()
        };
        assert_eq!(get("SystemRoot"), r"D:\WINDOWS");
        assert_eq!(get("windir"), r"D:\WINDOWS");
        assert_eq!(get("SystemDrive"), "D:");
        assert_eq!(get("ComSpec"), r"D:\WINDOWS\System32\cmd.exe");
        let path = get("PATH");
        assert!(path.starts_with(r"D:\WINDOWS\System32;"));
        assert!(
            path.split(';').all(|p| p.starts_with(r"D:\WINDOWS")),
            "{path}"
        );
        assert_eq!(
            get("PSModulePath"),
            r"D:\WINDOWS\System32\WindowsPowerShell\v1.0\Modules"
        );
        assert!(
            super::powershell_exe().ends_with(r"\System32\WindowsPowerShell\v1.0\powershell.exe")
        );
    }

    #[test]
    fn winget_location_must_be_a_windowsapps_package() {
        let pf = r"C:\Program Files";
        let loc = r"C:\Program Files\WindowsApps\Microsoft.DesktopAppInstaller_1.24.25200.0_x64__8wekyb3d8bbwe";
        assert_eq!(winget_exe_in(loc, pf), Some(format!(r"{loc}\winget.exe")));
        assert_eq!(
            winget_exe_in(&loc.to_lowercase(), pf),
            Some(format!(r"{}\winget.exe", loc.to_lowercase()))
        );
        for bad in [
            "",
            r"C:\Program Files\WindowsApps",
            r"C:\Program Files\WindowsApps\",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps\x",
            r"C:\Program Files\WindowsApps\..",
            r"C:\Program Files\WindowsApps\a\..\..\evil",
            r"C:\Program Files\WindowsAppsX\pkg",
            "C:\\Program Files\\WindowsApps\\pkg\nC:\\evil",
            r"D:\Program Files\WindowsApps\pkg",
        ] {
            assert_eq!(winget_exe_in(bad, pf), None, "should reject {bad:?}");
        }
        assert_eq!(winget_exe_in(loc, ""), None);
    }
    #[cfg(not(windows))]
    use super::{run_checked, wait_with_timeout};
    #[cfg(not(windows))]
    use std::process::{Command, Stdio};
    #[cfg(not(windows))]
    use std::time::Duration;

    #[test]
    fn safe_idents_are_accepted() {
        assert!(is_safe_ident("Spooler"));
        assert!(is_safe_ident("Ethernet 2"));
        assert!(is_safe_ident(r"\Microsoft\Windows\Defrag\ScheduledDefrag"));
        assert!(is_safe_ident("Discord.Discord"));
        assert!(is_safe_ident("{381b4222-f694-41f0-9685-ff5bb260df2e}"));
        assert!(is_safe_ident(
            r"HKLM:\SOFTWARE\Classes\*\shellex\ContextMenuHandlers\Sharing"
        ));
        assert!(is_safe_ident("game.exe"));
        // `;` and `|` are inert inside PS quotes; non-ASCII is valid UTF-16.
        assert!(is_safe_ident("foo; bar"));
        assert!(is_safe_ident("Mein Energiesparplan"));
        assert!(is_safe_ident("C:\\Benutzer\\Müller\\Seite.dat"));
        assert!(is_safe_ident("日本語の設定"));
    }

    #[test]
    fn ps_breakouts_are_rejected() {
        assert!(!is_safe_ident("foo'"));
        assert!(!is_safe_ident("foo' ; calc"));
        assert!(!is_safe_ident("foo\""));
        assert!(!is_safe_ident("$(calc)"));
        assert!(!is_safe_ident("foo`n"));
        assert!(!is_safe_ident(""));
        assert!(!is_safe_ident(&"x".repeat(257)));
        assert!(!is_safe_ident("foo\nbar"));
        assert!(!is_safe_ident("foo\rbar"));
    }

    #[test]
    fn unicode_quote_breakouts_are_rejected() {
        // PowerShell treats U+2018..U+201B as `'` and U+201C..U+201E as `"`.
        for q in ['\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}'] {
            assert!(!is_safe_ident(&format!(
                "a{q}; Write-Output INJECTED; {q}b"
            )));
            assert!(!is_safe_regkey_name(&format!("a{q}b")));
        }
        for q in ['\u{201C}', '\u{201D}', '\u{201E}', '\u{201F}'] {
            assert!(!is_safe_ident(&format!("a{q}b")));
            assert!(!is_safe_regkey_name(&format!("a{q}b")));
        }
        // Neighbouring typographic punctuation stays allowed.
        assert!(is_safe_ident("a\u{2014}b")); // em dash
        assert!(is_safe_ident("a\u{2020}b")); // dagger
    }

    #[test]
    fn quote_single_doubles_every_single_quote_form() {
        assert_eq!(quote_single("abc"), "'abc'");
        assert_eq!(quote_single(""), "''");
        assert_eq!(quote_single("it's"), "'it''s'");
        assert_eq!(quote_single("a\u{2019}b"), "'a\u{2019}\u{2019}b'");
        assert_eq!(
            quote_single("\u{2018}\u{2019}\u{201A}\u{201B}"),
            "'\u{2018}\u{2018}\u{2019}\u{2019}\u{201A}\u{201A}\u{201B}\u{201B}'"
        );
        // `$`, backtick and double quotes are inert inside single quotes.
        assert_eq!(quote_single("$(calc)`\""), "'$(calc)`\"'");
        // The verified breakout payload stays a single literal.
        assert_eq!(
            quote_single("a\u{2019}; Write-Output 'INJECTED'; \u{2018}b"),
            "'a\u{2019}\u{2019}; Write-Output ''INJECTED''; \u{2018}\u{2018}b'"
        );
    }

    #[test]
    fn regkey_names_reject_separators() {
        assert!(is_safe_regkey_name("chrome.exe"));
        assert!(is_safe_regkey_name("My App (x64).exe"));
        assert!(is_safe_regkey_name("WLAN-Treiber"));
        assert!(!is_safe_regkey_name(r"a\b"));
        assert!(!is_safe_regkey_name("a/b"));
        assert!(!is_safe_regkey_name(""));
    }

    // wait_with_timeout / run_checked are the heart of the PS bridge: output
    // must be captured (piped) and a hung child must be killed on timeout.
    // `sh` only exists on non-Windows dev hosts, the production powershell
    // path is verified on Windows; these guard the generic logic here.
    #[cfg(not(windows))]
    #[test]
    fn wait_with_timeout_captures_output() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo hello; echo oops >&2; exit 0"]);
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (status, out, err) = wait_with_timeout(&mut child, Duration::from_secs(10)).unwrap();
        assert!(status.success());
        assert_eq!(String::from_utf8_lossy(&out).trim(), "hello");
        assert_eq!(String::from_utf8_lossy(&err).trim(), "oops");
    }

    #[cfg(not(windows))]
    #[test]
    fn wait_with_timeout_kills_on_timeout() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30; echo done"]);
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let err = wait_with_timeout(&mut child, Duration::from_millis(300)).unwrap_err();
        assert!(err.contains("timed out"), "unexpected error: {err}");
    }

    #[cfg(not(windows))]
    #[test]
    fn run_checked_pipes_and_captures() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf captured; printf erred >&2; exit 0"]);
        let (out, err) = run_checked(&mut cmd, "sh", Duration::from_secs(10)).unwrap();
        assert_eq!(out, "captured");
        assert_eq!(err, "erred");
    }
}
