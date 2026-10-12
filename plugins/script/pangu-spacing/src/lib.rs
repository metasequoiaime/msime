//! pangu-spacing：上屏前在中日韩文字和 ASCII 字母、数字之间加一个空格，「用Rust写插件」上屏为「用 Rust 写插件」。
//!
//! 只声明 `before_commit`，申请 `read_commit` 和 `rewrite_commit`。`before_commit` 每次都在 `msime_init` 之后的快照上运行，所以下面两块静态缓冲每次都是干净的，也不留跨事件的状态。宿主拒绝改写时上屏保持原文。
//!
//! 和 docs/script-plugin-template 里的同名模板做的是同一件事，区别在工作量：模板逐字 `chars()` 加 `encode_utf8`，还要先对整段做 UTF-8 校验，body 超过约 100 个汉字就会耗尽 `before_commit` 的 fuel（算一次失败）。这里先看长度，超过 [`MAX_BODY`] 的 body 不读；读进来之后按字节扫描、按段整块复制；要看的地方超过 [`MAX_CHANGES`] 处就整段放弃。最坏情况的 fuel 见 README。
//!
//! fuel 的计法决定了写法：wasmi 按指令计 fuel，`local.get` 也算一条，而且每轮循环按循环体词法上的全部指令扣，不管走的是哪个分支。所以扫描循环只做一件事：查表看这个字节的种类和上一个字节是否相同。中日韩文字和拉丁字母相邻的地方，种类一定在「ASCII」和「多字节字符」之间变化，所以只有种类变化的地方才交给不内联的 [`Spacer::change`] 细看。

#![no_std]

#[path = "../../../../docs/script-plugin-template/sdk/msime_script.rs"]
mod msime_script;

use core::ptr::addr_of_mut;
use msime_script::{commit, event, hook, log, rewrite, Error};

/// 只处理不超过这么多字节的 body，更长的直接按原文上屏。
const MAX_BODY: usize = 1024;
/// 空格只插在两个字符之间，插入点少于字符数，结果不超过原文的两倍。
const MAX_OUT: usize = 2 * MAX_BODY;
/// 字节种类变化的次数上限，超过就整段放弃。一段夹着英文单词的中文，每个单词两次；`:`、`.`、`@` 各两次（`/`、`\`、`?`、`=` 一出现就整段放弃，不计）。正常输入的一次上屏远到不了这个数，汉字和字母逐个交替的病态输入才会。它和 [`MAX_BODY`] 一起给出最坏情况的 fuel：扫满 1024 字节约 17 000，每次变化连同插空格约 300，旁边是假名、注音（所在的块要完整解码，见 [`cjk_exact`]）时约 420，最坏约 43 800。
const MAX_CHANGES: u32 = 64;

static mut BODY: [u8; MAX_BODY] = [0; MAX_BODY];
static mut OUT: [u8; MAX_OUT] = [0; MAX_OUT];

/// 字节的种类：普通 ASCII、可能属于网址、邮箱或路径的 ASCII 标点（`:`、`.`、`@`、`/`、`\`、`?`、`=`）、多字节字符的字节。
const PLAIN: u8 = 0;
const MARK: u8 = 1;
const WIDE: u8 = 2;

static KIND: [u8; 256] = {
    let mut table = [WIDE; 256];
    let mut b = 0;
    while b < 128 {
        table[b] = match b as u8 {
            b':' | b'.' | b'@' | b'/' | b'\\' | b'?' | b'=' => MARK,
            _ => PLAIN,
        };
        b += 1;
    }
    table
};

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::BEFORE_COMMIT {
        before_commit();
    }
    0
}

