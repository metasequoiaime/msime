# Agent Note: 万象模型下载校验与桌面离线导入

Status: implemented

## Problem

#11394 报告 Linux 下载万象语法模型时反复提示校验失败。万象 LTS 会在固定的 Release 地址原地替换同名 `.gram` 文件，在线目录和编译锁文件记录的摘要可能落后于这次 HTTP 响应；下载器已经拿到了完整响应并校验了长度和 `.gram` 格式，却仍把实际摘要与旧目录摘要比较，因而把有效文件丢弃。共享资源包层已有 `import` 和 Host API 的 `files` 契约，但 Linux、macOS 的共享设置页没有文件选择入口，用户无法像 Windows 一样先在浏览器下载再导入。已有的[语法模型本地快照](../feature/2026-10-26-grammar-model-local-snapshot.md)只解决安装后锁文件变化时的读取，不能替代本次下载策略。

## Decision

万象下载只把目录固定的上游仓库、文件名和 `.gram` 格式作为来源边界，接受本次完整响应产生的实际 SHA-256 和长度，并把它们写入安装快照；八股文和紧凑八股文继续严格比较目录摘要与长度。这样只放宽已知会原地替换的万象资产，不扩大其他资源包的信任范围。目录无法访问时仍沿用[目录退回锁文件](../bug-fix/2026-11-03-grammar-catalog-lock-fallback.md)的策略，万象下载完成后再按本决定处理当前响应。

桌面 Tauri 宿主新增资源包离线导入命令，复用语音模型已有的系统文件选择器、`LocalModelInstalls` 互斥登记、进度事件和稳定错误码。共享资源包状态把 `import_files` 暴露给设置页，Linux、macOS 以及已有 Tauri 壳的桌面入口显示官方下载链接和「从文件导入」操作；文件内容、长度、摘要和 `.gram` 格式仍由共享 Rust 安装器判定，页面不自行判断文件。

## Alternatives considered

- **只更新锁文件和在线目录摘要**：短期能修当前文件，但万象下一次原地替换仍会再次失败，也无法解决 Linux 无法离线导入的问题。
- **对所有语法模型放宽摘要校验**：能统一代码，但八股文资产没有同样的原地替换约束，会把目录指向的错误字节当成可用模型，因此保留严格校验。
- **只增加错误提示或让用户手工把路径写进配置**：不能让设置页完成安装，也会绕过现有文件选择器和共享安装器的安全边界，因此复用既有导入契约。

## Consequences

万象 LTS 的当前有效 `.gram` 响应可以安装，安装清单会保存实际字节的摘要；八股文模型仍在摘要或长度不符时拒绝安装。Linux 和 macOS 用户可以浏览器下载后从设置页导入，取消选择不会改变已有安装，导入失败显示与语音模型一致的稳定中文错误。资源包导入入口使用 Tauri 桌面宿主能力，移动端和不提供该命令的测试宿主不显示按钮。

验证覆盖共享 Rust 策略单测、Tauri/TypeScript 类型与设置页行为测试。真实 Linux 文件选择器和完整模型下载需要设备或桌面运行环境；本机磁盘空间不足时不启动完整本地构建，未完成的设备验收会在 PR 中明确列出。

## Verification

- `cargo test -p msime-client-providers --lib resource_packs::tests::`：42 项通过。
- `pnpm --dir apps/desktop exec vitest run tests/settings/resource-packs.test.tsx --run`：32 项通过；包含 Linux 万象离线导入按钮调用。
- `pnpm --dir apps/desktop exec tsc --noEmit`、`cargo fmt --all -- --check`、`git diff --check` 和 `pnpm run verify-notes` 通过。
- `rbuild bash scripts/verify-local.sh --quick`：run `20261113-234134` 通过；Linux Docker 宿主、HarmonyOS 工具链等按环境规则跳过。
- `rbuild sh -c 'mkdir -p target/macos/EngineResources target/macos/水杉输入法.app && cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --all-targets'`：run `20261113-235356` 通过；只剩仓库已有的 `read_aloud::available` dead-code 警告。
