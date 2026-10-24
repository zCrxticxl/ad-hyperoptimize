//! Windows side of the broker: the named pipe, its access control, the
//! identity checks on both ends, and the elevated launch. Plain Win32 FFI so
//! no new dependency is needed.

use super::{encode, Args, Next, Request, Response, Session, MAX_LINE, PROTOCOL};
use std::ffi::c_void;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::io::FromRawHandle;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Handle = *mut c_void;
const INVALID_HANDLE: Handle = -1isize as Handle;

#[repr(C)]
struct SecurityAttributes {
    n_length: u32,
    security_descriptor: *mut c_void,
    inherit_handle: i32,
}

#[repr(C)]
struct ShellExecuteInfoW {
    cb_size: u32,
    f_mask: u32,
    hwnd: Handle,
    verb: *const u16,
    file: *const u16,
    parameters: *const u16,
    directory: *const u16,
    show: i32,
    inst_app: Handle,
    id_list: *mut c_void,
    class: *const u16,
    hkey_class: Handle,
    hot_key: u32,
    icon_or_monitor: Handle,
    process: Handle,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateNamedPipeW(
        name: *const u16,
        open_mode: u32,
        pipe_mode: u32,
        max_instances: u32,
        out_size: u32,
        in_size: u32,
        timeout: u32,
        sa: *const SecurityAttributes,
    ) -> Handle;
    fn ConnectNamedPipe(pipe: Handle, overlapped: *mut c_void) -> i32;
    fn GetNamedPipeClientProcessId(pipe: Handle, pid: *mut u32) -> i32;
    fn GetNamedPipeServerProcessId(pipe: Handle, pid: *mut u32) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *mut c_void,
        disposition: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn WaitNamedPipeW(name: *const u16, timeout: u32) -> i32;
    fn GetLastError() -> u32;
    fn LocalFree(mem: *mut c_void) -> *mut c_void;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        buf: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn CloseHandle(h: Handle) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn GetCurrentProcessId() -> u32;
    fn GetProcessId(process: Handle) -> u32;
}

#[link(name = "advapi32")]
extern "system" {
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        sddl: *const u16,
        revision: u32,
        sd: *mut *mut c_void,
        size: *mut u32,
    ) -> i32;
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(
        token: Handle,
        class: u32,
        buf: *mut c_void,
        len: u32,
        ret: *mut u32,
    ) -> i32;
    fn ConvertSidToStringSidW(sid: *mut c_void, out: *mut *mut u16) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteExW(info: *mut ShellExecuteInfoW) -> i32;
}

const PIPE_ACCESS_DUPLEX: u32 = 0x3;
const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x8;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const OPEN_EXISTING: u32 = 3;
const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;
const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
const ERROR_PIPE_CONNECTED: u32 = 535;
const TOKEN_QUERY: u32 = 0x8;
const TOKEN_USER_CLASS: u32 = 1;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const SEE_MASK_NOCLOSEPROCESS: u32 = 0x40;
const SW_HIDE: i32 = 0;

/// The broker exits when no request arrived for this long.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// The broker exits when its client does not connect in time.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn last_error(what: &str) -> String {
    // SAFETY: plain thread-local error query.
    let code = unsafe { GetLastError() };
    format!("{what} failed (Win32 error {code})")
}

/// String SID of the user running this process.
fn current_user_sid() -> Result<String, String> {
    // SAFETY: the token handle is closed below; the buffer is sized by the
    // first GetTokenInformation call and holds a TOKEN_USER, whose first field
    // is the SID pointer into the same buffer.
    unsafe {
        let mut token: Handle = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(last_error("OpenProcessToken"));
        }
        let mut len = 0u32;
        GetTokenInformation(token, TOKEN_USER_CLASS, std::ptr::null_mut(), 0, &mut len);
        let mut buf = vec![0usize; (len as usize).div_ceil(std::mem::size_of::<usize>())];
        let ok = GetTokenInformation(
            token,
            TOKEN_USER_CLASS,
            buf.as_mut_ptr().cast(),
            len,
            &mut len,
        );
        CloseHandle(token);
        if ok == 0 {
            return Err(last_error("GetTokenInformation"));
        }
        let sid = buf[0] as *mut c_void;
        let mut out: *mut u16 = std::ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut out) == 0 {
            return Err(last_error("ConvertSidToStringSid"));
        }
        let mut n = 0;
        while *out.add(n) != 0 {
            n += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(out, n));
        LocalFree(out.cast());
        Ok(s)
    }
}

