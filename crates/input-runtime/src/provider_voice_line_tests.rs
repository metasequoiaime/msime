use super::{read_voice_provider_line_with, VoiceCancellation};
use std::cell::Cell;
use std::io::{self, ErrorKind};
use std::time::{Duration, Instant};

fn scripted_read(
    steps: Vec<Result<Vec<u8>, ErrorKind>>,
) -> impl FnMut(Duration, &mut [u8]) -> Option<io::Result<usize>> {
    let mut steps = steps.into_iter();
    move |timeout, buffer| {
        assert!(!timeout.is_zero() && timeout <= Duration::from_millis(100));
        Some(match steps.next().expect("读取序列不应被耗尽") {
            Ok(bytes) => {
                buffer[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
            Err(kind) => Err(kind.into()),
        })
    }
}

#[test]
fn interrupted_voice_read_retries_before_the_first_bytes() {
    let line = "{\"text\":\"合成语音\"}\n";
    let read = scripted_read(vec![
        Err(ErrorKind::Interrupted),
        Ok(line.as_bytes().to_vec()),
    ]);
    assert_eq!(
        read_voice_provider_line_with(
            &mut Vec::new(),
            Instant::now() + Duration::from_secs(1),
            None,
            read
        ),
        Some(line.into())
    );
}

#[test]
fn interrupted_voice_read_preserves_a_partial_utf8_character() {
    let line = "{\"text\":\"合成语音\"}\n";
    let split = line.find('合').unwrap() + 1;
    let read = scripted_read(vec![
        Ok(line.as_bytes()[..split].to_vec()),
        Err(ErrorKind::Interrupted),
        Ok(line.as_bytes()[split..].to_vec()),
    ]);
    assert_eq!(
        read_voice_provider_line_with(
            &mut Vec::new(),
            Instant::now() + Duration::from_secs(1),
            None,
            read
        ),
        Some(line.into())
    );
}

#[test]
fn interrupted_voice_read_rechecks_cancellation() {
    let polls = Cell::new(0);
    let mut cancel = || {
        polls.set(polls.get() + 1);
        polls.get() == 2
    };
    let mut cancellation = VoiceCancellation::Callback(&mut cancel);
    let read = scripted_read(vec![Err(ErrorKind::Interrupted)]);
    assert_eq!(
        read_voice_provider_line_with(
            &mut Vec::new(),
            Instant::now() + Duration::from_secs(1),
            Some(&mut cancellation),
            read
        ),
        None
    );
    assert_eq!(polls.get(), 2);
}

#[test]
fn voice_read_still_rejects_a_connection_reset() {
    let read = scripted_read(vec![Err(ErrorKind::ConnectionReset)]);
    assert_eq!(
        read_voice_provider_line_with(
            &mut Vec::new(),
            Instant::now() + Duration::from_secs(1),
            None,
            read
        ),
        None
    );
}

#[test]
fn voice_read_does_not_start_after_its_deadline() {
    assert_eq!(
        read_voice_provider_line_with(&mut Vec::new(), Instant::now(), None, |_, _| panic!(
            "截止后不应读取"
        )),
        None
    );
}

#[test]
fn voice_read_stops_when_timeout_configuration_fails() {
    let calls = Cell::new(0);
    assert_eq!(
        read_voice_provider_line_with(
            &mut Vec::new(),
            Instant::now() + Duration::from_secs(1),
            None,
            |_, _| {
                calls.set(calls.get() + 1);
                None
            },
        ),
        None
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn voice_read_keeps_the_final_line_after_the_peer_closes() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let (mut reader, mut writer) = UnixStream::pair().unwrap();
    let line = format!("{{\"text\":\"{}\"}}\n", "合成语音".repeat(256));
    writer.write_all(line.as_bytes()).unwrap();
    drop(writer);
    assert_eq!(
        super::read_voice_provider_line(
            &mut reader,
            &mut Vec::new(),
            Instant::now() + Duration::from_secs(1),
            None,
        ),
        Some(line)
    );
}

#[test]
fn voice_read_rejects_an_incomplete_line_after_the_peer_closes() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    for bytes in [b"".as_slice(), b"{\"text\":\"synthetic\"}"] {
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(bytes).unwrap();
        drop(writer);
        let mut pending = Vec::new();
        assert_eq!(
            super::read_voice_provider_line(
                &mut reader,
                &mut pending,
                Instant::now() + Duration::from_secs(1),
                None,
            ),
            None
        );
        assert_eq!(pending, bytes);
    }
}
