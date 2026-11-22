//! 有界回环 HTTP 服务，实际请求经过生产传输；状态与内容均为合成数据。

use super::*;
use msime_client_core::cloud::learning_e2e::{HttpRequest, HttpResponse};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub(super) struct State {
    pub key: Option<String>,
    pub devices: BTreeMap<String, Value>,
    pub wraps: BTreeMap<String, Value>,
    pub blobs: BTreeMap<String, (u64, Vec<u8>)>,
    pub uploads: Vec<Vec<u8>>,
    pub trace: Vec<String>,
    pub upload_status: Option<u16>,
    pub lease_on_download: Option<msime_engine::RuntimePaths>,
    pub lease: Option<msime_client_services::dictionary::access::DictionaryAccess>,
    caller: String,
}

impl State {
    pub fn control(&mut self, request: &HttpRequest) -> HttpResponse {
        let body: Value = request
            .body
            .as_deref()
            .map(|body| serde_json::from_str(body).unwrap())
            .unwrap_or(Value::Null);
        let path = request.path.as_str();
        match request.method.as_str() {
            "GET" => {
                let id = path.split_once("device_id=").unwrap().1;
                self.caller = id.to_owned();
                let devices: Vec<_> = self.devices.iter().map(|(id, device)| json!({
                    "device_id": id, "public_key": device["public_key"], "name": device["name"],
                    "created_at": "2026-11-01T00:00:00Z", "last_seen": "2026-11-01T00:00:00Z",
                    "approved": self.wraps.contains_key(id),
                })).collect();
                let blobs: Vec<_> = self.blobs.iter().map(|(id, (generation, bytes))| json!({
                    "device_id": id, "key_id": self.key, "revision": generation, "bytes": bytes.len()
                })).collect();
                HttpResponse {
                    status: 200,
                    body: json!({"key_id": self.key, "devices": devices,
                    "wrap": self.wraps.get(id), "blobs": blobs})
                    .to_string(),
                }
            }
            "PUT" if path.contains("/devices/") => {
                self.devices
                    .insert(path.rsplit('/').next().unwrap().to_owned(), body);
                empty()
            }
            "POST" if path.ends_with("/key") => {
                self.key = body["key_id"].as_str().map(str::to_owned);
                self.wraps.insert(
                    self.caller.clone(),
                    json!({"wrap": body["wrap"],
                    "approver_device_id": self.caller}),
                );
                empty()
            }
            "PUT" if path.contains("/wraps/") => {
                self.wraps.insert(
                    path.rsplit('/').next().unwrap().to_owned(),
                    json!({
                    "wrap": body["wrap"], "approver_device_id": body["approver_device_id"]}),
                );
                empty()
            }
            _ => panic!("unexpected synthetic control path: {path}"),
        }
    }

    fn respond(&mut self, method: &str, path: &str, body: Vec<u8>) -> (u16, Vec<u8>) {
        self.trace.push(format!("{method} {path}"));
        if let Some(id) = path.strip_prefix("/v1/users/me/learning/blobs/") {
            if method == "PUT" {
                if let Some(status) = self.upload_status {
                    return (status, b"synthetic-sensitive-error-body".to_vec());
                }
                let (id, revision) = id.split_once("?revision=").unwrap();
                self.uploads.push(body.clone());
                self.blobs
                    .insert(id.to_owned(), (revision.parse().unwrap(), body));
                return (204, Vec::new());
            }
            assert_eq!(method, "GET");
            if let Some(paths) = self.lease_on_download.take() {
                self.lease =
                    msime_client_services::dictionary::access::DictionaryAccess::try_session(
                        &paths.user_data,
                        &paths.dictionaries,
                    )
                    .unwrap();
                assert!(self.lease.is_some());
            }
            return (200, self.blobs[id].1.clone());
        }
        let response = self.control(&HttpRequest {
            method: method.into(),
            path: path.into(),
            body: (!body.is_empty()).then(|| String::from_utf8(body).unwrap()),
        });
        (response.status, response.body.into_bytes())
    }
}

fn empty() -> HttpResponse {
    HttpResponse {
        status: 204,
        body: String::new(),
    }
}

pub(super) struct Server {
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    pub state: Arc<Mutex<State>>,
}

impl Server {
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        TEST_ORIGIN.with_borrow_mut(|slot| {
            *slot = Some(format!("http://{}", listener.local_addr().unwrap()))
        });
        let state = Arc::new(Mutex::new(State::default()));
        let captured = state.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let shutdown = stop.clone();
        let worker = thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("synthetic HTTP accept failed: {error}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let (end, length) = loop {
                    let mut buffer = [0; 4096];
                    let read = stream.read(&mut buffer).unwrap();
                    assert_ne!(read, 0);
                    bytes.extend_from_slice(&buffer[..read]);
                    assert!(bytes.len() <= 9 * 1024 * 1024);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                        assert!(
                            header.contains(&format!("authorization: bearer {}", "a".repeat(64)))
                        );
                        let length: usize = header
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length: "))
                            .unwrap_or("0")
                            .parse()
                            .unwrap();
                        break (end + 4, length);
                    }
                };
                while bytes.len() < end + length {
                    let mut buffer = [0; 4096];
                    let read = stream.read(&mut buffer).unwrap();
                    assert_ne!(read, 0);
                    bytes.extend_from_slice(&buffer[..read]);
                    assert!(bytes.len() <= 9 * 1024 * 1024);
                }
                let first = String::from_utf8_lossy(&bytes[..end])
                    .lines()
                    .next()
                    .unwrap()
                    .to_owned();
                let parts: Vec<_> = first.split_whitespace().collect();
                let (status, body) =
                    captured
                        .lock()
                        .unwrap()
                        .respond(parts[0], parts[1], bytes[end..].to_vec());
                let header = format!("HTTP/1.1 {status} Synthetic\r\nContent-Length: {}\r\nRetry-After: 17\r\nConnection: close\r\n\r\n", body.len());
                stream.write_all(header.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        Self {
            stop,
            worker: Some(worker),
            state,
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        TEST_ORIGIN.with_borrow_mut(|slot| *slot = None);
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let result = worker.join();
            if !thread::panicking() {
                assert!(result.is_ok(), "synthetic HTTP worker failed");
            }
        }
    }
}
