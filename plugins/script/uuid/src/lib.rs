//! uuid：插件指令（`provide_commands`）和随机数（`random_fill`）的参考样板，随桌面安装包分发。
//!
//! 只声明 `command` 钩子，只申请 `provide_commands`，清单声明两条指令：`/uuid`（标题 UUID）和 `/guid`（标题 GUID）。用户在 `/` 指令里打 `/uuid` 时，插件读这一次的 16 字节随机数，按 RFC 9562 设好版本位（4）和变体位（`10`），给出同一个 UUID 的四种写法：小写带连字符、大写带连字符、32 位小写不带连字符、`{大写}` 的 GUID 写法，注释分别是「小写」「大写」「无连字符」「GUID」。打 `/guid` 时 GUID 写法排在第一行。触发词后面还有字母时（`/uuidx`）不给任何行，输入法照常显示原样的那一行。
//!
//! 输入法每次在 `msime_init` 之后的快照上投递 `command`，所以插件拿不到上一次的状态：同一份预编辑串只问一次，翻页、按方向键时行不变，删掉重打才换一个新的 UUID。随机数由宿主取自操作系统的 CSPRNG，插件只读不存。
//!
//! 工作量是常数：读触发词和参数各一次、读 16 字节随机数、格式化四行、交出四行，最坏情况的 fuel 见 README。

#![no_std]

// 官方包：内容哈希锁在 plugins/official-packs.json，授权绑定在它上面。把 SDK 换成 crate 依赖会改变 LTO 合并的顺序，模块里 import 和函数的排列跟着变，哈希一变所有用户都要重新授权，所以这里仍用 `#[path]` 直接编进 SDK crate 的同一份源码，编出的字节和改成 crate 之前相同。下一次哈希本来就要变（升级编译器、改清单或源码）时，改成和别的插件一样依赖 msime-script-sdk。作为私有模块，SDK 里没用到的公开项会报 dead_code，crate 根的 `#![no_std]` 会报 unused_attributes。
#[allow(dead_code, unused_attributes)]
#[path = "../../../../crates/script-sdk/src/lib.rs"]
mod msime_script;

use msime_script::{command, hook, log, random, Error};

/// 一个 UUID 的字节数。
const UUID_BYTES: usize = 16;
/// `{` 加 36 个字符加 `}`，最长的那种写法。
const LONGEST: usize = 38;

const LOWER: &[u8; 16] = b"0123456789abcdef";
const UPPER: &[u8; 16] = b"0123456789ABCDEF";

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::COMMAND {
        on_command();
    }
    0
}

/// 这一次 `command`：读触发词和参数，参数为空时给出四行。
fn on_command() {
    let mut trigger = [0u8; 16];
    let guid = match command::text(command::TRIGGER, &mut trigger) {
        Ok(trigger) => trigger == "guid",
        Err(error) => return log::debug(reason(error)),
    };
    let mut argument = [0u8; 64];
    match command::text(command::ARGUMENT, &mut argument) {
        Ok("") => {}
        Ok(_) => return log::debug("skip: argument"),
        Err(error) => return log::debug(reason(error)),
    }
    let mut bytes = [0u8; UUID_BYTES];
    if let Err(error) = random::fill(&mut bytes) {
        return log::debug(reason(error));
    }
    // RFC 9562 第 5.4 节：第 7 个字节的高 4 位是版本 4，第 9 个字节的高 2 位是变体 `10`。
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;

    let mut lower = [0u8; LONGEST];
    let mut upper = [0u8; LONGEST];
    let mut compact = [0u8; LONGEST];
    let mut braced = [0u8; LONGEST];
    let lower = format(&bytes, LOWER, true, &mut lower);
    let upper = format(&bytes, UPPER, true, &mut upper);
    let compact = format(&bytes, LOWER, false, &mut compact);
    braced[0] = b'{';
    let length = hex(&bytes, UPPER, true, &mut braced[1..]);
    braced[length + 1] = b'}';
    let braced = ascii(&braced[..length + 2]);

    let rows: [(&str, &str); 4] = if guid {
        [
            (braced, "GUID"),
            (lower, "小写"),
            (upper, "大写"),
            (compact, "无连字符"),
        ]
    } else {
        [
            (lower, "小写"),
            (upper, "大写"),
            (compact, "无连字符"),
            (braced, "GUID"),
        ]
    };
    for (text, note) in rows {
        if let Err(error) = command::row(text, note) {
            return log::debug(reason(error));
        }
    }
    log::debug(if guid { "guid: 4 rows" } else { "uuid: 4 rows" });
}

/// `bytes` 的十六进制写进 `out`，返回其中的那一段。
fn format<'a>(
    bytes: &[u8; UUID_BYTES],
    digits: &[u8; 16],
    hyphens: bool,
    out: &'a mut [u8; LONGEST],
) -> &'a str {
    let length = hex(bytes, digits, hyphens, out);
    ascii(&out[..length])
}

/// `bytes` 的十六进制，`hyphens` 为真时按 8-4-4-4-12 插连字符，返回写了几个字节。
#[inline(never)]
fn hex(bytes: &[u8; UUID_BYTES], digits: &[u8; 16], hyphens: bool, out: &mut [u8]) -> usize {
    let mut length = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if hyphens && matches!(index, 4 | 6 | 8 | 10) {
            out[length] = b'-';
            length += 1;
        }
        out[length] = digits[usize::from(byte >> 4)];
        out[length + 1] = digits[usize::from(byte & 0x0F)];
        length += 2;
    }
    length
}

/// 只含 ASCII 的字节就是合法的 UTF-8。
fn ascii(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap_or("")
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
