# Agent Note: HarmonyOS 手机设置页的文本框行、选择行与滚动容器

Status: implemented

## Problem

HarmonyOS 手机的「语音输入」页（共享设置页 `packages/ui`，宿主在 `platforms/harmony` 的 Web 组件里加载）有两类问题，模拟器（HarmonyOS 6.0.0.130，API 22，1216×2688）上用 develop `6dbe64b368` 打包复现：

- 排版。文本框（`TextInputRow`、`EndpointInput`、`SecretInput`）没有任何手机样式，是浏览器默认的方框；`Row` 把控件放在标题旁边且 `shrink-0`，约 375 CSS px 宽的屏幕上说明被挤成几个字一行、接口地址被截断。「流式接口」「识别服务」在 `Row` 里直接放原生 `<select>`，绕过了 `SelectRow` 在 HarmonyOS 上改成选择面板的路径，显示成行尾的 ∨ 下拉框。豆包 API Key 用旧的 `SettingField`（`label.section-header`），放在新分组里没有行内边距，贴着卡片左边。「可用录音设备」是旧的标题加下拉框加默认样式按钮。
- 白屏。滚到「语音输入」页底部后继续上滑，整个页面（含底部标签栏）消失，只剩灰底，回滚也不恢复。开 Web 调试后用 DevTools 协议读到：`document.documentElement.scrollHeight` 是 2380，窗口高 761；白屏时 `scrollY` 是 1595，`#root` 的 `top` 是 -1595，看到的灰色是 `body` 的背景 `rgb(243,243,243)`。撑高文档的是 `PickerRow` 背后包着原生控件的 `<span class="sr-only">`：`sr-only` 是绝对定位，祖先里没有定位元素，包含块是初始包含块，于是这些元素按它们在滚动内容里的静态位置（y 两千多）相对文档定位。设置页的设计是只有内容区 `<main>` 滚动、文档不滚；`<main>` 滚到底后滑动链到文档，把整个应用滑出屏幕。主设置页底部没有选择行，文档不会被撑高，所以只有长页面出现。

## Decision

- `settings-style.ts` 的内容区滚动容器 `content` 加 `relative`，成为页面里绝对定位元素的包含块。文档高度回到窗口高度，滚到底继续滑动不再有可以滚的文档。`fixed` 的覆盖层（选择面板、对话框）不受 `position: relative` 影响。
- `Row` 新增 `wideControl`：HarmonyOS 手机上整行 `flex-wrap`，控件区占满一行，放到标题和说明下方；其余平台与原来相同。`TextInputRow`、`EndpointSettingRow`、`SecretSettingRow` 打开它。
- `platform-controls-style.ts` 新增 `textInput`、`secretInput`、`secretToggle`，只带 `harmony:` 变体：文本框是 40px 高、无边框、比分组底色深一层的胶囊（与词库页搜索框同款），「显示」是行尾强调色文字。`TextInputRow` 在调用方没有传 `className` 时用它，`EndpointInput` 和 `SecretInput` 直接用它。
- 带说明的 `PickerRow`（只在 HarmonyOS 上出现）改成网格：标题和当前值在第一行，当前值占标题右边剩下的宽度、靠末端对齐、放不下时截断，说明在第二行占满整行。没有说明的选择行不变。
- 「流式接口」`DoubaoStreamEndpointSection`、「识别服务」`VoiceProviderRow`、「预置模型」`ModelSettingField`（HarmonyOS 分支）改用 `SelectRow`。选项要作为直接子元素传入（`SelectRow` 只认直接的 `<option>` 和 Fragment），所以选项列表改成直接调用的普通函数。`ProviderPresetSection` 在 HarmonyOS 上把预置模型这一行放在 `managerBlock` 容器外，避免行内边距叠两层。
- 分组里的凭据行改用已有的 `SecretSettingRow`：语音识别的 `VoiceCredentialFieldsSection` 和文本润色的 `PolishCredentialFieldsSection`（接口地址同时改用 `EndpointSettingRow`）。只在 Linux 上出现、不在分组行里的 `AiCredentialSection` 和语音服务凭据表单仍用 `SettingField`。
- `VoiceDevicePicker` 改成分组里的两行：`ActionRow`「读取录音设备」（说明是读取状态，按钮「刷新设备」），`SelectRow`「可用录音设备」。占位项「选择设备」加 `hidden`，不出现在选择面板里。

