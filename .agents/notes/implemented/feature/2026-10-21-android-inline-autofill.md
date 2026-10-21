# Agent Note: Android 键盘内联自动填充

Status: implemented

## Problem

#8973 希望密码等支持自动填充的输入框，将填充选项显示在键盘上，类似现有最近复制提示。宿主没有声明内联填充能力，也没有创建请求或接收系统返回的建议。

## Decision

Android 11 起声明并实现系统内联填充接口，向填充服务提供最多五项的显示规格和 AndroidX V1 样式。系统返回的 `InlineContentView` 直接放入工具栏所在的等高横向滚动行，优先于最近复制和普通工具栏。密码框和隐私模式可以显示系统托管的填充界面；不把它等同于会读取、存储文本的剪贴板功能。输入法不读取凭据内容、不通过 `InputConnection.commitText` 重放填充。

新输入框、结束输入、隐藏键盘及销毁服务时清除建议和待完成的异步装载代次，迟到的视图不能进入下一输入框。空响应恢复原工具栏。输入视图重建后重新挂接当前已装载视图，避免同一个系统建议重复 `inflate`。挂载时保留 `inflate` 回调写入的实际宽高：`InlineContentView` 内部使用 `SurfaceView`，改成 `WRAP_CONTENT` 会使宽度归零，出现空白建议行。系统 11 以下不调用新 API，保留系统原有菜单。

原生包和不发布的开发合包共用同一组声明与依赖，开发合包的 R8 保留反射适配入口。用官方 AndroidX Autofill 1.3.0 生成版本化样式，不手工复制其 Bundle 私有键。AndroidX 样式适配沿用 `HandwritingRecognizerFactory` 的反射装配边界，使现有不解析 AAR 的纯 SDK 宿主门禁仍能编译服务；正式 Gradle 构建、设备请求和真实填充测试验证实际实现，缺实现时回退系统菜单。

## 历史决策审计

- [最近复制工具栏](../../implemented/feature/2026-10-08-android-recent-clip-toolbar.md) 部分重叠：沿用等高替换的位置，保留其剪贴板历史开关与隐私禁止规则，自动填充优先时暂不展示最近复制。
- [文本编辑中的复制提示](../../implemented/feature/2026-10-15-android-recent-copy-in-text-edit.md) 部分重叠：保留面板中的剪贴板规则，填充视图不进入该面板。
- 云剪贴板令牌隐藏笔记只讨论设置页录入令牌，与系统自动填充会话无共同决定，保持不变。

## Alternatives considered

- 只声明 `supportsInlineSuggestions`。改动小，却没有请求和显示实现，不能产生可用填充入口，放弃。
- 自己绘制文本按钮并调用 `commitText`。能复用剪贴板按钮，但会让输入法接触凭据、失去系统的认证和多字段填充，放弃。
- 手写 AndroidX 样式 Bundle。免于新增依赖和装配边界，但依赖私有键并复制协议实现，采用官方固定版本适配。
- 与候选同时额外增加一行。可以一直显示，却改变现有键盘高度，采用空闲工具栏位置等高替换。

## Consequences

支持的填充服务可以在键盘上提供系统托管的凭据入口，沿用工具栏高度和现有皮肤颜色，不把凭据复制进候选或剪贴板。代价是新增固定版本的 AndroidX 依赖、反射装配和系统异步视图的生命周期管理；建议显示时普通工具栏及最近复制暂时隐藏，展开面板和收起键区时不显示建议。系统必须启用支持内联的填充服务，应用输入框也需要提供填充信息，不能保证任意密码管理器或任意应用都有建议。样式不兼容时不创建请求，由系统显示旧菜单；服务或单项装载失败时不记录填充响应。

## Testing

- `check-host.sh` 通过：宿主编译、JNI 契约及 253 项 JVM 冒烟。`InlineAutofillGenerationSmoke` 覆盖输入前拒绝、响应替换、切框、隐藏及再次显示不能复活旧代次。
- Gradle `assembleFullRelease lintFullRelease` 通过。测试 APK 的资源也由真实 `aapt2` 编译和链接；六份产品 `method.xml` 和六份开发合包 `tauri_method.xml` 同步声明能力，最终 APK 中核对 AndroidX 许可和来源通知。
- API 35 专用模拟器的 `inline-autofill.sh emulator-5580` 通过：在普通非填充框中打开真实键盘，再点击用户名框自动触发请求；官方系统渲染器在键盘区域显示合成建议，隐藏／再显示、切到非填充框、切到密码框、空响应和点击后用户名／密码一起填入均通过。测试不直接调用产品回调或 `commitText`。截图目测合成建议文字和键盘正常，已合入的密码框自动数字行与填充建议同时显示；用 `WRAP_CONTENT` 覆盖系统实际尺寸的实现会在远程文字验收处失败，截图为空白行。
- API 28 专用模拟器的 `inline-autofill.sh emulator-5566` 通过：真实键盘正常启动，系统旧菜单显示合成建议，点击后用户名和密码同时填入，原填充服务恢复。新接口声明没有导致旧系统加载或验证错误。
- `bash scripts/verify-local.sh --quick` 与本机 `pnpm run verify-notes` 均通过。quick 包含本次修改涉及的 Rust Android 编译、宿主 Java/JVM 及契约检查；无 Docker、MinGW 环境的跨平台门禁明确跳过，未把它们记为验证通过。

测试自身通过 `settings` 恢复缺失值和非空白名单组件，通过 Settings 提供方的 `PUT_secure` 恢复空字符串；`UiAutomation.executeShellCommand` 不解析 shell 引号，不能给组件或空字符串套单引号。每次设备验收先覆盖三种状态并精确读回，结束时再次核对原值；外层脚本的 trap 仅作中断兜底，不能掩盖 instrumentation 的恢复错误。

测试包已达系统允许的 100 项 instrumentation，专项用环境变量替换默认入口，不新增第 101 项。合成提供方同时提供旧菜单和公开 AndroidX V1 Slice 载荷，不读取真实账号；脚本和测试的退出路径都恢复原填充服务，并保留空字符串与缺失设置的区别。测试包使用开发密钥，未做发布签名安装验收，开发合包的 Gradle 配置评估和配置契约检查通过，受构建机系统盘余量限制未继续编译或安装完整合包，不作为产品设备验收。真机、真实第三方密码管理器及输入视图重建时的迟到装载未实测，代次边界由 JVM 回归覆盖。

## References

- [Android 官方 IME 自动填充说明](https://developer.android.com/identity/autofill/ime-autofill)
- [AndroidX Autofill 固定发布版本](https://developer.android.com/jetpack/androidx/releases/autofill#1.3.0)