/// Full image path of a process.
fn process_image(pid: u32) -> Result<String, String> {
    // SAFETY: the handle is checked and closed; the buffer length is passed in
    // and updated by the call.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return Err(last_error("OpenProcess"));
        }
        let mut buf = vec![0u16; 32768];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(h);
        if ok == 0 {
            return Err(last_error("QueryFullProcessImageName"));
        }
        Ok(String::from_utf16_lossy(&buf[..size as usize]))
    }
}

/// DACL: full access for the given user SID and SYSTEM only; protected so
/// nothing is inherited.
pub fn pipe_sddl(user_sid: &str) -> String {
    format!("D:P(A;;GA;;;{user_sid})(A;;GA;;;SY)")
}

/// Creates the only instance of the pipe; fails if the name already exists
/// (someone squatting on it).
fn create_pipe(name: &str) -> Result<Handle, String> {
    let sddl = wide(&pipe_sddl(&current_user_sid()?));
    // SAFETY: the security descriptor is freed after CreateNamedPipeW, which
    // copies it into the pipe object.
    unsafe {
        let mut sd: *mut c_void = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut sd,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(last_error("ConvertStringSecurityDescriptor"));
        }
        let sa = SecurityAttributes {
            n_length: std::mem::size_of::<SecurityAttributes>() as u32,
            security_descriptor: sd,
            inherit_handle: 0,
        };
        let name = wide(name);
        let h = CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_REJECT_REMOTE_CLIENTS,
            1,
            64 * 1024,
            64 * 1024,
            0,
            &sa,
        );
        LocalFree(sd);
        if h == INVALID_HANDLE {
            return Err(last_error("CreateNamedPipe"));
        }
        Ok(h)
    }
}

/// Whether the connected client is the expected process running the same
/// executable as the broker.
fn verify_client(pipe: Handle, expected_pid: u32) -> Result<(), String> {
    let mut pid = 0u32;
    // SAFETY: valid pipe handle, out pointer to a local.
    if unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) } == 0 {
        return Err(last_error("GetNamedPipeClientProcessId"));
    }
    if pid != expected_pid {
        return Err(format!("unexpected client process {pid}"));
    }
    // SAFETY: plain query.
    let own = process_image(unsafe { GetCurrentProcessId() })?;
    let theirs = process_image(pid)?;
    if !own.eq_ignore_ascii_case(&theirs) {
        return Err("client runs a different executable".into());
    }
    Ok(())
}

/// Serves one client on `name` and returns when it disconnects, breaks the
/// protocol or stays idle. Exposed with an explicit client PID for tests.
pub fn serve_on(name: &str, client_pid: u32, nonce: &str) -> Result<(), String> {
    let pipe = create_pipe(name)?;
    // Give up if nobody connects: ConnectNamedPipe blocks indefinitely.
    let connected = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let connected = connected.clone();
        std::thread::spawn(move || {
            std::thread::sleep(CONNECT_TIMEOUT);
            if !connected.load(Ordering::SeqCst) {
                std::process::exit(3);
            }
        });
    }
    // SAFETY: valid pipe handle; synchronous connect.
    let ok = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) };
    // SAFETY: plain thread-local error query.
    if ok == 0 && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
        // SAFETY: we own the handle.
        unsafe { CloseHandle(pipe) };
        return Err(last_error("ConnectNamedPipe"));
    }
    connected.store(true, Ordering::SeqCst);
    // SAFETY: we own the handle; File closes it.
    let file = unsafe { File::from_raw_handle(pipe) };
    verify_client(pipe, client_pid)?;

    let last = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    {
        let last = last.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(30));
            let idle = start.elapsed().as_secs() - last.load(Ordering::SeqCst);
            if idle >= IDLE_TIMEOUT.as_secs() {
                std::process::exit(0);
            }
        });
    }

    let mut writer = file.try_clone().map_err(|e| format!("pipe clone: {e}"))?;
    let mut reader = BufReader::new(file);
    let mut session = Session::new(nonce);
    let mut line = String::new();
    loop {
        line.clear();
        let n = (&mut reader)
            .take(MAX_LINE as u64 + 1)
            .read_line(&mut line)
            .map_err(|e| format!("pipe read: {e}"))?;
        if n == 0 {
            return Ok(()); // client closed the pipe
        }
        last.store(start.elapsed().as_secs(), Ordering::SeqCst);
        let (resp, next) = session.handle_line(&line);
        writer
            .write_all(encode(&resp).as_bytes())
            .map_err(|e| format!("pipe write: {e}"))?;
        if next == Next::Close {
            return Ok(());
        }
    }
}

