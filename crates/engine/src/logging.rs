//! 引擎的日志出口。引擎只在自己兜住了的异常上记一行（日志不是数据库被挪开、反查索引建不了、下载的五笔码表并不进代次、辅助码表读不到），每一行都是警告，不影响调用结果。
//!
//! 缺省写到标准错误，前面加 `msime: `，和以前直接 `eprintln!` 的输出逐字节相同。嵌入引擎的程序用 [`set_log_sink`] 换成自己的出口（宿主日志、C ABI 的回调）；引擎的非测试代码不直接写标准错误，`lib.rs` 用 `clippy::print_stderr` 拦住。

use std::fmt;
use std::sync::{Arc, PoisonError, RwLock};

/// 接收一行日志（不带 `msime: ` 前缀和换行）。可能在任何线程上被调用，要自己处理并发；不要在里面再调引擎。
pub type LogSink = Arc<dyn Fn(&str) + Send + Sync>;

static SINK: RwLock<Option<LogSink>> = RwLock::new(None);

/// 换掉进程级的日志出口；`None` 回到缺省的标准错误。之后记的每一行都交给新的出口。
pub fn set_log_sink(sink: Option<LogSink>) {
    *SINK.write().unwrap_or_else(PoisonError::into_inner) = sink;
}

/// 记一行警告。出口在锁外调用，出口里再换出口也不会死锁。
#[allow(clippy::print_stderr)]
pub(crate) fn warn(message: fmt::Arguments<'_>) {
    let sink = SINK.read().unwrap_or_else(PoisonError::into_inner).clone();
    match sink {
        Some(sink) => sink(&message.to_string()),
        None => eprintln!("msime: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// 换上的出口收到不带前缀的整行；换回 None 后不再收到。只有这一个测试换出口，其他测试并行记的日志可能也会落进来，所以只找自己那一行。
    #[test]
    fn an_installed_sink_receives_lines_until_it_is_removed() {
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = Arc::clone(&lines);
        set_log_sink(Some(Arc::new(move |line: &str| {
            captured.lock().unwrap().push(line.to_owned());
        })));
        warn(format_args!("synthetic log line {}", 1));
        set_log_sink(None);
        warn(format_args!("synthetic log line {}", 2));
        let lines = lines.lock().unwrap();
        assert!(lines.iter().any(|line| line == "synthetic log line 1"));
        assert!(!lines.iter().any(|line| line == "synthetic log line 2"));
    }
}
