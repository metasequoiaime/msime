//! 写出路由测试和 `tests/smoke.mjs` 用的最小 msime.db；`--cantonese` 时改为写出最小的 msime-cantonese.db（SDK 冒烟测试的粤语方案用它）。
//!
//! usage: make_fixture <out.db> [--only wubi86 | --cantonese]

#[path = "../tests/support/fixture.rs"]
mod fixture;

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out: Option<PathBuf> = None;
    let mut only_wubi86 = false;
    let mut cantonese = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => match args.next().as_deref() {
                Some("wubi86") => only_wubi86 = true,
                other => return Err(format!("--only takes wubi86, got {other:?}").into()),
            },
            "--cantonese" => cantonese = true,
            _ if out.is_none() => out = Some(PathBuf::from(arg)),
            other => return Err(format!("unexpected argument: {other}").into()),
        }
    }
    let usage = "usage: make_fixture <out.db> [--only wubi86 | --cantonese]";
    let out = out.ok_or(usage)?;
    if cantonese && only_wubi86 {
        return Err(usage.into());
    }
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    // 覆盖旧文件：夹具总是从空库写起，结果才确定。
    if out.exists() {
        std::fs::remove_file(&out)?;
    }
    if cantonese {
        fixture::write_cantonese(&out)?;
    } else {
        fixture::write(&out, only_wubi86)?;
    }
    eprintln!("fixture written: {}", out.display());
    Ok(())
}
