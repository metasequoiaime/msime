//! 上屏前把三个英文句点 `...` 换成中文省略号 `……`。由 msime-pack new 生成，换成你自己的逻辑。
//!
//! 用到 `commit::text`（能力 `read_commit`）和 `rewrite::commit`（能力 `rewrite_commit`），清单里申请的正是这两项。

#![no_std]

use msime_script_sdk::{commit, event, hook, log, rewrite, Error};

/// 改写结果不能超过 4096 字节（宿主的上限），原文或结果更长时不改。
const MAX_TEXT: usize = 4096;

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script_sdk::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::BEFORE_COMMIT {
        before_commit();
    }
    // 返回负数算这一次失败。
    0
}

fn before_commit() {
    // 这次上屏不能改写（例如网址模式、来源不允许）时什么都不做。
    if event::scalar(event::REWRITABLE) != Ok(1) {
        return log::debug("skip: not rewritable");
    }
    let mut body = [0; MAX_TEXT];
    let body = match commit::text(commit::COMMIT_BODY, &mut body) {
        Ok(body) => body,
        Err(error) => return log::debug(reason(error)),
    };
    if !body.contains("...") {
        return log::debug("keep");
    }
    let mut out = [0; MAX_TEXT];
    let mut len = 0;
    for (index, piece) in body.split("...").enumerate() {
        if (index > 0 && !push(&mut out, &mut len, "……")) || !push(&mut out, &mut len, piece) {
            return log::debug("skip: too long");
        }
    }
    let Ok(text) = core::str::from_utf8(&out[..len]) else {
        return;
    };
    log::debug(text);
    // 宿主拒绝改写时按原文上屏，不算失败。
    let _ = rewrite::commit(text);
}

/// 把 `piece` 接到 `out[..*len]` 后面；放不下时返回 `false`，什么都不写。
fn push(out: &mut [u8], len: &mut usize, piece: &str) -> bool {
    let Some(slot) = out.get_mut(*len..*len + piece.len()) else {
        return false;
    };
    slot.copy_from_slice(piece.as_bytes());
    *len += piece.len();
    true
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
