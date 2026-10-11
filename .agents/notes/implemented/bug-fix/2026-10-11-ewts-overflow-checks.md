# Agent Note: 对 ewts 关掉溢出检查，让藏文转换在各种构建里一致

Status: implemented

## Problem

`host_session` 的长时间模糊测试（见 [cargo-fuzz 目标](../testing/2026-10-11-cargo-fuzz-targets.md)）在藏文方案下崩溃。调用链：`msime_client_character` → `input-runtime` → `engine` 的 `tibetan::scheme::build_request_into` → `ewts::EwtsConverter::ewts_to_unicode`，最后 panic 在 ewts 0.1.3 `src/converter.rs:136` 的 `self.ind - 2`。`is_lower_form` 判断第二个辅音能否作下加字时回看两个位置，第二个辅音恰好是 `w` 这类字母时（最短输入 `bw`、`yw`），`ind` 为 1，`usize` 下溢。

后果按构建分：

- 发版构建（各平台都用 `--release`，包括 Windows 的 `Build-RustOutputs.ps1`）不检查溢出，下标回绕后 `get_con_at` 得到 `None`，退回 `unwrap_or(prev_con_as_sup.is_finite)`，显示独立的 wa（`bw` → `བཝ`）。用户从来没遇到过崩溃。
- 检查溢出的构建会在这一键 panic，包括 dev 构建、oss-scanner 用的 `.oss-scanner/Dockerfile`（debug）以及 cargo-fuzz。`host-api` 的 `response()` 会把 panic 接成 `internal runtime failure`，这一键因此失效。

0.1.3 是 crates.io 上的最新版。上游仓库从 2024-12 起没有更新，修复已作为 emgyrz/ewts-rs#1 提交，但不知道什么时候合并。

## Decision

在根 `Cargo.toml` 对 `ewts` 这一个 crate 关掉溢出检查：`[profile.dev.package.ewts]` 写 `overflow-checks = false`，`[profile.release.package.ewts]` 写 `debug-assertions = true` 加 `overflow-checks = false`。`fuzz/Cargo.toml` 是独立的 workspace，有同样的 release 段。这样所有构建都得到发版构建一直以来的结果，与上游修复（`checked_sub(2)` 后走同一个 `unwrap_or` 分支）的输出完全一致。

release 段同时打开 `debug-assertions`，是因为 cargo 只在包的 `overflow-checks` 与 `debug-assertions` 取值不同时，才显式传 `-C overflow-checks=off`。release 里两者默认都是 false，只写 `overflow-checks = false` 时 cargo 什么也不传，而 `RUSTFLAGS=-Cdebug-assertions`（cargo-fuzz 正是这样构建的）会顺带把溢出检查打开。用一个只依赖 ewts 的小程序在 `RUSTFLAGS=-Cdebug-assertions cargo run --release` 下实测：只写 `overflow-checks = false` 时 `bw` 仍然 panic，两项都写时得到 `བཝ`。ewts 源码里没有 `debug_assert!`，打开它不改变任何行为。

`crates/engine/src/tibetan/scheme.rs` 新增 `a_second_consonant_that_may_be_a_subscript_converts_in_every_build`，固定 `bw`、`yw`、`bwa` 的转换结果；`fuzz/seeds/host_session/tibetan-subscript-wa` 是同一个最短复现输入（藏文方案、`b`、`w`）。

上游发版后，升级 `crates/engine/Cargo.toml` 里的 `ewts`，并删掉根 `Cargo.toml` 和 `fuzz/Cargo.toml` 里这几段。

## Alternatives considered

- **把 ewts vendor 进仓库并用 `[patch.crates-io]` 打补丁**：最强的理由是直接修掉 bug 本身，不靠构建配置兜底，以后也不怕有人用 `RUSTFLAGS=-Coverflow-checks` 强行打开检查。不用它，是因为要多维护 1,150 行第三方代码，还要按 ARCHITECTURE 附上来源、许可证和 NOTICE；`cargo vendor` 源码包、Gentoo 的逐个 crate 清单和 dependency-review 都要跟着改。而它换来的输出与现在发版构建的完全相同。
- **只等上游合并**：最强的理由是不在本仓库留任何权宜之计。不用它，是因为上游近两年没有动静；在这期间 dev 构建、oss-scanner 和 fuzz 都会反复撞上同一个 panic，fuzz 每次都会停在这里，后面的问题就找不到了。
- **在 `tibetan::scheme` 里避开会触发的输入**：最强的理由是不碰构建配置。不用它，是因为触发条件取决于 ewts 内部的下加字表和前一个辅音是否已经组合，在调用方很难完整描述，规则写不全就等于没修。

## Consequences

收益：dev、release、fuzz 以及任何全局打开 `overflow-checks` 的 release 构建里，藏文方案打 `bw` 都得到 `བཝ`，不再 panic；oss-scanner 的 debug 镜像不会再报这个问题，`host_session` 的长时间运行也能越过它继续找别的问题。

代价：ewts 内部如果还有别的溢出，在任何构建里都会静默回绕，而不是 panic。这和发版构建一直以来的行为相同，但 debug 构建因此失去了在这个 crate 里发现溢出的能力。用 `RUSTFLAGS=-Coverflow-checks=on` 显式强制打开检查的构建不受这几段保护。

## Testing

- 修复前，engine 新增的测试在 dev 构建下失败，报 `ewts-0.1.3/src/converter.rs:136:37: attempt to subtract with overflow`；修复后 `cargo test -p msime-engine --lib tibetan` 的 22 个测试全部通过（Mac Studio，经 rbuild）。
- `cargo test --release -p msime-engine --lib tibetan` 同样 22 个全部通过。
- 带 ASan 的 cargo-fuzz 构建重放 `tibetan-subscript-wa` 和长时间运行抓到的两个真实崩溃输入（都在藏文方案下，一个含 `bw`，一个含 `yw`），三个都正常执行完毕；修复前后两个在同一行 panic。
