//! 给候选窗口里的单字加注释「单字」。由 msime-pack new 生成，换成你自己的逻辑。
//!
//! 用到 `candidates::count`、`candidates::text`（能力 `read_candidates`）和 `annotate::set`（能力 `annotate_candidates`），清单里申请的正是这两项。

#![no_std]

use msime_script_sdk::{annotate, candidates, hook, log, Error};

/// 宿主交给插件的窗口最多 20 行。
const MAX_ROWS: u32 = 20;

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script_sdk::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::CANDIDATES {
        on_candidates();
    }
    // 返回负数算这一次失败。
    0
}

fn on_candidates() {
    let count = match candidates::count() {
        Ok(count) => (count as u32).min(MAX_ROWS),
        Err(error) => return log::debug(reason(error)),
    };
    let mut marked = false;
    for index in 0..count {
        let mut text = [0; 16];
        // 读不下（`Limit`）的是长词或整句，不是单字；被隐私规则屏蔽（`Absent`）的不看。
        let Ok(text) = candidates::text(index, &mut text) else {
            continue;
        };
        let mut chars = text.chars();
        if chars.next().is_some() && chars.next().is_none() {
            // 宿主拒绝时这一行照旧显示，不算失败。
            marked |= annotate::set(index, "单字").is_ok();
        }
    }
    log::debug(if marked { "annotated" } else { "keep" });
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
