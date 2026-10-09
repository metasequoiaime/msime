# Agent Note: 五笔混输开关统一叫「五笔拼音混输」，说明照引擎行为写

Status: implemented

## Problem

#6048：Linux 五笔用户请求「五笔拼音混输」。这个功能早就有，所有 Linux 发布版都带着：偏好 `wubi_mixed_pinyin`，引擎 `crates/engine/src/ime/mod.rs` 的 `decode()` 在它打开时对同一串字母再查一遍全拼，用 `merge_pinyin_fallback` 把去重后的拼音行接在五笔行之后。完整版默认关，水杉五笔版（`shared/contracts/editions.json` 的 `preference_defaults`）默认开。

用户找不到它，有两个原因：

- 开关叫「编码打不出时用拼音候选」，没有「混输」二字。同类功能都叫「中英混输」「emoji 混输」，用户会按这个词去找。
- 说明写「五笔词库无法回答当前编码时，用同一串字母查询全拼；词库能回答时不影响」，与引擎不符。`decode()` 不看 `wubi_table_answered` 就查全拼，拼音候选每次都会出现在五笔候选之后；`wubi_table_answered` 只决定能不能打第五码。这种「回退」式的说法让真正想要混输的用户以为这不是自己要的功能。

## Decision

- 所有宿主的开关标题统一为「五笔拼音混输」，说明统一为「五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选。」涉及：共享设置页 `packages/ui/src/settings/wubi-section.tsx`（Linux、Android、Harmony 和桌面壳都用它），macOS `AppearancePreferences.mm` 的开关与设置行，Windows `settings/main.cpp` 的 `bool_row`，iOS `OnboardingView.swift` 的引导行（副标题不带句号，与同组其他行一致）。
- macOS 的搜索别名改为「五笔混拼」和旧标题「编码打不出时用拼音候选」，按旧名或「混拼」搜仍能找到。
- 偏好键 `wubi_mixed_pinyin`、各版本的默认值都不变，不涉及持久化格式。
- 描述这项偏好的注释一并改成与引擎一致：`client-core` 的 `Preferences::wubi_mixed_pinyin`、`mcp-server` 的偏好字段说明（它会进 MCP 工具的 JSON schema，给调用方的模型看），以及 macOS 设置里解释五笔为什么保留模糊音那段注释。
- 依赖旧标题定位控件的测试同步改：`wubi-section.test.tsx`、`input-scheme-edition.test.tsx`、Android 设备冒烟 `SettingsDeviceSmoke.java` 的 `aria-label` 选择器。`wubi-section.test.tsx` 另外钉住新的说明文字。

## Alternatives considered

- **只改说明、保留标题**：改动最小，说明也就如实了。没有采用：用户按「混输」找功能，标题里没有这个词，搜索和目录浏览都找不到，#6048 的问题还在。
- **标题用「五笔混拼」**：仓库代码注释和 README 里一直这么叫，字也更少。没有采用：设置页里同类开关都是「×× 混输」，用户在 issue 里用的也是「五笔拼音混输」；「混拼」留作 macOS 搜索别名。
- **完整版也默认打开**：能让更多人直接用上。没有采用：会改变纯五笔用户的候选列表（后部出现拼音候选）以及 z 键和第五码的行为（`wubi/scheme.rs`、`ime/mod.rs` 的 `extended`），这是产品决定，不在这次文案修正的范围里。

## Consequences

收益：设置里按「混输」「五笔」「拼音」都能看到这个开关，说明与实际行为一致，打开前就知道拼音候选会排在五笔候选之后。

代价：Harmony 的 `entry/src/main/resources/rawfile/settings/index.html` 是共享设置页的构建产物，不在 git 里，要随下一次 Harmony 构建重新生成才会换成新文案。旧标题从界面上消失，看过旧版说明或截图的用户要按新名字找（macOS 搜索仍认旧名）。

验证：`apps/desktop` 下 `vitest run tests/settings tests/input tests/account` 与 `pnpm run typecheck`；原生宿主改的是字符串字面量和注释，用 `rbuild bash scripts/verify-local.sh --quick` 确认能编译。

尚未验证：没有在 Android 设备或模拟器上跑 `SettingsDeviceSmoke`，依赖 PR 上的 android-device.yml；没有在 Windows、iOS 真机上看新文案。
