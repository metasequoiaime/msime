# Agent Note: Tauri CLI 只能由平台构建脚本驱动

Status: implemented

## Problem

AGENTS.md 开篇规定产品本体是 `platforms/<os>` 下的原生宿主，`apps/desktop` 的 Tauri 工程只是宿主按需拉起的公共组件，不能拿来启动、调试或验收。这条规定只写在文档里：仓库根的 `pnpm tauri build` / `pnpm tauri dev` 照常能跑，README 及其四种译本还把它们列为常规开发命令（`pnpm tauri build --debug --no-bundle` 被描述成「构建不打包的开发二进制」）。于是直接编出来的 `msime-desktop` 一再被当成「某个平台的应用」运行和验收，规定靠每个读者自觉。

## Decision

Tauri CLI 在编译任何 crate 之前先执行 `tauri.conf.json` 的 `beforeBuildCommand` / `beforeDevCommand`。两者现在都以 `node scripts/require-platform-build.mjs &&` 开头：环境变量 `MSIME_PLATFORM_BUILD` 不等于 `1` 时，脚本打印中英文错误并指向本机系统的 `platforms/<os>` 入口，以退出码 1 结束，Tauri CLI 随之失败。`tauri build`、`tauri dev`、`tauri android build`、`tauri ios build` 都经过这一步。

合法驱动 Tauri CLI 的平台脚本只在那一条命令上设置 `MSIME_PLATFORM_BUILD=1`：

- `platforms/windows/Build-RustOutputs.ps1`：x64 的 `tauri build --no-bundle`。变量和脚本借用的其他环境变量一样，在 `finally` 里恢复成调用前的值。
- `platforms/android/build-client-apk.sh`：`tauri android build`。
- `platforms/ios/build-app.sh`：`MSIME_IOS_TAURI_COMPONENT=1` 分支里的 `tauri ios build`。
- `platforms/macos/package-release.sh`：`tauri bundle`。它是否执行 `beforeBuildCommand` 没有在 macOS 上核实过，这里设上变量是防它执行时被拦。

门禁只在 Tauri CLI 这一层。`cargo build/check/test/clippy -p msime-desktop` 不经过它：Linux 打包（`platforms/linux/package-container.sh`）、macOS 打包里的 `cargo_universal`、CI 的 `msime-desktop` clippy 与单元测试、`verify-local.sh` 的容器 `cargo check` 都直接用 cargo，它们是合法用途。README 与 `docs/readme/` 的四种译本删掉 `pnpm tauri dev`，并把 `pnpm tauri build --debug --no-bundle` 那句改成说明它会被拒绝、入口在 `platforms/<os>`。

## Alternatives considered

- **在 `src-tauri/build.rs` 里拦** — 最彻底，cargo 直接编也拦得住。否决：Linux 和 macOS 的正式打包、CI 的 clippy/test 和本地门禁的 `cargo check` 都直接编这个 crate，要么给每一处都加变量，要么门禁形同虚设；而它们都不是把 Tauri 工程当产品运行。
- **只拦 `tauri build`，放过 `tauri dev`** — `tauri dev` 是 README 写过的前端开发方式，放过它改动最小。否决：`tauri dev` 编出并运行的正是同一个独立程序，和 AGENTS.md 点名的错误用法（`pnpm tauri dev` 不是答案）一模一样；看共享设置页的正确路径是先编宿主再由宿主打开。
- **只改文档，不加门禁** — 零构建风险。否决：现状就是只有文档，规定一直被绕过。

## Consequences

- **收益**：直接 `pnpm tauri build` / `pnpm tauri dev` 在第一步就失败，并给出本机系统的正确入口；平台脚本的行为不变。
- **代价**：想单独看共享前端的开发者失去 `tauri dev` 的热重载窗口，只能用 `pnpm dev` 在浏览器里看（无本地配置），或者经宿主打开。手动设 `MSIME_PLATFORM_BUILD=1` 可以绕过，门禁拦的是误用，不是蓄意。
- **重访信号**：新增一个驱动 Tauri CLI 的平台脚本时，必须在那条命令上设 `MSIME_PLATFORM_BUILD=1`，否则它会在 `beforeBuildCommand` 失败；如果出现确实需要独立 Tauri 窗口的正当流程，再评估是否放开 `tauri dev`。

## Verification

- Windows 11：不设变量时 `pnpm tauri build --no-bundle` 与 `pnpm tauri dev` 都在 `beforeBuildCommand` / `beforeDevCommand` 退出 1 并打印错误，没有编译任何 crate；`MSIME_PLATFORM_BUILD=1 node apps/desktop/scripts/require-platform-build.mjs` 退出 0，`MSIME_PLATFORM_BUILD=0` 退出 1。
- `platforms/windows/tests/tools/build_client.ps1` 断言 Tauri 那一次 `pnpm` 调用时变量为 `1`，并在每个失败阶段核对变量已恢复；`python -m unittest platforms/ios/tests/settings/ProjectConfigurationTests.py` 23 项通过。
- 未在 macOS、Android、iOS 上实跑对应的平台脚本。
