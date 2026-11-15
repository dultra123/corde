//! `--first-pairing-code` end to end, with the real binary (docs/SECURITY-MODEL.md §7):
//! no device paired → a framed administrator code on the standard output, usable once
//! through `POST /api/hub/pair`; a device paired → nothing printed; option absent →
//! nothing printed and no code issued. Also `corde-server healthcheck` and a clean stop
//! on SIGTERM.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_corde-server");

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-hub-first-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

struct Hub {
    child: Child,
    /// Standard output up to the « Ctrl-C » line (banner, and the block when there is one).
    out: String,
    port: u16,
}

fn start(data: &Path, first_code: bool) -> Hub {
    let mut cmd = Command::new(BIN);
    cmd.args([
        "--role",
        "hub",
        "--port",
        "0",
        "--bind",
        "127.0.0.1",
        "--data-dir",
    ])
    .arg(data)
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    if first_code {
        cmd.arg("--first-pairing-code");
    }
    let mut child = cmd.spawn().expect("start corde-server");
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut out = String::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = rx.recv_timeout(left).expect("banner within 60 s");
        out.push_str(&line);
        out.push('\n');
        if line.contains("Ctrl-C") {
            break;
        }
    }
    let control: serde_json::Value = serde_json::from_slice(
        &std::fs::read(data.join("hub").join("control.json")).expect("control file"),
    )
    .unwrap();
    let port = control["port"].as_u64().unwrap() as u16;
    Hub { child, out, port }
}

impl Hub {
    /// The 8 digits of the « Code d'appairage : 1234 5678 » line, if printed.
    fn printed_code(&self) -> Option<String> {
        // The banner also has a « Code d'appairage : sudo corde-server pair » line.
        self.out.lines().find_map(|l| {
            let after = l.split_once("Code d'appairage : ")?.1;
            let digits: String = after
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == ' ')
                .filter(char::is_ascii_digit)
                .collect();
            (digits.len() == 8).then_some(digits)
        })
    }

    fn pair(&self, code: &str) -> (u16, serde_json::Value) {
        let agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = agent
            .post(&format!("http://127.0.0.1:{}/api/hub/pair", self.port))
            .send_json(serde_json::json!({ "code": code, "name": "PC", "kind": "app" }))
            .expect("pair request");
        let status = resp.status().as_u16();
        let body = resp
            .body_mut()
            .read_json()
            .unwrap_or(serde_json::Value::Null);
        (status, body)
    }

    /// SIGTERM (what `docker stop` sends), then the process must end cleanly within 10 s.
    fn stop(mut self) {
        #[cfg(unix)]
        {
            let ok = Command::new("kill")
                .args(["-TERM", &self.child.id().to_string()])
                .status()
                .expect("kill")
                .success();
            assert!(ok);
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    assert!(status.success(), "clean exit on SIGTERM: {status}");
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "still running 10 s after SIGTERM"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        #[cfg(not(unix))]
        {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[test]
fn first_code_is_printed_only_while_no_device_is_paired_and_pairs_once() {
    let data = temp_dir("on");
    let hub = start(&data, true);
    assert!(hub.out.contains("PREMIER APPAIRAGE"), "{}", hub.out);
    assert!(hub.out.contains("Valable 10 minutes"), "{}", hub.out);
    assert!(hub.out.contains("Réglages → Serveur Corde"), "{}", hub.out);
    let code = hub.printed_code().expect("a code is printed");
    assert_eq!(code.len(), 8, "{}", hub.out);
    // Nowhere else: not in any file of the data folder.
    for entry in walk(&data) {
        let bytes = std::fs::read(&entry).unwrap_or_default();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(&code),
            "the code is written in {}",
            entry.display()
        );
    }
    // Healthy.
    corde_server::hub::cli::healthcheck(hub.port).expect("healthcheck");
    let status = Command::new(BIN)
        .args(["healthcheck", "--port", &hub.port.to_string()])
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(0));
    // A wrong code first: same rules as any code (attempts counted, code still valid).
    let wrong = if code == "00000000" {
        "11111111"
    } else {
        "00000000"
    };
    let (status, body) = hub.pair(wrong);
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["code"], "wrong_code");
    // The printed code pairs, as an administrator, once.
    let (status, body) = hub.pair(&code);
    assert_eq!(status, 201, "{body}");
    assert_eq!(body["device"]["admin"], true);
    assert_eq!(body["token"].as_str().map(str::len), Some(64));
    let (status, body) = hub.pair(&code);
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["code"], "no_code", "single use");
    hub.stop();

    // A device is paired: nothing printed after a restart, and no code issued.
    let hub = start(&data, true);
    assert!(!hub.out.contains("PREMIER APPAIRAGE"), "{}", hub.out);
    assert!(hub.printed_code().is_none(), "{}", hub.out);
    let (status, body) = hub.pair(&code);
    assert_eq!((status, body["code"].as_str()), (400, Some("no_code")));
    hub.stop();
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn a_restart_with_still_no_device_gives_a_fresh_code() {
    let data = temp_dir("again");
    let hub = start(&data, true);
    let first = hub.printed_code().expect("code");
    hub.stop();
    let hub = start(&data, true);
    let second = hub.printed_code().expect("a new code");
    assert_eq!(second.len(), 8);
    // The old one died with the previous run (it lived in memory only).
    if first != second {
        let (status, body) = hub.pair(&first);
        assert_eq!((status, body["code"].as_str()), (400, Some("wrong_code")));
    }
    let (status, _) = hub.pair(&second);
    assert_eq!(status, 201);
    hub.stop();
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn without_the_option_nothing_is_printed_and_no_code_exists() {
    let data = temp_dir("off");
    let hub = start(&data, false);
    assert!(!hub.out.contains("PREMIER APPAIRAGE"), "{}", hub.out);
    assert!(hub.printed_code().is_none(), "{}", hub.out);
    let (status, body) = hub.pair("12345678");
    assert_eq!((status, body["code"].as_str()), (400, Some("no_code")));
    hub.stop();
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn healthcheck_fails_when_nothing_answers() {
    // A port nobody listens on: bind one, read it, free it.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    assert!(corde_server::hub::cli::healthcheck(port).is_err());
    let status = Command::new(BIN)
        .args(["healthcheck", "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(1));
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}
