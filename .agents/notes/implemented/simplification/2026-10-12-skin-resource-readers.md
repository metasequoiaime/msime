# Agent Note: 桌面皮肤资源读取共用响应与读取入口

Status: implemented

## Problem

桌面 Tauri 的图片和字体皮肤资源读取函数分别重复了资源读取、存储错误映射和响应结构。两条路径的差异只有 MIME 类型校验。

## Decision

在 `apps/desktop/src-tauri/src/lib.rs` 增加 `SkinResourceResponse` 和 `read_skin_resource_at`。图片、字体保留原有私有函数和命令名，只调用共用入口后分别校验 `image/` 或 `font/` 前缀；类型别名保持序列化字段、前端命令和现有测试不变。

## Alternatives considered

- 把 MIME 校验下沉到 `client-core`：会让桌面命令边界的响应类型和 UI 资源策略泄漏到 host-agnostic crate。
- 删除图片、字体的包装函数：会改变测试和命令边界，且丢失各自的类型校验位置。

## Consequences

资源路径读取和错误映射只有一份实现，图片和字体仍有独立 MIME 约束，外部命令契约不变。验证使用 `cargo fmt --all -- --check`、`cargo test -p msime-desktop --lib skin --no-fail-fast`、`cargo clippy -p msime-desktop --lib --tests -- -D warnings` 和 `pnpm run verify-notes`。
