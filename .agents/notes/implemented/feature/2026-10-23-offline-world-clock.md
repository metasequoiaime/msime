# Agent Note: 键盘内的离线世界时钟和时间插入

Status: implemented

## Problem

#9272 要在 Android 键盘内查看系统时间和海外城市时间，选择格式上屏，并离线管理城市。现有日期关键词候选仍要求输入触发词，不能承担这个入口。

旧笔记审计：[日期行内候选](2026-10-10-inline-date-time-candidates.md)和[农历关键词](2026-10-09-lunar-date-keywords.md)只部分重叠，保留各自入口，本功能不修改引擎候选。

## Decision

共享 `client-core::world_clock` 持有城市目录、时间格式、时区换算、太阳事件和持久化；host-api 提供有界 JSON 请求，Android 只负责面板、城市名称本地化和生命周期。显式使用 Jiff 的 `TimeZoneDatabase::bundled()`（内嵌规则为 2026c），不联网、不请求定位权限；城市经纬度来自固定 IANA `tzdata2026a/zone.tab`，保留原始文件和许可证，额外补北京。

系统卡片始终使用宿主提供的当前系统偏移，时区对应的目录城市仅作为日出日落的参考，明确写出参考城市，不能声称是当前位置。未找到对应城市时明确提示无法提供参考，不伪造。海外卡片使用 IANA 时区的动态偏移；太阳事件使用当地日期、城市坐标、海平面近似并处理极昼极夜；日期变更线附近会检查相邻的计算日期，只保留换算后属于目标当地日期的事件。

城市最多 32 个，去重并验证目录 ID；独立 `WorldClock.json` 在共享偏好目录通过私有文件入口、作用域锁和原子替换保存。损坏文件报错并保留。加载和写入走已有工作线程，定时刷新只更新本面板，不每秒调用全局 render；关闭或换编辑器后丢弃过期回调。定时快照忙碌时将点按、插入和删除排队，完成当前请求后依次执行；关闭面板清空队列，不能因碰上每秒刷新而吞掉操作。

功能面板新增「世界时钟」。系统和城市卡片轻点打开日期、星期、时间、日期时间、ISO 8601、Unix 时间等格式，点格式插入当时的新值。添加城市按地区、国家和本地化城市离线选择，不在输入法内打开需要重新聚焦的输入框；长按城市后显示删除选项。

## Alternatives considered

- `chrono-tz` 提供成熟的静态时区类型，最初实现的边界测试也通过。但 0.10.4 内嵌规则仍是 2025b，因此采用工作区已有 Jiff 与 2026c 内嵌数据库，显式选择 bundled，不能受系统数据库和 `TZDIR` 环境变量影响。

- 使用 Android `java.time` 完成全部规则：可直接使用系统更新的时区库。但持久化、格式和太阳计算会留在宿主，其他平台无法复用。保留系统时区偏移的适配，核心换算用共享库。
- 只内置几个常用城市：界面最简单、数据少。但无法满足全球用户按国家添加城市，采用覆盖 IANA 各国家/地区的目录和逐级选择。
- 从系统时区推断真实位置：打开即可给出太阳时间。但同一时区跨度很大，上海的日出不代表北京，必须标注参考城市。

## Consequences

键盘内可免触发词离线查看和插入时间。城市目录每个时区只有代表城市，并非地球全部城镇；日出日落是参考坐标的近似，不使用地形和定位。时区法律变化需升级内嵌数据库。城市列表有数百项，用逐级选择控制显示规模。共享库依赖影响所有目标，通过 quick 检查跨目标编译覆盖面。

## Verification

核心库完整测试 513 项、库级 clippy 和 host-api 世界时钟专项 1 项通过；日期变更线回归能在旧实现中复现偏差。桥接专项发现无参数操作忽略多余字段，改为空结构变体并加入回归。

API 28/35 上的 client-base 36 项、client-core 512 项和 client-providers 254 项通过，包含世界时钟 8 项。API 28 的 client-services 1240 项通过；API 35 为 1239 项通过，`loopback_relay_bypasses_the_system_proxy` 一次失败，随后单项复跑通过，不能称整套一次全绿。

