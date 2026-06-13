//! PowerShell / external command bridge. All WMI, service, powercfg and
//! diagnostic queries flow through here so output handling and window
//! suppression are uniform.

use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn configure(cmd: &mut Command) {
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
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
    let out = cmd.output().map_err(|e| format!("powershell spawn: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        Err(if err.trim().is_empty() {
            format!("powershell exited with {}", out.status)
        } else {
            err
        })
    }
}

/// Run a PowerShell pipeline and parse `ConvertTo-Json` output.
/// The script is wrapped in `$( … )` so multi-line scripts (try/catch blocks,
/// loops) pipe their collected output into ConvertTo-Json without producing
/// an EmptyPipeElement parse error.
pub fn run_json(script: &str) -> Result<serde_json::Value, String> {
    // Force invariant culture: German/French locales emit "15,625" → invalid JSON.
    // The wrapper pipes through ConvertTo-Json for scripts that output bare PS objects.
    // Scripts that call ConvertTo-Json *themselves* get double-wrapped into a JSON string;
    // we detect that case and transparently unwrap it.
    let s = run(&format!(
        "[System.Threading.Thread]::CurrentThread.CurrentCulture = \
         [System.Globalization.CultureInfo]::InvariantCulture; \
         $ProgressPreference='SilentlyContinue'; $(\n{script}\n) | ConvertTo-Json -Depth 6 -Compress"
    ))?;
    let t = s.trim();
    if t.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    let v = serde_json::from_str::<serde_json::Value>(t)
        .map_err(|e| format!("json parse: {e}"))?;
    // If the inner script already called ConvertTo-Json, the outer wrapper re-encoded it
    // as a JSON string (e.g. `"{ ... }"`). Unwrap that extra layer.
    if let Some(inner_str) = v.as_str() {
        serde_json::from_str(inner_str).map_err(|e| format!("json parse inner: {e}"))
    } else {
        Ok(v)
    }
}

/// Run an arbitrary executable (reg.exe, powercfg, driverquery, schtasks...).
pub fn exec(exe: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    configure(&mut cmd);
    let out = cmd.output().map_err(|e| format!("{exe} spawn: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

/// True when the process runs elevated.
pub fn is_admin() -> bool {
    run("([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)")
        .map(|s| s.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
