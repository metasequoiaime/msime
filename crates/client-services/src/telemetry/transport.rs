//! 生产遥测固定使用正式源站；显式测试构建只向编译时指定的回环服务发送。

use super::TelemetrySender;
use crate::account::{AccountError, BackendAccountClient};

/// 构造遥测专用发送器，不向调用方暴露账号操作或任意源站配置。
pub fn backend_sender() -> Result<impl TelemetrySender, AccountError> {
    #[cfg(feature = "telemetry-loopback")]
    {
        isolated_sender(option_env!("MSIME_TEST_TELEMETRY_ORIGIN"))
    }
    #[cfg(not(feature = "telemetry-loopback"))]
    {
        BackendAccountClient::new()
    }
}

/// 验收工具核对编译时目标；缺失配置是错误，不能从运行时环境补值。
#[cfg(feature = "telemetry-loopback")]
pub fn isolated_origin() -> Result<&'static str, AccountError> {
    validate_origin(option_env!("MSIME_TEST_TELEMETRY_ORIGIN"))
}

#[cfg(any(test, feature = "telemetry-loopback"))]
fn validate_origin(origin: Option<&str>) -> Result<&str, AccountError> {
    let origin = origin.ok_or(AccountError::Invalid)?;
    // 先按字节接受唯一规范形状，避免 URL 解析器规范化短 IP、空白、凭据和路径。
    let port = origin
        .strip_prefix("http://127.0.0.1:")
        .ok_or(AccountError::Invalid)?;
    let number = port.parse::<u16>().map_err(|_| AccountError::Invalid)?;
    if number == 0 || port != number.to_string() {
        return Err(AccountError::Invalid);
    }
    Ok(origin)
}

#[cfg(any(test, feature = "telemetry-loopback"))]
fn isolated_sender(origin: Option<&str>) -> Result<BackendAccountClient, AccountError> {
    BackendAccountClient::telemetry_loopback(validate_origin(origin)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::Delivery;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn isolated_configuration_never_accepts_a_noncanonical_target() {
        assert!(isolated_sender(None).is_err());
        for origin in [
            "",
            "https://api.msime.app",
            "https://127.0.0.1:80",
            "http://localhost:80",
            "http://127.1:80",
            "http://2130706433:80",
            "http://[::1]:80",
            "http://127.0.0.2:80",
            "http://127.0.0.1",
            "http://127.0.0.1:0",
            "http://127.0.0.1:65536",
            "http://127.0.0.1:080",
            "http://127.0.0.1:+80",
            "http://127.0.0.1:80/",
            "http://127.0.0.1:80/x",
            "http://127.0.0.1:80?q=1",
            "http://127.0.0.1:80#x",
            "http://user@127.0.0.1:80",
            " http://127.0.0.1:80",
            "http://127.0.0.1:80\n",
            "HTTP://127.0.0.1:80",
        ] {
            assert!(isolated_sender(Some(origin)).is_err(), "{origin:?}");
        }
        assert_eq!(
            validate_origin(Some("http://127.0.0.1:80")).unwrap(),
            "http://127.0.0.1:80"
        );
        assert!(validate_origin(Some("http://127.0.0.1:65535")).is_ok());
    }

    #[test]
    fn isolated_sender_posts_without_credentials_and_refuses_redirects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < until, "未收到遥测请求");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
                assert!(request.len() < 8192);
            }
            let headers = String::from_utf8(request).unwrap().to_ascii_lowercase();
            assert!(headers.starts_with("post /v1/telemetry/events http/1.1\r\n"));
            assert!(!headers.contains("authorization:"));
            assert!(!headers.contains("cookie:"));
            assert!(headers.contains("content-length: 2\r\n"));
            let mut body = [0; 2];
            stream.read_exact(&mut body).unwrap();
            assert_eq!(&body, b"{}");
            // 重定向指向另一个仅回环监听器；不得跟随，也不接触公网。
            let trap = TcpListener::bind("127.0.0.1:0").unwrap();
            trap.set_nonblocking(true).unwrap();
            write!(stream, "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", trap.local_addr().unwrap()).unwrap();
            (trap, stream)
        });
        assert_eq!(
            isolated_sender(Some(&origin)).unwrap().send(b"{}"),
            Delivery::Failed
        );
        let (trap, _stream) = worker.join().unwrap();
        assert!(
            matches!(trap.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }

    #[cfg(feature = "telemetry-loopback")]
    #[test]
    fn test_build_factory_cannot_fall_back_to_production() {
        let expected = validate_origin(option_env!("MSIME_TEST_TELEMETRY_ORIGIN"));
        assert_eq!(isolated_origin().is_ok(), expected.is_ok());
        assert_eq!(backend_sender().is_ok(), expected.is_ok());
    }
}
