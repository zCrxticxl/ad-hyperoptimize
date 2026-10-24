//! Elevated broker, phase 2 of docs/audit-2026-10/elevation-design.md.
//!
//! The same executable started as `--broker <pipe> <ui-pid> <nonce>` runs
//! without a window, creates a named pipe only the signed-in user and SYSTEM
//! may open, accepts exactly one client (the UI process that started it,
//! checked by PID and image path) and answers newline-delimited JSON
//! requests: `hello`, `ping` and, since phase 3, applying and reverting
//! catalog tweaks by id. Nothing uses the broker at runtime unless the
//! `broker` cargo feature is enabled.
// Without the `broker` feature the UI never connects, so the client side is
// compiled (and tested) but unused.
#![cfg_attr(not(feature = "broker"), allow(dead_code))]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(windows)]
pub mod pipe;

pub const PROTOCOL: u32 = 1;
/// Longest accepted request or response line.
pub const MAX_LINE: usize = 1 << 20;
const PIPE_PREFIX: &str = r"\\.\pipe\adho-broker-";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    Hello {
        protocol: u32,
        nonce: String,
    },
    Ping {},
    /// Catalog tweak by id (tweaks.rs); the broker runs the journaled engine.
    ApplyTweak {
        id: String,
    },
    RevertTweak {
        id: String,
    },
}

/// Tweak ids are short snake_case catalog keys; anything else is rejected
/// before it reaches the engine.
pub fn valid_tweak_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Runs an authenticated, non-handshake request (privileged work).
pub type Executor = fn(&Request) -> Result<Value, String>;

