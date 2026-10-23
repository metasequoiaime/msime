# Agent Note: Android 为不区分组字的搜索框保留键盘内拼音

Status: implemented

## Problem

#9192 报告盛京银行 7.2.3 功能搜索在拼音第一个字母时立即触发，组字和最终文本都被当作搜索条件。现有 Android 宿主逐键调用 `setComposingText`；诊断包没有崩溃、性能或输入事件记录，不能据此确认银行内部监听的实现。[应用宝的银行发行页面](https://sj.qq.com/appdetail/com.shengjingbank.mobile.cust)对应包名 `com.shengjingbank.mobile.cust`。

## Decision

仅在该精确包名、全拼或双拼中文组字时，让 `composingText` 返回空串，沿用现有 14 键等布局的键盘内组字路径。引擎继续维护拼音与候选，候选栏按实际未写入编辑器的状态显示拼写；选词或明确确认时仍经 `EditorBridge` 提交最终结果。专用英文、本地模式和其他语言继续原行为。每次根据当前编辑器包名判断，不增加持久化文件、全局开关或应用枚举。

## Historical audit

- [触屏拼写入框](../../implemented/feature/2026-10-15-touch-composition-in-field.md)部分重叠：默认行内组字继续成立；原先没有具体不兼容应用的假设已被 issue 报告补充，例外限定为该应用。
- [组字被截短](../../implemented/bug-fix/2026-10-15-android-composition-length-filter.md)部分重叠：不复用截短后的清理机制，避免先写拼音再删除导致应用已触发搜索。
- [同编辑器重启](../../implemented/bug-fix/2026-10-19-android-same-editor-composition-restart.md)部分重叠：没有组字入框时不尝试恢复编辑器标记，普通应用的重启保护保持原契约。

## Alternatives considered

- **全局行内组字开关**：能让用户自行适配其他应用，但翻转既有产品决定并改变全部应用的体验；本 issue 有具体应用，使用定向例外。
- **先写再清除拼音**：能复用长度过滤回退，但搜索监听已经收到第一次文字改变，无法解决本问题。
- **推断所有搜索框都不支持组字**：不依赖包名，但正常应用的搜索框也支持规范组字，不能由输入类型判断是否不兼容。

## Verification

专用 API 28、35 AVD 中，未修复原生 full APK 均在首字母处触发合成搜索框文字变化；确认前不写入的断言失败。最终同包名无联网夹具均通过全拼、九键与双拼检查：逐键无文本改变或 `setComposingText`，拼音在候选栏可见，删除只改键盘，选词后只有一次提交且保留合成前缀，再次组字不改已提交文本。切换同控件的普通包名后恢复行内拼音，验证例外不跨应用泄漏。API 28 基础词组与删除、API 35 基础词组、繁体、删除及密码框直输也均通过。

API 35 首轮夹具没有适配系统栏，位于顶部的计数器不可见；沿用 `WindowLayout.fitSystemBars` 后进入正确的首字母红例。失败记录保留，不将它计作产品缺陷。完整 quick 门禁、JVM 策略检查、设备测试源码编译、笔记门禁与差异检查均通过。四个生产与设备测试源文件的 SHA-256 与实际构建镜像一致，自建 AVD 已关闭。

复现方式：构建原生宿主后，以 `MSIME_ANDROID_APP_COMPOSITION=1` 构建 `tests/device/build-editor.sh`，在专用 AVD 安装生成的 `composition-search-editor.apk` 和 `editor-test.apk`，执行 `adb -s <serial> shell am instrument --no-restart -w -e app-composition true app.msime.android.test/app.msime.android.test.CompositionRestartDeviceSmoke`（API 28 去掉 `--no-restart`）。相同环境变量下运行 `tests/device/smoke.sh` 也会安装并执行专项，`--core` 包含它。

## Consequences

同包名合成编辑器验证输入法策略和系统路由，不等于真实银行 7.2.3 与 Redmi 实机验收。规则覆盖该应用的全拼、双拼编辑器，不推断字段用途；如银行修复，需要明确移除例外。测试夹具只在显式构建开关下生成，专用 AVD 使用，不能安装到用户手机或替换真实银行应用。
