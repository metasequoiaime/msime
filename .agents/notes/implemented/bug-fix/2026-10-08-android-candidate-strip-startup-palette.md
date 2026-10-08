# Agent Note: Android 候选条在新输入框里先按出厂薄荷配色画

Status: implemented

## Problem

#5933：Via 浏览器里点进一个默认英文的输入框（光标前已有单词，候选条立刻出英文联想），候选条先铺一层浅薄荷色底、约一秒后才变成用户自己的深色皮肤，键还是用户的皮肤，联想词是白字压在浅底上。录屏取样得到的底色是 `#D8F0E4`，正是出厂触屏皮肤「薄荷晨光」（`TouchKeyboardSkinDesign::mint_morning`）的背景。

原因和 [工具栏那一跳](2026-10-08-android-toolbar-startup-state.md)、[剪贴板历史被清空](2026-10-08-android-clipboard-history-retention.md) 同源：`onStartInput` 用 `runtime-options.json` 里的偏好副本调 `applyEditorPreferences(…, RUNTIME_OPTIONS_COPY)`，那份副本是首次安装时写下的出厂默认。键盘皮肤早已只认实时偏好和皮肤片段，`applyCandidateAppearance` 却不分来源照样执行，把候选条配色重算成薄荷、字号重置为出厂的 18（读音行 15），直到实时偏好应用后才换回来。候选条空闲时是隐藏的，所以只有一进框就有联想的输入框看得到这一层底色。

0.3.0 里的 #5766 让会话建好时直接应用建会话前读到的实时偏好，正常情况下这一帧被盖住了；但实时读取失败（`withLivePreferences` 拿不到快照）或应用抛错时薄荷底仍会出现，字号也仍会在每次换输入框时回到出厂值。

## Decision

- `applyEditorPreferences` 只在 `live` 时调 `applyCandidateAppearance`。副本路径保留当前的候选条配色和字号，也就是上次真正读到的偏好或启动时按皮肤片段算出的那一份。
- `candidateHorizontal` 的字段初值改为 `true`。触屏候选条始终横排，原先靠副本路径每次把它置为 `true`；不再走那条路径后，新进程在实时偏好到来前也必须是横排，否则候选落到竖排视图里。
- 皮肤片段 `keyboard-skin-hint.json` 的键加上候选条用到的 `candidate_theme`、`candidate_font_family`、`candidate_english_font`、`candidate_fallback_fonts`、`candidate_font_size`、`candidate_preedit_font_size`；`onCreate` 读到片段时调 `applyCandidateAppearance(hint)`，新进程第一帧的候选条就是用户上次的外观，而不是字段初值的经典绿。片段分支之前先 `refreshLocalSettings()`：`candidateAppearanceFor` 经 `ImeStyler.themed` 着色，第一次着色按 `localSettings` 里的应用主题算种子并缓存约 60 秒（`SEASON_CHECK_INTERVAL_MS`），本地设置还是字段初值时种子会落在默认的四季上，键盘和候选条都要错色一分钟。
- `check-host.sh` 守住：`applyEditorPreferences` 里的 `if (live) applyCandidateAppearance(preferences);`、`onCreate` 片段分支里的 `applyCandidateAppearance(hint);` 及其之前的 `refreshLocalSettings();`、片段键含候选条字段、`candidateHorizontal` 初值为 `true`。

## Alternatives considered

- **只靠 #5766 的「会话建好即应用实时偏好」** — 正常路径上已经看不到这一层底色，不改也行；但根因那行仍在，实时读取一失败就回来，而且每换一个输入框都把候选字号打回出厂值，只是被随后的实时偏好盖住。
- **只加 `if (live)`，不动 `onCreate`** — 改动最小；但冷启动且实时偏好还没到时，候选条会停在字段初值 `KeyboardSkin.system(false)` 的经典绿上，等于把一种错色换成另一种。
- **让 `runtime-options.json` 的副本跟着偏好更新** — 理由同 [剪贴板历史那篇](2026-10-08-android-clipboard-history-retention.md)：副本由 Bootstrap 写，各处都依赖它是出厂默认，影响面远大于这个问题。

## Consequences

- **收益**：新输入框里的候选条从第一帧起就是用户的配色和字号，与键盘皮肤一致；实时读取失败时也不再出现出厂薄荷底。
- **代价**：片段文件多了候选条字段，老片段在下次读到实时偏好时重写一次。老片段没有 `candidate_theme`，按 `follow` 算，用户单独给候选条指定了明暗时，升级后第一次冷启动的第一帧可能与之不符，实时偏好到了即纠正。
- **未覆盖**：没有在真机上复现验证，只有 `check-host.sh` 的契约守卫与 JVM 冒烟。录屏里候选条底部那条灰线推测是 `horizontalCandidateScroll` 的滚动条，只在浅色底上看得见，本次不改。
