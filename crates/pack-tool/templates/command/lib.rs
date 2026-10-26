//! 在 `/` 指令里给出候选：只打触发词时掷一个六面骰子，后面跟着字母时给出它们的大写。由 msime-pack new 生成，换成你自己的逻辑。
//!
//! 用到 `command::text`、`command::row`（能力 `provide_commands`）和不需要能力的 `random::fill`。触发词和标题在清单的 `[[script.commands]]` 里。

#![no_std]

use msime_script_sdk::{command, hook, log, random, Error};

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script_sdk::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::COMMAND {
        on_command();
    }
    // 返回负数算这一次失败。
    0
}

fn on_command() {
    // 触发词后面的小写字母，最多 64 字节，可以为空。
    let mut argument = [0; 64];
    let argument = match command::text(command::ARGUMENT, &mut argument) {
        Ok(argument) => argument,
        Err(error) => return log::debug(reason(error)),
    };
    let result = if argument.is_empty() {
        match roll() {
            Ok(face) => command::row(face, "掷骰"),
            Err(error) => return log::debug(reason(error)),
        }
    } else {
        let mut upper = [0; 64];
        let upper = &mut upper[..argument.len()];
        upper.copy_from_slice(argument.as_bytes());
        upper.make_ascii_uppercase();
        command::row(core::str::from_utf8(upper).unwrap_or(argument), "大写")
    };
    match result {
        Ok(_) => log::debug("1 row"),
        Err(error) => log::debug(reason(error)),
    }
}

/// 1 到 6 之间的一个数。每次 `command` 事件有 64 个随机字节，按调用顺序取；大于等于 252 的字节丢掉重取，免得取模之后各面的机会不均。
fn roll() -> Result<&'static str, Error> {
    const FACES: [&str; 6] = ["1", "2", "3", "4", "5", "6"];
    loop {
        let mut byte = [0];
        random::fill(&mut byte)?;
        if byte[0] < 252 {
            return Ok(FACES[usize::from(byte[0] % 6)]);
        }
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
