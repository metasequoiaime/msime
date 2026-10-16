# Agent Note: Android 长度限制不把拼音组字算进字符上限

Status: implemented

## Problem

#7841：Android 输入框设置 30 个字符上限时，已有 26 个汉字后输入 `nihao`，输入框把拼音预编辑区也交给 `LengthFilter` 计数。编辑器会保留或截短 `ni`，后续按键看起来像不能输入；最终候选「你好」本来只占两个字符，却无法像其他输入法一样填满剩余空间。

根因在 Android 宿主把预编辑写成真实的 composing region。`EditorInfo` 没有公开编辑器的最大长度，`setComposingText` 也可能在超出限制时返回成功但只保留前缀，因此只看返回值无法判断是否被过滤。

## Decision

每次成功调用 `setComposingText` 并结束该批 `endBatchEdit` 后，宿主同步读取光标前最多一段与组字同长的文字，检查它是否以完整组字文本结尾。若编辑器只留下组字前缀，先结束组字标记，再用 `deleteSurroundingText` 删除读回的实际前缀长度，把这次 Engine 组字标记为「编辑器隐藏」：后续拼音仍由键盘自己的预编辑和候选栏显示，直到候选提交或组字结束。提交候选时重新允许 `commitText`，所以最终汉字按编辑器的字符上限计算。

检查和前缀长度推断集中在无 Android 依赖的 `CompositionTextPolicy`，未知编辑器上下文时保守保留原有内联行为。`LengthFilter` 截短时可能同时丢掉 composing span，因此单独调用 `setComposingText("", 1)` 会留下已经变成普通文字的前缀。隐藏状态由当前编辑器会话持有，不能放在每次 `apply` 都重新创建的 `Sink` 里；提交、结束组字与进入新输入框时清除。`SelectionEchoTracker` 按零长度替换记录清空后的实际光标位置，避免异步 `onUpdateSelection` 把这次正常回声当成用户移光标。

Compose 编辑器在批量编辑期间可以缓存写入，读取上下文仍返回上一批的文字。ChatGPT 1.2026.279 的真实输入框（`inputType=0x2c001`）在批内读回长度 0、批后读回完整的首字母；第二笔也分别读回旧的一字母和新两字母。因此截短判断必须在批后进行，不能把批内旧快照当作过滤结果。清理截短前缀另开一批；选区跟踪先记录实际长度的中间回声（允许过滤器保留或丢弃 composing span），再记录清理后的光标，避免两批之间的异步回报取消引擎组字。

设备夹具加入一个延迟到 `endBatchEdit` 才执行写入的输入框，连续输入 `nihao`、候选提交及中英往返都经过安装产品的真实按键路径。

设备夹具加入一个 30 字符的 `LengthFilter` 输入框：先明确聚焦并把光标移到 26 个合成汉字末尾，输入 `nihao` 后输入框保持 26 字，继续输入 `nihao` 仍不写入拼音，空格后得到恰好 30 字的「26 个汉字 + 你好你好」。之后恢复普通输入框，继续验证正常组字与繁体输出。

API 28 的读音节点可能仍返回第一拍缓存的 `n`；这项断言在旧系统上先 `refresh()`，再读取当前组字文本。连续隐藏组字和最终四字上屏仍走真实按键与 `InputConnection`，不通过跳过断言来处理旧缓存。

## Alternatives considered

- **批内读回并立刻判定截短**：能把写入和清理放在同一批，避免中间选区回报，但 Compose 可以延迟写入，ChatGPT 上会把正常拼音结束为原文字母；改成批后验证，并跟踪两批回声。

- **始终不把拼音写进 Android 输入框**：能避开所有编辑器长度过滤器，但会失去现有的内联组字、光标点击和读屏语义，也改变已记录的 Android 宿主契约。
- **只在 `setComposingText` 返回 `false` 时回退**：不够，因为 Android `LengthFilter` 常在返回成功的同时静默截短组字。
- **用空 `setComposingText` 清理截短的前缀**：可复用正常组字的清空路径，但过滤器可能已移除 composing span，这时空写入不会删除残留拼音；改为结束标记后按读回长度删除。
- **尝试从 `EditorInfo` 读取最大长度**：`EditorInfo` 没有这个字段，应用的 `LengthFilter` 也不会把限制传给 IME。

## Consequences

受长度限制的编辑器在剩余空间不足以容纳完整拼音时，键盘仍显示完整组字，输入框暂时不显示拼音；选词后只写入最终文字。正常编辑器继续使用真实 composing region。若编辑器在批量结束后仍无法读回文字，保守采用原来的内联路径。

## Verification

- `platforms/android/check-host.sh`：230 个 JVM 冒烟、110 个设备套件源码编译和 Android 宿主契约检查通过。
- `EditorSmoke` 新增 `CompositionTextPolicy` 的完整组字、截短前缀和未知上下文用例。
- 当前 develop 在本任务独立 API 35 AVD 上复现 `limited composition typing` 失败；本地状态跟踪确认截短后原来的空组字写入留下前缀，并把光标回声误判为外部移动。
- 最终 APK 在独立 API 28 arm64 AVD `msime-client-test-api28-nightly-38639487504`（`emulator-5586`）通过完整 `smoke.sh --core`，包括连续隐藏组字、30 字上屏和剪贴板保存生命周期；API 35 AVD `msime-client-test-api35-nightly-38639487504`（`emulator-5584`）通过同一主回归以及普通组字、繁体输出、密码框直输。两台设备安装的产品 APK SHA-256 与本地最终包一致。
- 修复后本地 quick 门禁通过。没有用共享 AVD 的结果作为本次验证证据。

- ChatGPT 批量快照回归：`BatchCompositionDeviceSmoke` 在官方 0.4.2 的延迟写入夹具上于汉字提交阶段失败；修复后在独立 API 35 AVD 通过连续 `nihao`、汉字提交和中英往返。该用例纳入 `smoke.sh --core`，不依赖外部应用安装。
- 修复后的 Java 宿主在同一 API 35 AVD 通过完整 `smoke.sh --core`，包括普通组字、30 字上限的连续隐藏组字与汉字提交、繁体输出、密码框直输、候选身份保护和剪贴板生命周期。设备包的 JNI 与词库沿用校验通过的官方 0.4.2 发布包，宿主由本分支源码经 Gradle 构建；本次没有重编引擎，也没有重新运行 API 28。
- 真实 ChatGPT 1.2026.279 游客输入框验收通过：按真实键盘逐字输入 `nihao` 并提交「你好」，中 → 英 → 中后再输入并提交，草稿恰为「你好你好」，未发送消息。临时诊断输入法和验收 APK 已移除。
- 本次最终 `scripts/verify-local.sh --quick`、`pnpm run verify-notes` 和 `git diff --check` 通过；quick 包含 Rust 工作区、Android 目标与 Android 宿主 JVM 检查。
