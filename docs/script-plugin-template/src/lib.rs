//! pangu-spacing：上屏前在中文和英文字母、数字之间加一个空格，「用Rust写插件」上屏为「用 Rust 写插件」。
//!
//! 只声明 `before_commit` 一个钩子，申请 `read_commit`（读上屏文本）和 `rewrite_commit`（改写它）。`before_commit` 每次都在 `msime_init` 之后的快照上运行，所以这里不留任何跨事件的状态。宿主拒绝改写（`DENIED` 等）时上屏保持原文；`msime-pack run` 会把这次改写请求和它的结果列在输出里。

#![no_std]

#[path = "../sdk/msime_script.rs"]
mod msime_script;

use msime_script::{commit, event, hook, log, rewrite};

/// 改写结果不能超过 4096 字节，原文更长时不改。
const MAX_TEXT: usize = 4096;

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
    // 不可改写（本地模式、来源、字段不允许）时什么都不做。
    if event::scalar(event::REWRITABLE) != Ok(1) {
        return;
    }
    let mut body = [0; MAX_TEXT];
    // 没有授权、文本被屏蔽、或者太长：都按原文上屏。
    let Ok(text) = commit::text(commit::COMMIT_BODY, &mut body) else {
        return;
    };
    let mut spaced = [0; MAX_TEXT];
    let Some(spaced) = space(text, &mut spaced) else {
        return;
    };
    if spaced.len() == text.len() {
        return;
    }
    log::debug(spaced);
    // 宿主拒绝改写时按原文上屏，不算失败。
    let _ = rewrite::commit(spaced);
}

/// 中日韩文字：汉字（含扩展 A 和兼容汉字）、假名、注音、部首和带圈字符。
fn is_cjk(c: char) -> bool {
    matches!(
        c,
        '\u{2E80}'..='\u{2FDF}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{3100}'..='\u{312F}'
            | '\u{3200}'..='\u{32FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
    )
}

/// 在中日韩文字和 ASCII 字母、数字相邻的地方插入空格，写进 `out`。放不下时返回 `None`。
fn space<'a>(text: &str, out: &'a mut [u8]) -> Option<&'a str> {
    let mut len = 0;
    let mut previous: Option<char> = None;
    for c in text.chars() {
        if let Some(previous) = previous {
            let boundary = (is_cjk(previous) && c.is_ascii_alphanumeric())
                || (previous.is_ascii_alphanumeric() && is_cjk(c));
            if boundary {
                *out.get_mut(len)? = b' ';
                len += 1;
            }
        }
        let encoded = c.len_utf8();
        c.encode_utf8(out.get_mut(len..len + encoded)?);
        len += encoded;
        previous = Some(c);
    }
    core::str::from_utf8(&out[..len]).ok()
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
