use super::*;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;

fn token() -> String {
    "a".repeat(64)
}

/// 每次最多交出 `step` 个字节的读者，让多字节字符落在两次读取之间。
struct Trickle<'a> {
    bytes: &'a [u8],
    step: usize,
}

impl Read for Trickle<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.step.min(buffer.len()).min(self.bytes.len());
        buffer[..count].copy_from_slice(&self.bytes[..count]);
        self.bytes = &self.bytes[count..];
        Ok(count)
    }
}

fn copied(bytes: &[u8], step: usize) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    copy_dictionary_export(Trickle { bytes, step }, &mut output).map(|count| {
        assert_eq!(count, output.len() as u64);
        output
    })
}

#[test]
fn characters_split_across_reads_are_kept_whole() {
    let text = "ni\t你\t10000\n𠀀\u{1F600}\twubi\n".repeat(3);
    for step in 1..=7 {
        assert_eq!(
            copied(text.as_bytes(), step).unwrap(),
            text.as_bytes(),
            "{step}"
        );
    }
}

#[test]
fn an_export_that_is_empty_not_utf8_has_nul_or_ends_mid_character_is_refused() {
    let truncated = &"你".as_bytes()[..2];
    for (bytes, label) in [
        (&b""[..], "empty"),
        (&b"ni\t\xff\n"[..], "not UTF-8"),
        (&b"ni\0\n"[..], "NUL"),
        (truncated, "truncated"),
    ] {
        assert!(copied(bytes, 3).is_err(), "{label}");
    }
}

/// 只答一次请求的本机服务端：把收到的请求头交回，再写出 `response`。
fn serve_once(response: Vec<u8>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0, "request ended before its headers");
            request.extend_from_slice(&buffer[..read]);
        }
        sender.send(String::from_utf8(request).unwrap()).unwrap();
        stream.write_all(&response).unwrap();
    });
    (format!("http://{address}"), receiver)
}

fn ok(body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/// `directory` 里除了 `keep` 之外的文件名，用来确认失败时没有留下临时文件。
fn other_files(directory: &Path, keep: &str) -> Vec<String> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != keep)
        .collect()
}

#[test]
fn the_export_is_streamed_into_the_file_with_the_same_request_as_the_text_export() {
    let body = "ni\t你\t10000\n".repeat(1000);
    let (origin, requests) = serve_once(ok(body.as_bytes()));
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("dictionary-wubi.tsv");
    let bytes = client
        .export_dictionary_to_file(DictionaryKind::Wubi, "windows", &destination, &token())
        .unwrap();
    assert_eq!(bytes, body.len() as u64);
    assert_eq!(std::fs::read_to_string(&destination).unwrap(), body);
    let request = requests.recv().unwrap().to_ascii_lowercase();
    assert!(
        request.starts_with("get /v1/users/me/dictionaries/wubi/export?format=windows "),
        "{request}"
    );
    assert!(request.contains("accept: text/plain"), "{request}");
    assert!(
        request.contains(&format!("authorization: bearer {}", token())),
        "{request}"
    );
    assert!(other_files(directory.path(), "dictionary-wubi.tsv").is_empty());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&destination)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "{mode:o}");
    }
}

#[test]
fn a_bad_body_or_a_refusal_leaves_no_file() {
    let unauthorized = b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n".to_vec();
    for (response, expected) in [
        (ok(b"ni\0\n"), AccountError::Unavailable),
        (ok(b""), AccountError::Unavailable),
        (unauthorized, AccountError::Unauthorized),
    ] {
        let (origin, _requests) = serve_once(response);
        let client = BackendAccountClient::loopback(&origin).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("dictionary-pinyin.tsv");
        assert_eq!(
            client.export_dictionary_to_file(
                DictionaryKind::Pinyin,
                "standard",
                &destination,
                &token()
            ),
            Err(expected)
        );
        assert!(other_files(directory.path(), "").is_empty());
    }
}

#[test]
fn bad_arguments_are_refused_before_the_network() {
    // 端口 9 上没有服务端：走到联网这一步就会是 Unavailable，不是 Invalid。
    let client = BackendAccountClient::loopback("http://127.0.0.1:9").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("export.tsv");
    for (format, destination, access_token) in [
        ("csv", destination.clone(), token()),
        ("standard", PathBuf::from("relative.tsv"), token()),
        ("standard", destination.clone(), "short".to_owned()),
    ] {
        assert_eq!(
            client.export_dictionary_to_file(
                DictionaryKind::Quick,
                format,
                &destination,
                &access_token
            ),
            Err(AccountError::Invalid),
            "{format} {}",
            destination.display()
        );
    }
}
