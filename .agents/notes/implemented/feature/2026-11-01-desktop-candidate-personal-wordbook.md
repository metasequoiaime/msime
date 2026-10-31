# Agent Note: 桌面候选窗的「加入生词本」

Status: implemented

## Problem

[可写的个人生词本](2026-11-01-personal-wordbook.md)只有共享层的存储，用户还没有办法从打字的地方把一个词加进去。桌面三端（macOS、Windows 新宿主、Linux 的 IBus 和 Fcitx5）各有自己的候选窗，背单词的 Tauri 页面也还不认识这本书。

## Decision

入口放在候选的操作菜单里，四个前端同一个位置：菜单最后隔一条线的「加入生词本」，只在这一行候选显示着释义时出现。

- macOS：候选右键菜单（`InputController.mm` 的 `menuForCandidate`），标签 30。菜单打开时把这一帧的释义（`CandidateTranslation`，U+2028 换回换行）和读音（`pronunciationForCandidate`）记进菜单项，之后释义再变也不影响这一项。
- Windows：候选右键浮层（`CandidateMenuLayout.h` 的 `candidate_menu_items` 多一个 `personal_wordbook` 参数），打开时记下候选原文、`translation` 和 `pronunciation`，选中后经 `DesktopCandidateCallbacks::personal_wordbook` 交给 Server 里单独的 `SingleClickWorker`。AI 占位行没有这一项；云候选、AI 候选这些词库操作不可用的行照样有。
- IBus：每个候选在「候选操作」里的那一组子菜单最后加一项（名字 `CandidatePersonalWordbook/<session>/<generation>/<index>`），条件和面板上画释义的条件相同（`displayed_candidate_gloss`）。
- Fcitx5：候选右键动作 30，取 `FcitxCandidateDisplay::translation`。

四个前端都只把「候选原文、显示着的释义、读音」交给新的 C 接口 `msime_client_personal_wordbook_add`，由 `client-services::vocabulary::personal::add_gloss` 决定怎么变成卡片、怎么合并，并且回答一句给用户看的提示（「已加入生词本」「已在生词本里」「生词本已满」「这条释义不能加入生词本」）。宿主原样显示这句话：macOS 和 Windows 用光标旁的输入状态提示，Linux 用候选上方的辅助行。这个提示回应的是用户刚点的菜单项，不受「切换提示」开关约束（macOS 的 `presentHUDText:`，Windows 的 `InputStateHudText::answers_action`）。

写入的线程：macOS 在自己的串行队列，Windows 在 Server 的工作线程；Linux 两个前端在菜单回调里直接调用。那里写的是一个不到 2 MB 的文件，只在用户点了菜单项之后发生一次，为它起线程再把结果送回 GLib 主循环换来的只是几毫秒。

背单词页（`packages/ui` 的 `VocabularyReviewPage`）认 `personal` 字段：有生词本时说明它有几个词、怎么往里加，提供「导出生词本」（`export_personal_vocabulary`，走宿主的 `saveExport`，没有时用下载链接）；选中生词本时删除按钮叫「清空生词本」；复习生词本里的卡片时多一个「移出生词本」（`ReviewAction::RemovePersonalWord`）。只有给了 `removePersonalWord` 的宿主才显示这些，候选窗还没有这个入口的宿主（这一步之前的移动端）页面不提生词本。

## Alternatives considered

- **快捷键，例如组字时的 Ctrl+Shift+Enter，作用于高亮候选**：最强的理由是不离开键盘，而且三端已经有 Ctrl+Enter 上屏释义这一族快捷键，好记。不用：快捷键看不见，用户不会知道有这件事；Ctrl+Shift+Enter 在不少编辑器里有自己的意思，桌面端在组字时又一律把 Ctrl 组合键交还给应用，接住它要在四个前端各改一遍按键路由。菜单项一眼能看见，也和置顶、删除这些对一个候选做的事放在一起。以后需要再补快捷键，接到同一个 C 接口即可。
- **提示文字由各宿主按 outcome 自己写**：宿主之间不依赖共享层的措辞。不用：四个前端加上后面的三个移动端会有七份文案，改一次措辞要改七处，迟早不一致。
- **成功时不提示，只在失败时响一声**：最省事。不用：菜单关掉以后用户无从知道词进没进去，也分不出「已经在了」和「加进去了」。
- **Linux 也起工作线程写文件**：和 macOS、Windows 一致。不用：理由见上，Linux 的 IBus 引擎和 Fcitx5 插件都没有现成的、能把结果带回主循环并核对会话的工作线程，为几毫秒的一次写入新起一套不划算。

## Consequences

- 候选窗显示释义的用户，在四个桌面前端都能从候选菜单把释义收进生词本；没开释义的用户菜单不变。
- Windows 的 HUD 现在也可能为菜单动作出现，即使「切换提示」关着。
- 背单词页多出导出和移出两个动作；Windows 的原生设置窗口和 Win7 面板照常把生词本当一本书列出、复习和删除，没有导出和「移出生词本」。
- 移动三端接在长按候选的菜单里，见[移动键盘的「加入生词本」](2026-11-01-mobile-candidate-personal-wordbook.md)。

## Verification

共享层：`cargo test -p msime-client-services --lib vocabulary` 127 项通过（新增 `personal` 模块和会话层「移出生词本」的用例），`cargo test -p msime-host-api --lib` 里新增的边界用例覆盖新增、已存在、无效释义、相对目录、多余字段、空指针，以及加进去之后 `msime_client_vocabulary_review` 列出生词本并能移出单词。React：`apps/desktop/tests/settings/vocabulary-review.test.tsx` 43 项通过（新增 4 项），桌面与鸿蒙设置页 `tsc --noEmit` 通过。macOS：`shortcut` ctest 通过，新增的断言覆盖菜单项的位置、带的释义和它不经过 Engine。Windows：`candidate_menu_layout.cpp` 在本机编译运行通过，改动的候选窗源文件用 MinGW 做了语法检查，完整交叉构建交给 PR 的 CI。Linux：Fcitx5 契约脚本通过，原生构建交给 PR 的 CI（本机没有 Docker）。四个前端都没有在真实的系统输入法里点过。