/// The broker's executor: the same engine functions the UI used to call.
pub fn execute(req: &Request) -> Result<Value, String> {
    match req {
        Request::ApplyTweak { id } if valid_tweak_id(id) => crate::tweaks::apply(id),
        Request::RevertTweak { id } if valid_tweak_id(id) => crate::tweaks::revert(id),
        Request::ApplyTweak { .. } | Request::RevertTweak { .. } => Err("invalid tweak id".into()),
        Request::Hello { .. } | Request::Ping {} => Err("not an executable request".into()),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub error: Option<String>,
}

impl Response {
    fn ok(value: Value) -> Self {
        Self {
            ok: true,
            value,
            error: None,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            value: Value::Null,
            error: Some(msg.into()),
        }
    }
}

/// Startup arguments of the broker process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub pipe: String,
    pub ui_pid: u32,
    pub nonce: String,
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `--broker <pipe> <ui-pid> <nonce>` anywhere after the program name; any
/// malformed value means "not a broker launch".
pub fn args_from<I: IntoIterator<Item = String>>(args: I) -> Option<Args> {
    let args: Vec<String> = args.into_iter().collect();
    let at = args.iter().position(|a| a == "--broker")?;
    let pipe = args.get(at + 1)?.clone();
    let ui_pid: u32 = args.get(at + 2)?.parse().ok()?;
    let nonce = args.get(at + 3)?.clone();
    let suffix = pipe.strip_prefix(PIPE_PREFIX)?;
    if !is_hex(suffix, 32) || !is_hex(&nonce, 64) || ui_pid == 0 {
        return None;
    }
    Some(Args {
        pipe,
        ui_pid,
        nonce,
    })
}

/// A fresh pipe name and nonce for one broker launch.
pub fn new_launch_ids() -> (String, String) {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut name = [0u8; 16];
    let mut nonce = [0u8; 32];
    rng.fill_bytes(&mut name);
    rng.fill_bytes(&mut nonce);
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    (format!("{PIPE_PREFIX}{}", hex(&name)), hex(&nonce))
}

pub fn decode_request(line: &str) -> Result<Request, String> {
    if line.len() > MAX_LINE {
        return Err("request too long".into());
    }
    serde_json::from_str(line.trim_end()).map_err(|e| format!("invalid request: {e}"))
}

pub fn encode<T: Serialize>(msg: &T) -> String {
    let mut s = serde_json::to_string(msg).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

/// Per-connection state: the first request must be a matching `hello`.
pub struct Session {
    nonce: String,
    authenticated: bool,
    exec: Executor,
}

/// What the connection does after answering.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    Continue,
    Close,
}

impl Session {
    pub fn new(nonce: &str) -> Self {
        Self::with_executor(nonce, execute)
    }

    pub fn with_executor(nonce: &str, exec: Executor) -> Self {
        Self {
            nonce: nonce.to_string(),
            authenticated: false,
            exec,
        }
    }

    pub fn handle_line(&mut self, line: &str) -> (Response, Next) {
        let req = match decode_request(line) {
            Ok(r) => r,
            Err(e) => return (Response::err(e), Next::Close),
        };
        match (self.authenticated, req) {
            (false, Request::Hello { protocol, nonce }) => {
                if protocol != PROTOCOL {
                    return (
                        Response::err(format!(
                            "protocol {protocol} not supported (broker speaks {PROTOCOL})"
                        )),
                        Next::Close,
                    );
                }
                if !constant_time_eq(nonce.as_bytes(), self.nonce.as_bytes()) {
                    return (Response::err("authentication failed"), Next::Close);
                }
                self.authenticated = true;
                (
                    Response::ok(json!({ "protocol": PROTOCOL })),
                    Next::Continue,
                )
            }
            (false, _) => (Response::err("hello required first"), Next::Close),
            (true, Request::Hello { .. }) => (Response::err("already authenticated"), Next::Close),
            (true, Request::Ping {}) => (Response::ok(json!("pong")), Next::Continue),
            (true, req) => match (self.exec)(&req) {
                Ok(v) => (Response::ok(v), Next::Continue),
                // An engine error is an answer, not a protocol violation.
                Err(e) => (Response::err(e), Next::Continue),
            },
        }
    }
}

// ── UI side: the connection to the broker ──────────────────────────────────

#[cfg(windows)]
static CLIENT: std::sync::Mutex<Option<pipe::Client>> = std::sync::Mutex::new(None);

/// Whether this build can use a broker at all.
pub const AVAILABLE: bool = cfg!(all(windows, feature = "broker"));

/// Whether an authenticated broker connection is open.
pub fn connected() -> bool {
    #[cfg(windows)]
    {
        CLIENT.lock().map(|c| c.is_some()).unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Starts the broker (UAC prompt) and connects; a no-op when connected.
pub fn unlock() -> Result<Value, String> {
    #[cfg(all(windows, feature = "broker"))]
    {
        let mut guard = CLIENT
            .lock()
            .map_err(|_| "broker state poisoned".to_string())?;
        if guard.is_none() {
            *guard = Some(pipe::launch_and_connect()?);
        }
        Ok(json!({ "connected": true }))
    }
    #[cfg(not(all(windows, feature = "broker")))]
    {
        Err("This build has no admin helper (built without the broker feature).".into())
    }
}

/// Sends `req` to the broker when connected. `None` means "no broker": the
/// caller runs the work in-process exactly as before. A broken connection
/// (broker idle timeout, crash) is dropped and reported, never retried
/// in-process, because the UI may not have the rights.
pub fn route(req: Request) -> Option<Result<Value, String>> {
    #[cfg(windows)]
    {
        let mut guard = CLIENT.lock().ok()?;
        let client = guard.as_mut()?;
        let result = client.request(&req);
        if let Err(e) = &result {
            if e.starts_with("broker ") || e.starts_with("the broker") {
                *guard = None;
                return Some(Err(format!(
                    "The admin session ended ({e}). Unlock admin actions again."
                )));
            }
        }
        Some(result)
    }
    #[cfg(not(windows))]
    {
        let _ = req;
        None
    }
}

/// `--broker-selftest`: unlock, ping, print the result. Exit code 0 = ok.
#[cfg(windows)]
pub fn selftest() -> i32 {
    let result = unlock().and_then(|_| match route(Request::Ping {}) {
        Some(r) => r,
        None => Err("not connected".into()),
    });
    match result {
        Ok(v) if v == json!("pong") => {
            println!("broker selftest: ok (helper started, identity checked, ping answered)");
            0
        }
        Ok(v) => {
            println!("broker selftest: unexpected reply {v}");
            1
        }
        Err(e) => {
            println!("broker selftest: failed: {e}");
            1
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn broker_args_are_strict() {
        let (pipe, nonce) = new_launch_ids();
        let ok = args_from(args(&["app.exe", "--broker", &pipe, "1234", &nonce])).unwrap();
        assert_eq!(ok.ui_pid, 1234);
        assert_eq!(ok.pipe, pipe);
        assert!(args_from(args(&["app.exe"])).is_none());
        assert!(args_from(args(&[
            "app.exe",
            "--broker",
            r"\\.\pipe\other",
            "1",
            &nonce
        ]))
        .is_none());
        assert!(args_from(args(&["app.exe", "--broker", &pipe, "0", &nonce])).is_none());
        assert!(args_from(args(&["app.exe", "--broker", &pipe, "x", &nonce])).is_none());
        assert!(args_from(args(&["app.exe", "--broker", &pipe, "1", "short"])).is_none());
        assert!(args_from(args(&["app.exe", "--broker", &pipe, "1"])).is_none());
    }

    #[test]
    fn launch_ids_are_unique_and_well_formed() {
        let (a, n1) = new_launch_ids();
        let (b, n2) = new_launch_ids();
        assert_ne!(a, b);
        assert_ne!(n1, n2);
        assert!(args_from(args(&["x", "--broker", &a, "5", &n1])).is_some());
    }

    #[test]
    fn requests_are_typed_and_bounded() {
        assert_eq!(
            decode_request(r#"{"op":"ping"}"#).unwrap(),
            Request::Ping {}
        );
        assert!(decode_request(r#"{"op":"ping","extra":1}"#).is_err());
        assert!(decode_request(r#"{"op":"runScript","script":"calc"}"#).is_err());
        assert!(decode_request("not json").is_err());
        assert!(decode_request(&"x".repeat(MAX_LINE + 1)).is_err());
    }

    #[test]
    fn tweak_ids_are_validated_before_the_engine() {
        assert!(valid_tweak_id("game_mode_on"));
        assert!(!valid_tweak_id(""));
        assert!(!valid_tweak_id("Game_Mode"));
        assert!(!valid_tweak_id("x; calc"));
        assert!(!valid_tweak_id("../journal"));
        assert!(!valid_tweak_id(&"a".repeat(65)));
        assert_eq!(
            execute(&Request::ApplyTweak {
                id: "x;calc".into()
            }),
            Err("invalid tweak id".to_string())
        );
        assert!(decode_request(r#"{"op":"applyTweak","id":"game_mode_on"}"#).is_ok());
        assert!(decode_request(r#"{"op":"applyTweak","id":"a","script":"x"}"#).is_err());
    }

    #[test]
    fn authenticated_requests_reach_the_executor_and_errors_keep_the_session() {
        fn fake(req: &Request) -> Result<Value, String> {
            match req {
                Request::ApplyTweak { id } if id == "ok_id" => Ok(json!({ "status": "applied" })),
                _ => Err("engine refused".into()),
            }
        }
        let nonce = "ef".repeat(32);
        let mut s = Session::with_executor(&nonce, fake);
        // Not before hello.
        let (r, next) = s.handle_line(&encode(&Request::ApplyTweak { id: "ok_id".into() }));
        assert!(!r.ok);
        assert_eq!(next, Next::Close);

        let mut s = Session::with_executor(&nonce, fake);
        s.handle_line(&encode(&Request::Hello {
            protocol: PROTOCOL,
            nonce: nonce.clone(),
        }));
        let (r, next) = s.handle_line(&encode(&Request::ApplyTweak { id: "ok_id".into() }));
        assert!(r.ok);
        assert_eq!(r.value["status"], "applied");
        assert_eq!(next, Next::Continue);
        let (r, next) = s.handle_line(&encode(&Request::RevertTweak { id: "other".into() }));
        assert_eq!(r.error.as_deref(), Some("engine refused"));
        assert_eq!(next, Next::Continue);
    }

    #[test]
    fn without_a_connection_work_stays_in_process() {
        assert!(!connected());
        assert!(route(Request::Ping {}).is_none());
    }

    #[test]
    fn session_requires_hello_with_the_right_nonce() {
        let nonce = "ab".repeat(32);
        let hello = |n: &str, p: u32| {
            encode(&Request::Hello {
                protocol: p,
                nonce: n.into(),
            })
        };

        let mut s = Session::new(&nonce);
        let (r, next) = s.handle_line(&encode(&Request::Ping {}));
        assert!(!r.ok);
        assert_eq!(next, Next::Close);

        let mut s = Session::new(&nonce);
        let (r, next) = s.handle_line(&hello(&"cd".repeat(32), PROTOCOL));
        assert!(!r.ok);
        assert_eq!(next, Next::Close);

        let mut s = Session::new(&nonce);
        assert_eq!(s.handle_line(&hello(&nonce, PROTOCOL + 1)).1, Next::Close);

        let mut s = Session::new(&nonce);
        let (r, next) = s.handle_line(&hello(&nonce, PROTOCOL));
        assert!(r.ok);
        assert_eq!(next, Next::Continue);
        let (r, _) = s.handle_line(&encode(&Request::Ping {}));
        assert_eq!(r.value, json!("pong"));
        // A second hello is a protocol violation.
        assert_eq!(s.handle_line(&hello(&nonce, PROTOCOL)).1, Next::Close);
    }
}