## Alternatives considered

- 只在文档层面挡住滚动：`html, body { overflow: hidden }` 或给 `<main>` 加 `overscroll-behavior: contain`。理由最强的一条：一行 CSS，直接让「滑出屏幕」这个症状不可能发生，也不用推敲哪些元素会受影响。没采用：文档仍然比窗口高，只是不让用户滚；焦点移到某个 `sr-only` 控件时浏览器会为了把它滚进视口而滚动文档，同样把应用推出屏幕。`relative` 去掉的是文档被撑高这件事本身。
- 把 `sr-only` 的包裹元素各自放进一个 `relative` 的父元素（例如 `PickerRow` 外层的 `div`）。理由：改动只落在出问题的组件上。没采用：页面里还有别的视觉隐藏控件（分段控件、复选框的 `sr-only` 输入）也是同样的定位方式，逐个补会漏；滚动容器是它们共同的、也是语义上正确的包含块。
- 文本框在所有平台统一换成新样式。理由：一套外观，少一个平台分支。没采用：桌面各平台的文本框跟随系统控件外观，这次只在 HarmonyOS 手机上复现了问题、也只在那里验证了新外观；样式用 `harmony:` 变体限定。
- 新建一个 `SecretInputRow`。实现过程中先写了一个，随后发现仓库里已有行为相同的 `SecretSettingRow`，删掉新组件改用旧的。

## Consequences

- HarmonyOS 手机上，文本框、密钥框和接口地址各占一整行；带说明的选择行说明占满整行。代价是这些行比原来高一些，页面更长。
- 桌面平台的变化：「流式接口」从未加平台样式的原生 `<select>` 变成 `Select`（带平台外框）；文本润色的接口地址和 API Token、语音识别的 API Key、「可用录音设备」从旧的 `SettingField` 标记变成 `Row`，与所在分组的其他行一致。桌面上没有逐页截图核对。
- `content` 成为定位元素后，页面里以后新加的绝对定位元素都相对内容区定位、随内容滚动。需要相对窗口固定的元素要用 `fixed`。
- 往 HarmonyOS 手机设置页加文本框时用 `TextInputRow`、`EndpointSettingRow`、`SecretSettingRow`，或者给自写的 `<input>` 加 `textInput`；加选择时用 `SelectRow`，不要在 `Row` 里直接放 `<select>`。
- 「语音快捷键」分组在手机上列出 Ctrl+F9、右 Alt、Ctrl+Win 等实体键盘快捷键，「更多选项」的箭头紧跟文字而不在行尾，这次没有动。

## Verification

- `apps/desktop` 下 `pnpm run typecheck` 通过，`pnpm exec vitest run` 全部 523 个文件、2756 个用例通过。`tests/settings/voice-handwriting-harmony.test.tsx` 新增三条：「识别服务」「流式接口」从选择面板选择并写回 `asr_endpoint`；API Key 和录音设备是分组的行；滚动容器带 `relative`。把 `relative` 去掉，第三条失败。
- `tests/dictionary/desktop-cloud-dictionary.test.tsx` 在机器负载高时偶发失败（点击「完整目录」时按钮还处于 `disabled`），与本改动无关：在 develop 和本分支上交替各跑 8 次都通过。
- 模拟器上逐页截图核对了「语音输入」「输入」「键盘」「词库」「AI 辅助」，并在「语音输入」页底部连续上滑 10 次，页面保持显示，DevTools 读到 `scrollHeight` 等于窗口高度。没有在真机上验证，原生库是 10-8 的构建（词库页的「词库操作失败」来自它，与本改动无关）。
