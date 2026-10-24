//! Elevated broker, phase 2 of docs/audit-2026-10/elevation-design.md.
//!
//! The same executable started as `--broker <pipe> <ui-pid> <nonce>` runs
//! without a window, creates a named pipe only the signed-in user and SYSTEM
//! may open, accepts exactly one client (the UI process that started it,
//! checked by PID and image path) and answers newline-delimited JSON
//! requests. Phase 2 only knows `hello` and `ping`; privileged requests are
//! added in phase 3. Nothing uses the broker at runtime unless the `broker`
//! cargo feature is enabled.
// Phase 2 builds the pieces; the UI client is wired up in phase 3. Until
// then parts are unused in every build configuration.
#![allow(dead_code)]

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
    Hello { protocol: u32, nonce: String },
    Ping {},
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
}

/// What the connection does after answering.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    Continue,
    Close,
}

impl Session {
    pub fn new(nonce: &str) -> Self {
        Self {
            nonce: nonce.to_string(),
            authenticated: false,
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
