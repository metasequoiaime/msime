# Agent Note: 皮肤文件夹导入复制绑定 staging 目录句柄

Status: implemented

## Problem

皮肤文件夹导入在创建 staging 后，递归复制普通文件仍按目标路径创建文件。staging 或其子目录条目被替换时，路径解析可能把导入内容写入另一个目录；复制失败时的路径清理也可能解析到被替换的条目。

## Decision

Unix 导入用 skin root 目录句柄创建 staging，并为每个递归子目录打开句柄。普通文件通过 `openat`、`O_EXCL` 和 `O_NOFOLLOW` 在对应句柄下创建；失败清理由已打开的父目录执行。非 Unix 保留路径实现，由 `copy_tree` 独自创建暂存目录；入口不预先创建同一个目录，否则首次复制会因目录已存在而失败。Unix 仍在入口创建并打开目录，随后通过句柄复制，不改变其路径约束。

## Alternatives considered

- 复制前重复检查 staging 路径：检查与创建之间仍有竞态。
- 只给最终文件加 `NOFOLLOW`：无法防止 staging 或中间目录被替换。
- 继续用 `remove_dir_all` 清理：目录条目替换后可能跟随错误路径。

## Consequences

皮肤导入复制的每个 Unix 写入都绑定到本次导入创建的 staging 目录对象，源文件读取和原有预算、格式校验保持不变。

Windows 7 原生主题页的实际导入测试暴露了非 Unix 路径的重复创建：有效皮肤已能扫描，但导入总返回存储失败。删除入口处的非 Unix 创建块后，路径复制与递归创建恢复各自只执行一次；首次导入、整包替换和 Unix 句柄边界使用同一组现有 Rust 测试约束。

## Verification

Windows 11 上使用 Win7 目标重新编译同源核心后，`cargo -Z build-std=std,panic_unwind test --locked --release --target x86_64-win7-windows-msvc -p msime-client-services --lib skin::folder_import::tests`：7 passed，覆盖首次导入、整包替换、暂存残留、旧槽位是普通文件、无效清单与复制超限时保留旧包。Win7 原生设置的 HWND 测试同时覆盖确认、取消、真实导入和替换后移除旧文件。同目标 `cargo clippy -Z build-std=std,panic_unwind --locked --release --target x86_64-win7-windows-msvc -p msime-client-services --lib -- -D warnings` 通过，`cargo fmt -p msime-client-services --check` 通过。

`cargo test -p msime-client-core skin::folder_import::tests:: --lib --locked`：15 passed。

`cargo test -p msime-client-core skin::candidate_community::tests:: --lib --locked`：54 passed。

`cargo fmt --all` 和 `cargo clippy -p msime-client-core --lib --locked -- -D warnings` 通过。
