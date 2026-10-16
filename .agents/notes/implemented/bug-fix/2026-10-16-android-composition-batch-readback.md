# Agent Note: Android 在批量编辑结束后验证组字长度

Status: implemented

## Problem

用户反馈 Android 0.4.2 在 Iceraven 地址栏输入中文时首字母直接上屏、候选消失，而 0.3.1 正常。两个发布提交分别为 `9bd282f753c4c0e3e7a8ffa75ab69737654e5162` 与 `b5c1b46ad049480420c9501e3bd85ac7fbc2ed26`；URI 默认英文策略在两个版本都存在，不能据此解释回归。

0.4.2 增加的长度检查在 `beginBatchEdit` 与 `endBatchEdit` 之间调用 `getTextBeforeCursor`。Compose 的输入连接会排队编辑命令，到批量结束才应用；批量内读取旧快照会让首字母被误判为零长度，提前结束组字。这是独立可复现的宿主时序缺陷。官方 [Iceraven 2.50.0](https://github.com/fork-maintainers/iceraven-browser/releases/tag/iceraven-2.50.0)（arm64）与官方水杉 0.4.2 在 API 28 上复现：URI 初始英文，明确切中文后输入 `n`，候选短暂出现，接着候选消失；继续 `i` 得到普通文本 `ni`。用户未提供其具体版本，不能据此覆盖所有历史浏览器配置。

## Decision

`EditorBridge.apply` 在结束批量编辑后调用可返回失败的 `Sink.verify`。Android sink 只在本批成功写入非空组字时读回长度；结束组字、提交与失焦清理不做长度探测。真正截短时另开一批结束标记并删除残留前缀，记录截短回声和清理回声，继续保留键盘侧候选。普通空组字写入也记录零长度替换，避免组字删掉后仍预测旧的光标位置。

## Historical note audit

- [长度过滤回退](../../implemented/bug-fix/2026-10-15-android-composition-length-filter.md)：部分重叠；保留回退机制，本篇替代其批量内同步读取与单次回声的时序决定，两篇互链。
- [默认输入模式](../../implemented/bug-fix/2026-10-08-android-no-suggestions-default-mode.md)：相关但不变更；URI 初始英文与本问题分开验证。
- [连接失败恢复](../../implemented/bug-fix/2026-10-15-android-input-connection-recovery.md)：部分重叠；验证阶段拒写通过布尔结果进入现有编辑器失败恢复路径。

## Alternatives considered

- 直接移除长度检查：最容易恢复旧行为，但会重新引入 30 字符限长框无法连续组词的问题。
- 在 `Sink.end()` 中读回并清理：能省去一个接口方法，但 `end()` 还服务于失焦清理，且没有布尔结果，只能把正常拒写改成异常。单独的 `verify()` 只在正常写入批次成功结束后运行，拒写可直接返回失败。
- 改 URI 默认中文：能够改变进地址栏的初始键盘，但两个发布版本已有同一 URI 策略，不能修复批量读旧状态的缺陷。

## Verification

批量缓冲连接上连续 `n`、`ni`、候选提交与新组字不提前结束。真实截短（有或没有 composing span）清理后不残留拼音，自身回声不取消组字。拒写、未知上下文及普通原生输入框仍按原契约运行。设备 `CompositionBatchDeviceSmoke` 使用安装产品的真实 `EditorBridge` 和 sink；官方 0.4.2 在 `deferred first letter` 稳定失败。JVM `EditorSmoke` 验证读回在批量结束之后、验证拒写/断开向上传播、失焦结束组字不运行探测。

- 修复包在独立 API 28（`emulator-5566`）与 API 35（`emulator-5584`）通过安装产品的批量缓冲连接回归，覆盖排队写入、连续组字、提交后新组字、空组字清理、两种截短 span 状态、零剩余空间、未知上下文与拒写。
- API 28 的真实 Iceraven 2.50.0 地址栏：明确切中文后输入 `n`，延迟观察仍有组字下划线和候选；继续输入 `ihao` 后保留 `nihao` 组字及「你好」候选，选词后地址栏为汉字，无残留拼音。
- 这次验证包由本分支原生 Java 宿主工程构建，复用官方 0.4.2 APK 内的词库、资源和 arm64 原生库，隔离 Java 时序改动；不把它称为本分支 Rust 原生库的整套设备验收。APK SHA-256 为 `374ceb838be7b5c0dc6bf060d61b571b86c9c2ad3b9aad19306a3a0ac2bb935e`。
- 最终 `check-host.sh`：236 个 JVM 冒烟、133 份设备套件源码编译、JNI 语法与 Android 宿主契约检查通过。`pnpm run verify-notes` 与 `git diff --check` 通过。
- API 28 的最终 `CompositionBatchDeviceSmoke` 还用平台 `EditText`、真实 `InputConnection` 和 `LengthFilter(30)` 验证截短清理、隐藏后续组字与最终四字填满，不依赖无障碍节点。
- API 28 的 `DeviceSmoke` 在重新绑定输入法后通过，包含真实原生输入框的拼音组词、选词、删除、连续隐藏组字与 30 字限长上屏。首次运行在过载环境中因无障碍节点失效退出；重新绑定后整项通过。
- API 35 的安装产品批量回归通过；其额外 UI 验收遇到系统 `System UI isn't responding`，不能记为完整 UI 冒烟通过。本任务没有改文件操作或 Android 资源，不声称跑过四个 Rust crate 的设备单测。

- `bash scripts/verify-local.sh --quick` 通过：契约检查、Rust workspace、Android 目标、Android Java 宿主与共享 Apple 桥接编译均通过；其他平台按改动范围跳过。

## Consequences

读回仍需一次跨进程同步查询；此次只改变顺序。清理拆成第二批后可能有两条选区回声，两种状态都须跟踪。缺少用户具体浏览器版本时，已复现的环境范围必须明确。批量结束抛异常时不再继续读回，直接返回编辑器失败；失焦清理仍尽力结束。

### ChatGPT 场景回归

ChatGPT 1.2026.279 的实际输入框同样会在批内读回旧快照。补充独立 Activity：将组字和提交写入排队到最外层 `endBatchEdit`，由真实系统输入法逐字输入 `nihao`、提交「你好」，再中英往返并再次提交。相比直接调用 sink 的回归，它同时覆盖系统输入连接、键盘按键、引擎候选与模式切换，接入 `smoke.sh` 的核心验收。保留现有 `Sink.verify` 实现，不再引入服务私有的第二套读回路径。

该端到端夹具在官方 0.4.2 上于汉字提交阶段失败，在本次独立修复实现上通过；同一实现曾在 API 35 跑完核心设备验收，并在真实 ChatGPT 游客输入框得到合成草稿「你好你好」，未发送消息。这些运行发生在整合已合入的 `Sink.verify` 实现之前，不能当作整合结果的设备验证。
