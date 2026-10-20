# Agent Note: Windows 原生设置窗口自己画「AI 辅助」页

Status: implemented

## Problem

原生设置窗口的「AI 辅助」是 `PageHost::Shell` 页：只有一张「打开」卡片，用 `MSIME.exe --route=settings:ai` 拉起共享 Tauri 应用。整项工作的目标是任何设置都在原生窗口里改。共享页（`ai-settings-content.tsx`、`ai-settings-page-section.tsx`）在 Windows 上有：启用开关、服务提供商、按接口地址来源键存的 API Token、预置模型与接入说明、模型、获取模型列表和已获取模型、「更多选项」里的接口地址、「测试 AI 辅助配置」、候选数量、提示词方案和所选槽位的提示词、AI 润色测试。上一步已经备好 C 接口（见 [原生设置窗口 AI 页用到的 C 接口](../architecture/2026-10-11-windows-settings-ai-host-api.md)）。最后一组「AI 对话」只在宿主提供 `client.chat` 时出现；`apps/desktop/src/main.tsx` 只给 Android、iOS 接 `chat`，Windows 的设置应用没有这一组。

## Decision

- `SettingsNavigation.h` 里 `ai` 改为 `PageHost::Native`，`shell_page_description` 去掉 `ai`，`build_page` 调新的 `build_ai_page`。导航测试改为核对 AI 页是原生页，共享应用只剩账号与同步。`shell_links` 不变（AI 页没有用过其中任何一项）。
- 规则写成只有头文件的 `AiSettings.h`：十三个服务商的标题、默认地址和模型、已知模型、文档（逐项照抄 `ai-provider-options.ts`），`aiProviderUpdate`（空或原服务商默认值才换，比较不去空白，与 TS 一样），`aiServiceCredentialTestDisabled`，三个提示词槽位和未知 `prompt_id` 退回 `custom_1`，候选数量的取值，润色测试指令，获取模型后的提示和换模型规则，两句兜底失败文字。由 `tests/ui/ai_settings.cpp` 核对，加进 CMake 测试；测试也核对服务商 id 与偏好库 `AI_PROVIDERS` 相同。
- 接口地址的来源键和说明每次都问 `msime_client_ai_endpoint_check`（纯函数），不在 C++ 里再写一份地址规则。Token 用 `PreferencesDocument::SetEntry` 写 `ai_assistant.tokens[来源键]`（键原样保存，含点和冒号），同时把旧的 `ai_assistant.token` 清空，与共享页 `updateToken` 相同；地址不能用时 Token 框不可用。
- 输入框（模型、接口地址、Token、提示词）沿用 `credential_input` 和 `write_credential`：边输入边写、停下后保存，不重建页面。跟着变的控件由 `ai_sync_` 就地更新：Token 行的说明、可用状态和框里的值（改地址时换成新来源下存的 Token；这次赋值与存的值相同，不算编辑），地址说明，预置模型的选中项，「测试 AI 辅助配置」，测试工具；获取模型那一块在当前事件处理完后整块重画，按钮不在自己的 Click 里被拆掉。开关和下拉框改动保存后重画整页。预置模型一行和语音页共用 `add_provider_preset`，改成接受模型表、文档、同步列表和写入函数。
- 共享页 `updateAi` 的副作用集中在 `ai_edited`：每次改 AI 设置都让 `ai_generation_` 加一、清掉测试结果和两个忙碌状态；改服务商或接口地址时再清掉已获取的模型。获取模型（`msime_client_ai_models`）和润色测试（`msime_client_ai_test`）用 `run_host` 在工作线程上调用，回来时代数或请求序号变了就丢弃。失败时只显示共享页的兜底句子，因为 Tauri 拒绝的是错误码字符串，共享页的 `formatError` 不会显示它。获取到的列表不含当前模型时换成第一个并重画。
- 「复制结果」走剪贴板页已有的 `recopy_clipboard_entry`：放进系统剪贴板，历史开着时记进历史，与 Tauri 的 `copy_text` 相同；`copy_text` 不收的文字（空、含 NUL、超过 `clipboard::MAX_TEXT_BYTES` 即 12000 字节）不复制。
- 「更多选项」第一次画这一页时按地址能不能用决定是否展开（`MoreOptions` 的 `defaultOpen` 只在挂载时读一次），之后记住用户的展开状态；搜索跳到「接口地址」时也展开。
- 候选数量用 `NumberBox`，上下限 1 和 10（对应输入框的 `min`、`max`），按 `clamp(Number(value) || 3, 1, 10)` 取值，显示值与存下的值不同时在事件之后改回。不设下限时在 1 上按向下箭头得到 0，`|| 3` 把它存成 3；设了下限，箭头停在 1 和 10，与浏览器的数字框一样。

## Alternatives considered

- **在 C++ 里照 `ai-endpoint-policy.ts` 判断地址** — 不用每次跨 C 接口；但这是第三份地址规则，Token 存在哪个来源键下一旦和 Rust 对不上，联想请求就拿不到 Token。
- **输入框改动也重画整页** — 写法简单；但每输入一个字都会丢焦点。
- **单个请求代数，润色测试开始时也作废进行中的获取模型** — 共享页就是这样（`test` 里 `++requestGeneration`）；但作废后它的 `modelsBusy` 不会复位，按钮会一直停在「获取中…」直到下一次改设置。原生页让两个请求各有序号，只有改设置才作废两者。
- **候选数量照样存小数** — 共享页输入 2.5 会存 2.5，偏好库的 `u8` 拒收，保存失败；原生页四舍五入。

## Consequences

- 原生窗口不再为 AI 设置拉起共享应用；共享应用里只剩账号与同步一页由「打开」卡片跳转。
- 共享页的服务商表、默认值、换服务商规则、提示词槽位或文案改了，要同步 `AiSettings.h` 和 `main.cpp`。
- 与共享页有三处有意的差别（见上）：润色测试不作废进行中的获取模型；候选数量存整数；手输 0 时 `NumberBox` 按下限改成 1，共享页取 3。

## Verification

- `tests/ui/ai_settings.cpp`、`tests/ui/settings_navigation.cpp`、`tests/ui/voice_settings.cpp` 用 cl `/W4 /WX` 单独编译运行，全部通过。
- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1`，链接上一步构建的 `msime_host_api.dll.lib`）。Rust 没有改动。
- 没有启动窗口，也没有对真实 AI 服务发请求：页面的绘制、就地更新、获取模型和润色测试都没有在运行中的窗口里看过。
