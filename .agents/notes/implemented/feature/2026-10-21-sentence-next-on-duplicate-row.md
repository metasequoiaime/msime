# Agent Note: 共享设置页补上「去重后补充次选」

Status: implemented

## Problem

Windows 侧有「去重后补充次选」这一项：某个模型排第一的整句和已有候选重复时，改显示它排名最高的下一条不同结果。macOS 侧的两个前置条件都已就位，却没有任何入口：

- 偏好字段 `SentenceAssociationPreferences.show_next_on_duplicate` 已在 `crates/client-core/src/preferences.rs`，默认 `false`，用户机器的 `preferences.json` 里可能已经存着它。
- 引擎已按它运行：26 键的 `lattice::merge::take_sentence_index` 与九键的 `place_keyboard_pick` 在模型首选已被列出时，只有打开它才继续看模型的下一句（[九键句子设置](2026-10-09-nine-key-sentence-options.md)）。
- 桌面共享设置页 `packages/ui/src/settings/sentence-association-section.tsx` 的「整句联想」组只画「桌面神经联想」和资源包行，没有这一行。macOS 用户既看不到也改不了，只能手改偏好文件。

写这篇之前按机制名检索了活跃笔记（`rg --hidden --glob '!.agents/notes/archived/**' "整句联想|神经联想|sentence_association|show_next_on_duplicate" .agents/notes/`）。[精简「输入」「快捷键」两页](../simplification/2026-10-16-macos-input-settings-prune.md) 持有「候选与联想」组的行布局，并规定「本地整句联想」收进「更多选项」；[整句联想偏好与级别规则独立于 React 展示层](../simplification/2026-10-15-sentence-association-preferences.md) 持有纯偏好模块，并在级别写回时保留 `show_next_on_duplicate`；[九键句子设置](2026-10-09-nine-key-sentence-options.md) 持有引擎语义。三篇都不拥有「把这个字段暴露成一个设置行」，因此新建本篇并与它们互链。

## Decision

`SentenceAssociationSection` 的桌面分支新增一行「去重后补充次选」，紧跟「桌面神经联想」（有模型下载行时排在它后面）：

- 文案：标题 `去重后补充次选`，说明「模型排第一的整句与已有候选重复时，改显示它排名最高的下一条不同结果。」
- 显示条件只有 `!mobile`。触屏宿主的整句联想走「表达」页的级别选择（[iOS 与鸿蒙全平台设计](../feature/2026-10-07-ios-harmony-all-platform-design.md)），级别里没有这一项，保持原样。
- 读取用 `value?.show_next_on_duplicate ?? false`，与 Rust 结构体的默认值一致；写回是 `{ ...value, show_next_on_duplicate: checked }`，只覆盖这一个字段，缺键文档打开后得到的正是该字段为真。
- 不放进「更多选项」的折叠里：这是模型排序行为的开关，与同为整句来源的「桌面神经联想」并列才找得到；「更多选项」留给「本地整句联想」这类显示级开关。

## Alternatives considered

- **收进「更多选项」，和 `SentenceWordLatticeRow` 并排**：行布局最省事，也符合[精简「输入」「快捷键」两页](../simplification/2026-10-16-macos-input-settings-prune.md)把少改的候选行折叠起来的做法。没有采用，是因为折叠后 macOS 用户仍然要先展开才看得到，和本次缺口「看不到、改不了」只差一次点击，而它描述的是模型行为，与同组的「桌面神经联想」直接相关。
- **只在 `neural_desktop` 打开时显示**：语义上最贴切，模型关着时这一项不产生任何输出。没有采用，是因为偏好是独立持久化并会上云的字段：别的设备同步来的选择、或用户先设好再打开模型，都不该被隐藏；模型关着时改它只是暂时不生效，不是无效项。
- **把 `showNextOnDuplicateChecked` 加进 `sentenceAssociationControlState` 的返回值**：与 `wordLatticeChecked` 的读取集中在一处。没有采用，是因为该返回值在既有用例里按完整对象断言，增字段要改三处精确匹配断言，收益只是少一次 `?? false`，不值得动已通过的测试。
- **同时给触屏宿主补一行**：九键的键盘模型同样读这个字段。没有采用，是因为本次缺口只在 macOS，触屏宿主的整句联想已经用级别选择表达；在手机布局里加一个只在「增强」下有意义的小开关，会改动 iOS 与鸿蒙的设计稿布局，超出范围。

## Consequences

- **收益**：macOS 用户在「输入 › 候选与联想」里能看到并切换「去重后补充次选」，写入 `sentence_association.show_next_on_duplicate`，不再需要手改偏好文件；与 Windows 侧已有该项对齐。
- **代价**：Linux 的桌面共享页同样会显示这一行（同一个组件）。Linux 的引擎路径与 macOS 相同，行为一致，没有平台专属分支。
- 默认值不变（缺键仍按关），不会改变任何没动过这一项的用户的候选结果；切换只写一个字段，级别写回、模型下载和其他宿主的显示都不受影响。
- 触屏宿主的页面、`sentenceAssociationControlState` 的输出形状和 Windows 原生设置窗口都不动。
- **需要重访的信号**：触屏用户要求在「增强」级别下补这一项；或 `sentence_association` 的字段改成 `show_next_on_duplicate` 之外的语义。

## Verification

- `pnpm --filter @msime/desktop test` 全绿，其中新增 `apps/desktop/tests/settings/sentence-next-on-duplicate-row.test.tsx` 两条用例：缺键文档点开关只写 `{ show_next_on_duplicate: true }`；已存 `word_lattice`、`neural_desktop` 时点开关保留这两个字段。改前两条都红在 `Unable to find an accessible element with the role "switch" and name "去重后补充次选"`，改后通过。
- 复用的既有用例 `apps/desktop/tests/settings/sentence-association-preferences.test.ts` 与 `resource-packs.test.tsx` 未改动。
- `pnpm format:check` 与 `pnpm run verify-notes` 通过。
