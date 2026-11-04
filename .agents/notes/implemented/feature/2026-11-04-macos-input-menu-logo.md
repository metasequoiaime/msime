# Agent Note: macOS 输入模式改用水杉标志和方案名

Status: implemented

## Problem

维护者反馈：macOS 的系统输入法菜单里，豆包、微信、Chatterfly 等输入法都显示自己的标志，水杉的「水杉输入法 · 中」只有一个光秃的「中」字，看不出是哪家输入法；「水杉输入法 · 双」这类名字里的单字也不直观。

单字图标是早先定下的做法：十一个输入模式各用一张铺满图块的大字（`render_menu_icon.swift` 生成），让菜单栏输入源和 Ctrl+空格 切换条分得清当前模式，因为「标志加右下角角标」在 16 点下认不出角标。

## Decision

**十一个输入模式的 `tsInputModeMenuIconFileKey` / `tsInputModePaletteIconFileKey` 全部指向产品标志 `MSIMEClientInputMethodMenuIcon.tiff`。** 输入菜单、菜单栏输入源、切换 HUD 和 Ctrl+空格 切换条都显示水杉标志。代价是这些地方不再分得清模式：模式靠输入菜单里每一行的名字和输入法自己的菜单栏状态项区分。这是维护者明确要的取舍。

**模式名改成方案名**：中 / 双 / 五 / 粤 / 注 / 日 / 韩 / 越 / 藏 / 笔 / 英 → 水杉全拼、水杉双拼、水杉五笔、水杉粤语、水杉注音、水杉日语、水杉韩语、水杉越南语、水杉藏语、水杉笔画、水杉英文；英文系统为 Metasequoia Pinyin、Shuangpin、Wubi、Cantonese、Zhuyin、Japanese、Korean、Vietnamese、Tibetan、Stroke、English。改的是两份 `InfoPlist.strings`、`MSIMEInputModeMenuName`（状态项悬停提示与读屏标签）和共享设置页的 `macosInputModeEntries`。单方案版本的主模式 `.Hans` 本来就借默认方案模式的名字（`edition_bundle.py`），所以五笔版的主模式叫「水杉五笔」、日文版叫「水杉日语」；新名字不再以「水杉输入法 · 」开头，版本改名那一步对它们不起作用，模式名在各版本里一样，`bundle` 本身的名字（水杉输入法 / 水杉五笔……）不变。

**菜单栏状态项继续画大字。** 它现在是唯一按模式显示的地方（#7066、#10630），不能跟着 plist 变成标志。新增 `MSIMEInputModeGlyphFile(identifier)`：按模式标识的后缀（Hans → Chinese、Roman → English，其余同名）取 `MSIMEClientInputMethodMenuIcon<名>.tiff`，不看 plist；状态项改用它，取不到时退回输入法本身声明的图标（标志）。单方案版本里控制器报的是方案自己的模式（五笔版报 `.Wubi`），所以照样画「五」，与改动前经 `.Hans` 取到五笔图标的结果一致。大字 TIFF 都还在 bundle 里（CMake 无条件打包），`info-plist-icons` 改为要求每个模式都指向标志，并检查每个声明的模式都有自己的大字、带齐 16 与 32 两页。

**CI 对实际暂存的各版本 bundle 复用 `info_plist_icons.py`。** `ci-macos.yml` 在 full、五笔、日文、越南文和藏文版的签名与资源检查后验证菜单/面板均为产品标志、引用文件存在、菜单和状态项字形带齐两页；保留原有 bundle 身份、连接名和语言归属断言。旧工作流单独要求五笔等版本继续使用大字图标，与本决定冲突，在签名成功后必定失败，不能靠重跑消除。

## Alternatives considered

- **只把主模式「中」换成标志，其余保留大字**：最强的理由是开多个模式的用户在菜单栏仍分得清模式。维护者要求全部换成标志，不选。
- **标志加角标**：最强的理由是品牌与模式兼得。不选它：16 点下角标认不出，大字方案就是为此替换它的。
- **状态项也画标志**：最强的理由是改动最小。不选它：状态项会退回 #10630 的问题，切中英文时一动不动。
- **名字保留「水杉输入法 · 」前缀再加方案名**：最强的理由是版本改名逻辑照旧生效。不选它：输入菜单行太长，且维护者给定的就是「水杉全拼」这类名字。
- **在工作流里重新枚举产品标志断言**：补丁最小。但会继续与 `info-plist-icons` 各维护一份规则，也不能验证文件或 TIFF 页缺失，因此直接复用已存在的行为检查。

## Consequences

- 菜单栏输入源、切换条只显示标志；开着「菜单栏显示中/英状态」的用户靠状态项看模式。
- 名字与图标在 bundle 替换后不一定立刻刷新：设置应用安装或更新后会清掉「添加」对话框的输入源缓存并结束 CursorUIViewService、TextInputSwitcher（`refresh_system_input_source_lists`），Sparkle 应用内更新只替换 bundle、不重新登记；菜单栏输入菜单本身是否立刻换上新名字和图标没有在真机上确认，最坏情况要注销重登。
- 本篇部分取代 2026-11-02「状态项画带框的模式字形」里状态项与系统菜单取同一张字形的做法；带框、回退和测试思路不变。

## Verification

- `info-plist-icons`、`info-plist-names`、`scripts/test-macos-editions.py`、`installer_package.py render`（full 与五笔版的英文模式名）。
- `input-mode-identifiers`、`input-mode-status-item` 单测在本机编译运行通过；`input-mode-status-item` 在改动前的状态项实现上以「Chinese and English modes draw the same status item image」失败。
- 共享设置页 `macos-input-mode-entries-section` 等 vitest。
- 没有在已安装的构建里看系统输入法菜单。