/// Entry point of `--broker`: serves the UI that launched it, then exits.
pub fn serve(args: &Args) -> i32 {
    match serve_on(&args.pipe, args.ui_pid, &args.nonce) {
        Ok(()) => 0,
        Err(_) => 2,
    }
}

/// UI side of the connection.
pub struct Client {
    reader: BufReader<File>,
    writer: File,
}

impl Client {
    /// Connects to `name`, checks that the server is `server_pid` (the
    /// process this UI launched) and authenticates with `nonce`.
    pub fn connect(
        name: &str,
        server_pid: u32,
        nonce: &str,
        wait: Duration,
    ) -> Result<Self, String> {
        let wname = wide(name);
        let deadline = Instant::now() + wait;
        let handle = loop {
            // SAFETY: valid wide string; identification-level impersonation
            // only, so the server cannot act with this client's token.
            let h = unsafe {
                CreateFileW(
                    wname.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    std::ptr::null_mut(),
                )
            };
            if h != INVALID_HANDLE {
                break h;
            }
            if Instant::now() >= deadline {
                return Err(last_error("connecting to the broker"));
            }
            // SAFETY: valid wide string.
            unsafe { WaitNamedPipeW(wname.as_ptr(), 200) };
            std::thread::sleep(Duration::from_millis(50));
        };
        // SAFETY: we own the handle; File closes it.
        let file = unsafe { File::from_raw_handle(handle) };
        let mut pid = 0u32;
        // SAFETY: valid pipe handle, out pointer to a local.
        if unsafe { GetNamedPipeServerProcessId(handle, &mut pid) } == 0 {
            return Err(last_error("GetNamedPipeServerProcessId"));
        }
        if pid != server_pid {
            return Err(format!("pipe is served by process {pid}, not the broker"));
        }
        let writer = file.try_clone().map_err(|e| format!("pipe clone: {e}"))?;
        let mut client = Self {
            reader: BufReader::new(file),
            writer,
        };
        client.request(&Request::Hello {
            protocol: PROTOCOL,
            nonce: nonce.to_string(),
        })?;
        Ok(client)
    }

    pub fn request(&mut self, req: &Request) -> Result<serde_json::Value, String> {
        self.writer
            .write_all(encode(req).as_bytes())
            .map_err(|e| format!("broker write: {e}"))?;
        let mut line = String::new();
        let n = (&mut self.reader)
            .take(MAX_LINE as u64 + 1)
            .read_line(&mut line)
            .map_err(|e| format!("broker read: {e}"))?;
        if n == 0 {
            return Err("the broker closed the connection".into());
        }
        let resp: Response =
            serde_json::from_str(line.trim_end()).map_err(|e| format!("broker reply: {e}"))?;
        if resp.ok {
            Ok(resp.value)
        } else {
            Err(resp.error.unwrap_or_else(|| "broker error".into()))
        }
    }
}