fn before_commit() {
    // 不可改写（来源、本地模式、选法、输入框不允许）时什么都不做。
    if event::scalar(event::REWRITABLE) != Ok(1) {
        return log::debug("skip: not rewritable");
    }
    // 先读长度：太长的 body 连复制都省掉。
    let len = match commit::scalar(commit::COMMIT_BODY) {
        Ok(0) => return log::debug("keep"),
        Ok(len) if len as usize <= MAX_BODY => len as usize,
        Ok(_) => return log::debug("skip: too long"),
        Err(error) => return log::debug(reason(error)),
    };
    // SAFETY: 插件单线程运行，两块缓冲只在这一次调用里用。
    let (body, out) = unsafe { (&mut *addr_of_mut!(BODY), &mut *addr_of_mut!(OUT)) };
    let body = match commit::bytes(commit::COMMIT_BODY, &mut body[..len]) {
        Ok(body) => body,
        Err(error) => return log::debug(reason(error)),
    };
    let spaced = match space(body, out) {
        Ok(Some(spaced)) => spaced,
        Ok(None) => return log::debug("keep"),
        Err(why) => return log::debug(why),
    };
    // SAFETY: 结果由原文（宿主给的合法 UTF-8）在字符边界处切开的片段和 ASCII 空格拼成。
    let spaced = unsafe { core::str::from_utf8_unchecked(spaced) };
    log::debug(spaced);
    // 宿主拒绝改写时按原文上屏，不算失败。
    let _ = rewrite::commit(spaced);
}

fn reason(error: Error) -> &'static str {
    match error {
        Error::Denied => "skip: denied",
        Error::Absent => "skip: absent",
        _ => "skip: unreadable",
    }
}

/// 在中日韩文字和 ASCII 字母、数字直接相邻的地方插入一个空格，写进 `out`。没有要插的地方是 `Ok(None)`；像网址、邮箱，或者要看的地方太多，是带原因的 `Err`。
fn space<'a>(body: &[u8], out: &'a mut [u8]) -> Result<Option<&'a [u8]>, &'static str> {
    let mut spacer = Spacer {
        start: 0,
        written: 0,
        changes: 0,
    };
    let mut previous = PLAIN;
    let mut i = 0;
    loop {
        i = next_change(body, i, previous);
        if i >= body.len() {
            break;
        }
        previous = KIND[body[i] as usize];
        match spacer.change(body, i, out) {
            GO_ON => {}
            ADDRESS => return Err("skip: address"),
            _ => return Err("skip: too many changes"),
        }
        i += 1;
    }
    if spacer.written == 0 {
        return Ok(None);
    }
    let (start, written) = (spacer.start, spacer.written);
    let tail = body.len() - start;
    out[written..written + tail].copy_from_slice(&body[start..]);
    Ok(Some(&out[..written + tail]))
}

/// 从 `i` 起第一个种类不是 `previous` 的字节；没有时是 `body.len()`。这是唯一逐字节执行的循环，所以单独成函数，循环体只有查表和比较。
#[inline(never)]
fn next_change(body: &[u8], mut i: usize, previous: u8) -> usize {
    while i < body.len() && KIND[body[i] as usize] == previous {
        i += 1;
    }
    i
}

/// [`Spacer::change`] 的结果。
const GO_ON: u8 = 0;
const ADDRESS: u8 = 1;
const TOO_MANY: u8 = 2;

struct Spacer {
    /// 还没复制进 `out` 的那段从哪里开始。
    start: usize,
    /// `out` 里已经写了多少字节。
    written: usize,
    changes: u32,
}

// 下面的函数都不内联：wasmi 在函数入口按函数体里（循环以外）词法上的全部指令扣 fuel，即使提前返回也一样。只在少数情况下才用到的代码拆成单独的函数，常见的路径就只付自己那一小段。
impl Spacer {
    /// `body[i]` 和前一个字节的种类不同：看是不是网址、邮箱，要不要在 `i` 之前插空格。
    #[inline(never)]
    fn change(&mut self, body: &[u8], i: usize, out: &mut [u8]) -> u8 {
        self.changes += 1;
        if self.changes > MAX_CHANGES {
            return TOO_MANY;
        }
        let right = body[i];
        if KIND[right as usize] == MARK {
            return if is_address(body, i) { ADDRESS } else { GO_ON };
        }
        if i == 0 {
            return GO_ON;
        }
        // 种类变化处至少一边是 ASCII。一边是字母数字、另一边是中日韩文字时插空格。
        let left = body[i - 1];
        let cjk = if right >= 0x80 {
            left.is_ascii_alphanumeric() && cjk_at(body, i)
        } else {
            left >= 0x80 && right.is_ascii_alphanumeric() && cjk_before(body, i)
        };
        if cjk {
            self.insert(body, i, out);
        }
        GO_ON
    }

    /// 把还没复制的 `body[self.start..i]` 整段复制进 `out`，后面跟一个空格。
    #[inline(never)]
    fn insert(&mut self, body: &[u8], i: usize, out: &mut [u8]) {
        let len = i - self.start;
        out[self.written..self.written + len].copy_from_slice(&body[self.start..i]);
        out[self.written + len] = b' ';
        self.written += len + 1;
        self.start = i;
    }
}

