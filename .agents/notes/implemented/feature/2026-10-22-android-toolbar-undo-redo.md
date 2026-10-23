# Agent Note: Android 工具栏委托编辑器撤销与重做

Status: implemented

## Problem

#8654 要求在键盘工具栏设置中开启撤销、重做，并在键盘顶部直接操作。截图指明两个独立按钮，不能用文本编辑面板入口替代。

已检索活跃笔记中的撤销、重做、工具栏和排序。[工具栏排序](../../implemented/feature/2026-10-15-android-toolbar-order.md) 部分重叠，沿用稳定 ID、遗漏项追加、两端固定和原生设置入口；剪贴板删除撤销、手写撤销不持有编辑器历史，与本项无关。

## Decision

两个开关由 `client-core::TouchToolbarPreferences` 定义并校验，默认关闭，经现有偏好保存入口持久化；Android 设置只适配界面，复用工具栏排序和预览。现有账号同步工具栏成员表不包含这两个字段，本项不扩展同步契约。共享 React 设置页没有 Android 工具栏入口，字段一致性门禁在 `RUST_ONLY` 按原生设置归属登记这两个字段。按钮向当前输入连接发送 `Ctrl+Z` 和 `Ctrl+Shift+Z` 的完整按下/松开事件，由编辑器持有历史；不读取或保存编辑器内容。动作前结束组字、使选区回声失效，密码框及无连接时禁用。

设置明确提示当前应用需支持撤销和重做。异步连接接受按键不代表编辑器完成动作，因此不显示成功提示，也不基于未知历史禁用按钮。不同时发送菜单命令和快捷键，避免同一次点击撤销两步。

参考 FlorisBoard 固定提交 `fe1241f4921b3eae923571ff7a3e113a7e10d677` 的 `ImeEditor.performUndo/performRedo`，仅核对公开系统协议，不复制源码或图标。

## Alternatives considered

- 自建编辑历史能绕过部分编辑器不支持快捷键的问题。但需要读取、保留和重放用户文本，难以与应用自身修改、选区和焦点切换同步，且会扩大密码与跨输入框恢复风险，因此不采用。
- `performContextMenuAction(android.R.id.undo/redo)` 可直接调用标准原生编辑器的菜单动作，但自绘和网页编辑器通常使用键盘快捷键；采用参考输入法使用的快捷键协议，不串联两套协议试探。

## Verification

- 共享偏好测试覆盖旧文档缺字段、默认关闭、开启后的保存重读、非法字符串拒绝；`cargo fmt --all --check` 和 `cargo clippy -p msime-client-core --locked --all-targets -- -D warnings` 通过。
- API 28、35 的 `ToolbarEditorDeviceSmoke` 默认完整套件及 `history-only` 切片通过：从原生设置开启和关闭、保存按钮顺序、重启输入法后保留；实际系统键盘输入 `nihao` 并提交为 `你好`，撤销回 `nihao`、重做回 `你好`，删除后撤销和重做均核对真实文字。系统编辑器定义一步的边界，不要求一次撤销清空整个词。另验证新编辑清除重做分支、跨框隔离、不支持快捷键的编辑器保持为空、密码框禁用。
- 两台设备的 `ToolbarEdgesDeviceSmoke` 通过：无连接安全禁用，既有 240/320/600 dp 布局、固定两端、滚动、排序、计算器和剪贴板顶栏保持通过。
- 两台设备均经 `build-core-test.sh` 在正常应用 UID 运行四个客户端 crate：client-base 36 项、client-core 484 项、client-providers 247 项、client-services 主进程 1236 项及 4 个子进程专项通过；保留套件原有 ignored，SELinux 下不适用的 3 个 hard-link 用例按 runner 过滤。
- 原生 APK、JNI/API 检查、255 项 JVM 冒烟和设备测试源编译通过；按需下载配置的签名 full APK 为 130867586 字节。
- 通用 `smoke.sh` API 28 运行到 `ClipboardSegmentDragDeviceSmoke` 的全文预览断言失败；API 35 运行到 `CompositionRestartDeviceSmoke` 的重建组字断言失败。两个测试源均未修改。用固定基线 `61ed5bc3e4c6a79b306431e65256b651363bdadd` 的完整 APK 对照，API 28 在相同全文预览断言失败；API 35 完整通过。恢复修改版的已签名 APK 后，首次复跑通过原失败处，但在切换双拼时缺少输入法窗口；同一 APK 再次运行完整组字重启与放大镜套件通过。该套件本轮结果不稳定，未定位根因，不能把首次失败归为基线问题，也不将整个 `smoke.sh` 报告为全通过。
- 首次 quick 只在偏好字段一致性检查失败；补充 Android 原生入口归属登记后，远端 `bash scripts/verify-local.sh --quick`（run `20261022-121500`）退出 0，包含 111 项 Linux 原生测试、Android/Wasm/Rust 编译、macOS 原生与共享 Apple 桥构建。未 staged 的 macOS Tauri bundle、缺少前置的 HarmonyOS 打包和 Windows 原生交叉构建仍按脚本跳过，不把 quick 当作这些目标的运行验收。`pnpm run verify-notes` 和 `git diff --check` 通过。

## Consequences

编辑器决定合并粒度和支持情况；快捷键投递成功无法查询可撤销步数。设备验证需检查实际文字变化，不能只断言事件发送成功。