/// Starts this executable elevated as broker (UAC prompt) and connects.
pub fn launch_and_connect() -> Result<Client, String> {
    let exe = std::env::current_exe().map_err(|e| format!("own path: {e}"))?;
    let (pipe, nonce) = super::new_launch_ids();
    // SAFETY: plain query.
    let ui_pid = unsafe { GetCurrentProcessId() };
    let params = wide(&format!("--broker {pipe} {ui_pid} {nonce}"));
    let file = wide(&exe.to_string_lossy());
    let verb = wide("runas");
    let mut info = ShellExecuteInfoW {
        cb_size: std::mem::size_of::<ShellExecuteInfoW>() as u32,
        f_mask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: std::ptr::null_mut(),
        verb: verb.as_ptr(),
        file: file.as_ptr(),
        parameters: params.as_ptr(),
        directory: std::ptr::null(),
        show: SW_HIDE,
        inst_app: std::ptr::null_mut(),
        id_list: std::ptr::null_mut(),
        class: std::ptr::null(),
        hkey_class: std::ptr::null_mut(),
        hot_key: 0,
        icon_or_monitor: std::ptr::null_mut(),
        process: std::ptr::null_mut(),
    };
    // SAFETY: all string pointers outlive the call; the process handle is
    // closed after its PID is read.
    let server_pid = unsafe {
        if ShellExecuteExW(&mut info) == 0 {
            return Err(last_error("starting the broker (UAC declined?)"));
        }
        if info.process.is_null() {
            return Err("the broker did not start".into());
        }
        let pid = GetProcessId(info.process);
        CloseHandle(info.process);
        pid
    };
    Client::connect(&pipe, server_pid, &nonce, Duration::from_secs(30))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own_pid() -> u32 {
        // SAFETY: plain query.
        unsafe { GetCurrentProcessId() }
    }

    #[test]
    fn dacl_allows_only_the_user_and_system() {
        let sid = current_user_sid().unwrap();
        assert!(sid.starts_with("S-1-"), "{sid}");
        let sddl = pipe_sddl(&sid);
        assert!(sddl.starts_with("D:P"), "protected DACL");
        assert!(!sddl.contains(";;;WD)") && !sddl.contains(";;;BU)") && !sddl.contains(";;;AU)"));
    }

    #[test]
    fn pipe_round_trip_with_identity_checks() {
        let (pipe, nonce) = crate::broker::new_launch_ids();
        let (p, n) = (pipe.clone(), nonce.clone());
        let server = std::thread::spawn(move || serve_on(&p, own_pid(), &n));
        let mut client =
            Client::connect(&pipe, own_pid(), &nonce, Duration::from_secs(10)).unwrap();
        assert_eq!(
            client.request(&Request::Ping {}).unwrap(),
            serde_json::json!("pong")
        );
        drop(client);
        assert!(server.join().unwrap().is_ok());
    }

    #[test]
    fn a_second_server_cannot_take_the_name() {
        let (pipe, nonce) = crate::broker::new_launch_ids();
        let (p, n) = (pipe.clone(), nonce.clone());
        let server = std::thread::spawn(move || serve_on(&p, own_pid(), &n));
        let client = Client::connect(&pipe, own_pid(), &nonce, Duration::from_secs(10)).unwrap();
        assert!(create_pipe(&pipe).is_err(), "squatting must fail");
        drop(client);
        server.join().unwrap().unwrap();
    }

    #[test]
    fn unexpected_client_is_rejected() {
        let (pipe, nonce) = crate::broker::new_launch_ids();
        let (p, n) = (pipe.clone(), nonce.clone());
        // The server expects another PID than ours.
        let server = std::thread::spawn(move || serve_on(&p, own_pid() + 1, &n));
        let result = Client::connect(&pipe, own_pid(), &nonce, Duration::from_secs(10));
        assert!(result.is_err());
        assert!(server.join().unwrap().is_err());
    }

    #[test]
    fn wrong_nonce_is_rejected() {
        let (pipe, nonce) = crate::broker::new_launch_ids();
        let (p, n) = (pipe.clone(), nonce.clone());
        let server = std::thread::spawn(move || serve_on(&p, own_pid(), &n));
        let wrong = "0".repeat(64);
        assert!(Client::connect(&pipe, own_pid(), &wrong, Duration::from_secs(10)).is_err());
        assert!(server.join().unwrap().is_ok());
    }

    #[test]
    fn client_rejects_a_foreign_server() {
        let (pipe, nonce) = crate::broker::new_launch_ids();
        let (p, n) = (pipe.clone(), nonce.clone());
        let server = std::thread::spawn(move || serve_on(&p, own_pid(), &n));
        // We expect the broker to be another process than the one serving.
        assert!(Client::connect(&pipe, own_pid() + 1, &nonce, Duration::from_secs(10)).is_err());
        let _ = server.join();
    }
}