/// `body[i]` 是 [`MARK`] 类的 ASCII 标点：它是不是网址、邮箱或路径的一部分。`://`、`www.`（不分大小写）、两边都是 ASCII 字母数字的 `@` 都算；`/`、`\`、`?`、`=` 一出现就算，`example.com/a`、`C:\Users`、`?q=` 这类不带协议头的网址、路径和查询串没有可靠的边界，整段放弃。宁可错过加空格的机会，也不在网址或路径里插空格。
#[inline(never)]
fn is_address(body: &[u8], i: usize) -> bool {
    let at = |k: usize| body.get(k).copied().unwrap_or(0);
    match body[i] {
        b':' => at(i + 1) == b'/' && at(i + 2) == b'/',
        b'.' => i >= 3 && body[i - 3..i].eq_ignore_ascii_case(b"www"),
        b'@' => i > 0 && at(i - 1).is_ascii_alphanumeric() && at(i + 1).is_ascii_alphanumeric(),
        _ => true,
    }
}

/// 以 `body[end - 1]` 结尾的多字节字符是不是中日韩文字。中日韩文字都是三字节字符，或者首字节为 `F0` 的四字节字符。
#[inline(never)]
fn cjk_before(body: &[u8], end: usize) -> bool {
    if end >= 3 && body[end - 3] & 0xF0 == 0xE0 {
        cjk_at(body, end - 3)
    } else {
        end >= 4 && body[end - 4] == 0xF0 && cjk_at(body, end - 4)
    }
}

/// `body[start]` 开头的多字节字符是不是中日韩文字。只看前两个字节：四字节字符看是不是第二、第三平面，三字节字符查 [`BMP_BLOCKS`]，只有落在部分属于中日韩文字的块里才完整解码。
#[inline(never)]
fn cjk_at(body: &[u8], start: usize) -> bool {
    let b0 = body[start];
    let b1 = body.get(start + 1).copied().unwrap_or(0);
    if b0 >= 0xF0 {
        // U+20000..U+3FFFF 是 F0 A0..BF xx xx。
        return b0 == 0xF0 && b1 >= 0xA0;
    }
    if b0 < 0xE0 {
        return false;
    }
    match BMP_BLOCKS[(usize::from(b0 & 0x0F) << 6) | usize::from(b1 & 0x3F)] {
        NONE => false,
        ALL => true,
        _ => cjk_exact(body, start),
    }
}

/// 三字节字符完整解码后判断。
#[inline(never)]
fn cjk_exact(body: &[u8], start: usize) -> bool {
    let at = |k: usize| u32::from(body.get(start + k).copied().unwrap_or(0) & 0x3F);
    is_cjk(u32::from(body[start] & 0x0F) << 12 | at(1) << 6 | at(2))
}

/// 基本多文种平面每 64 个码位一块（三字节 UTF-8 的前两个字节正好确定是哪一块）：整块都不是中日韩文字、整块都是、一部分是。编译时由 [`is_cjk`] 算出。
const NONE: u8 = 0;
const ALL: u8 = 1;
const SOME: u8 = 2;
static BMP_BLOCKS: [u8; 1024] = {
    let mut table = [NONE; 1024];
    let mut block = 0;
    while block < 1024 {
        let first = block as u32 * 64;
        let mut count = 0;
        let mut cp = first;
        while cp < first + 64 {
            if is_cjk(cp) {
                count += 1;
            }
            cp += 1;
        }
        table[block] = match count {
            0 => NONE,
            64 => ALL,
            _ => SOME,
        };
        block += 1;
    }
    table
};

/// 中日韩文字，范围同 pangu.js：部首、康熙部首、假名（不含 U+30FB「・」）、注音、带圈字、扩展 A、基本区、兼容汉字，另加第二、第三平面的汉字。不含 U+3000..U+303F 和全角形式里的标点，所以全角标点两侧不加空格。
const fn is_cjk(cp: u32) -> bool {
    matches!(
        cp,
        0x2E80..=0x2FDF
            | 0x3040..=0x309F
            | 0x30A0..=0x30FA
            | 0x30FC..=0x30FF
            | 0x3100..=0x312F
            | 0x3200..=0x32FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x3FFFF
    )
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
