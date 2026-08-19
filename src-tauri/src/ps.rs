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
    cmd.creation_flags(CREATE_NO_WINDOW);
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
    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ]);
    configure(&mut cmd);
    run_checked(&mut cmd, "powershell", CMD_TIMEOUT).map(|(out, _)| out)
}

/// Like [`run`], but with the long timeout for operations that legitimately
/// take minutes (winget install/upgrade, WU driver install, DISM/SFC checks).
pub fn run_long(script: &str) -> Result<String, String> {
    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
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
    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
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
pub fn is_safe_ident(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.chars()
            .all(|c| !c.is_control() && !matches!(c, '\'' | '"' | '$' | '`'))
}

/// Like [`is_safe_ident`], but additionally rejects `\`/`/` so the value can
/// also be used as a single registry subkey name without escaping the key
/// hierarchy.
pub fn is_safe_regkey_name(s: &str) -> bool {
    is_safe_ident(s) && !s.contains(['\\', '/'])
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
    use super::{is_safe_ident, is_safe_regkey_name, run_checked, wait_with_timeout};
    use std::process::{Command, Stdio};
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
