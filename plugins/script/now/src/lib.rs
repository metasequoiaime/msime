//! now：`/` 指令里的本地时间的参考样板。
//!
//! 只声明 `command` 钩子，只申请 `provide_commands`，清单声明一条指令 `/now`（标题「现在」）。用户在 `/` 指令里打完 `/now` 时，输入法在这一次 `command` 事件里交给插件四个时间字段，不需要能力：`LOCAL_DATE`（`yyyymmdd`）、`LOCAL_MINUTE`（当天第几分钟）、`WEEKDAY`（1 到 7，周一为 1）和 `UTC_OFFSET_MINUTES`（比 UTC 早多少分钟，可以为负，用 `event::signed` 读）。插件据此给出五行：
//!
//! | 行 | 例子 | 注释 |
//! | --- | --- | --- |
//! | 日期 | `2026-10-26` | 日期 |
//! | 中文日期 | `2026年10月26日 星期一` | 中文 |
//! | 时间 | `14:05` | 时间 |
//! | 日期和时间 | `2026-10-26 14:05` | 日期时间 |
//! | ISO 8601 | `2026-10-26T14:05+08:00` | ISO 8601 |
//!
//! 时间字段只给到分钟，也只在 `command` 里有；宿主读不出系统时钟时四个字段都是 `ABSENT`，这时插件不给任何行，输入法照常显示原样的 `/now`。触发词后面还有字母时（`/nowx`）也不给行。
//!
//! 工作量是常数：读参数一次、读四个字段、拼五行、交出五行、写一行日志。行用固定大小的缓冲手工拼，不用 `core::fmt`：格式化机制会让模块大好几倍，在解释器里也贵得多。

#![no_std]

use msime_script_sdk::{command, event, hook, log, Error};

/// 最长的一行是中文日期：`9999年12月31日 星期日`，ASCII 和汉字合计 30 字节。
const LINE_BYTES: usize = 32;

/// `WEEKDAY` 1 到 7 对应的汉字，周一为 1。
const WEEKDAYS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script_sdk::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::COMMAND {
        on_command();
    }
    0
}

/// 这一次 `command` 里的本地时间，各字段已经核对过范围。
struct Now {
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    /// 1 到 7，周一为 1。
    weekday: usize,
    /// 比 UTC 早多少分钟，可以为负。
    offset: i64,
}

fn on_command() {
    let mut argument = [0u8; 64];
    match command::text(command::ARGUMENT, &mut argument) {
        Ok("") => {}
        Ok(_) => return log::debug("skip: argument"),
        Err(error) => return log::debug(reason(error)),
    }
    let now = match read_now() {
        Ok(now) => now,
        Err(error) => return log::debug(reason(error)),
    };

    let mut date = Line::new();
    date.date(&now);

    let mut chinese = Line::new();
    chinese.number(now.year);
    chinese.push("年");
    chinese.number(now.month);
    chinese.push("月");
    chinese.number(now.day);
    chinese.push("日 星期");
    chinese.push(WEEKDAYS[now.weekday - 1]);

    let mut time = Line::new();
    time.time(&now);

    let mut both = Line::new();
    both.date(&now);
    both.push(" ");
    both.time(&now);

    let mut iso = Line::new();
    iso.date(&now);
    iso.push("T");
    iso.time(&now);
    iso.push(if now.offset < 0 { "-" } else { "+" });
    let offset = now.offset.unsigned_abs() as u32;
    iso.two(offset / 60);
    iso.push(":");
    iso.two(offset % 60);

    let rows: [(&Line, &str); 5] = [
        (&date, "日期"),
        (&chinese, "中文"),
        (&time, "时间"),
        (&both, "日期时间"),
        (&iso, "ISO 8601"),
    ];
    for (line, note) in rows {
        if let Err(error) = command::row(line.text(), note) {
            return log::debug(reason(error));
        }
    }
    log::debug("now: 5 rows");
}

/// 读四个时间字段。任何一个读不到（宿主没交时间，或者不在 `command` 里）都是 `Err(Error::Absent)`；范围不对是 `Err(Error::Invalid)`，宿主已经核对过，这里只为了拼行时不越界。
fn read_now() -> Result<Now, Error> {
    let date = event::scalar(event::LOCAL_DATE)?;
    let minute = event::scalar(event::LOCAL_MINUTE)?;
    let weekday = event::scalar(event::WEEKDAY)?;
    let offset = event::signed(event::UTC_OFFSET_MINUTES)?;
    let (year, month, day) = (
        (date / 10_000) as u32,
        (date / 100 % 100) as u32,
        (date % 100) as u32,
    );
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || minute >= 1440
        || !(1..=7).contains(&weekday)
        || offset.unsigned_abs() >= 24 * 60
    {
        return Err(Error::Invalid);
    }
    Ok(Now {
        year,
        month,
        day,
        hour: (minute / 60) as u32,
        minute: (minute % 60) as u32,
        weekday: weekday as usize,
        offset,
    })
}

/// 拼一行文本的固定缓冲。
struct Line {
    bytes: [u8; LINE_BYTES],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Self {
            bytes: [0; LINE_BYTES],
            len: 0,
        }
    }

    fn push(&mut self, text: &str) {
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
        self.len += text.len();
    }

    fn digit(&mut self, value: u32) {
        self.bytes[self.len] = b'0' + (value % 10) as u8;
        self.len += 1;
    }

    /// 两位数，不足补 0。
    fn two(&mut self, value: u32) {
        self.digit(value / 10);
        self.digit(value);
    }

    /// 不补 0 的十进制，最多四位（年份）。
    fn number(&mut self, value: u32) {
        let mut started = false;
        for divisor in [1000, 100, 10, 1] {
            let digit = value / divisor % 10;
            if started || digit != 0 || divisor == 1 {
                self.digit(digit);
                started = true;
            }
        }
    }

    /// `yyyy-mm-dd`。
    fn date(&mut self, now: &Now) {
        self.two(now.year / 100);
        self.two(now.year % 100);
        self.push("-");
        self.two(now.month);
        self.push("-");
        self.two(now.day);
    }

    /// `hh:mm`，24 小时制。
    fn time(&mut self, now: &Now) {
        self.two(now.hour);
        self.push(":");
        self.two(now.minute);
    }

    /// 拼进去的都是完整的 UTF-8 片段。
    fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

fn reason(error: Error) -> &'static str {
    match error {
        Error::Denied => "skip: denied",
        Error::Absent => "skip: absent",
        Error::Limit => "skip: limit",
        _ => "skip: invalid",
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