操作排队修复后的 APK 已构建并签名，ELF 对齐、依赖和世界时钟导出检查通过；quick 的契约、actionlint、Rust workspace、Android 与 WASM 目标、Android 宿主 Java、macOS 原生宿主及共享 Apple bridge 均通过。当前机器缺少 Docker、MinGW、DevEco 和 PATH 中的 Node/pnpm，对应阶段按脚本跳过；笔记在本机单独校验通过。iOS 资源暂存与 XcodeGen 工程生成通过，未据此声称 iOS 产品实测。

设备专项发现快照忙碌时吞点按/删除后，已改为排队。同一产品 APK 在远端 API 28 和本机 API 35 均通过完整 `WorldClockDeviceSmoke` 与 `MoreToolsDeviceSmoke`，覆盖实时刷新、添加东京与纽约、ISO 与星期插入、重启持久化、长按删除、删除后重启和返回键盘。两版最终截图已实际检查，确认系统卡片、城市时间和参考城市说明。

夹具通过系统 Home 收起键盘、原生 Intent 启动新合成编辑器，等待窗口事件与全部交互窗口中的可访问控件就绪，不能假设活动窗口始终属于编辑器。API 30 以上沿用其他设备套件的 `ime-requests-settled` 标记，等编辑器的延迟显示请求结束；刷新节点后再判断 Q 键可见性与可点击性，菜单展开后清理旧节点并确认面板。仅对输入法注册准备作有限重试，不重试功能断言。

远端 API 35 曾在编辑器、键区或菜单入口准备阶段失败，未进入时钟功能断言；未改动的 MoreTools 基线也报 `IME showed []`。系统记录包含 `system_server` 的 `PointerEventDispatcher0` ANR、Launcher 焦点超时及合成编辑器 `failed to attach / start timeout`。保留日志和截图后，改用本机专用 API 35 AVD，使用已有 APK、不重新构建，两个完整 UI 专项均通过。不能仅根据 `am instrument` 的退出码判断成功，还必须检查 `MSIME_DEVICE_SMOKE_PASSED` 与失败标记。

PR CI 发现 Windows 许可收集合成仓库漏建新声明，已补夹具并通过 PowerShell 专项。macOS `settings-glass` 曾一次外观同步断言失败，该单项复跑和新一轮完整 CI 均通过。代码提交 `3d73305284` 的 PR CI 已完成：28 项通过、6 项按条件跳过，无失败；后续提交仅同步最终验收记录。

同步 `70590fe035` 后，`verify-native.sh` 同时保留上游热键导出 `msime_client_toggle_hotkey_chords`、世界时钟 C API `msime_client_world_clock` 与 JNI `worldClockRaw`。冲突只标在 C API 长行，JNI 行仍有已自动合并的本 PR 改动；恢复整份上游文件会误删该行，首次 quick 的 `android jni names` 已拦住这个遗漏。补回后对两份导出清单分别检查上游与旧分支的完整并集，156 个 JNI 方法、127 个 C API 导出的契约专项通过。新基线世界时钟 8 项测试和 client-core 全目标 Clippy（`-D warnings`）通过，旧基线的 Clippy 限制已不存在。本次不改世界时钟运行逻辑；设备结果仍对应前述已验收 APK，不把新基线编译当作重跑设备验收。

补齐 JNI 清单后的完整 `bash scripts/verify-local.sh --quick` 通过（`quick-exports-restored.exit` 为 0），包括 176 项 Windows 可移植测试、全部契约、Rust workspace、Android/WASM 目标、Android Java/JNI、macOS 原生宿主和共享 Apple 桥接构建。Docker、MinGW、DevEco 与远端 PATH 中的 Node/pnpm 仍缺少，对应阶段按脚本跳过；本机笔记与引导文案检查单独通过。本次新检出在远端系统卷构建，开始前空闲 62 GiB，本机未创建构建树。
