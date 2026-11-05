# Agent Note: 共享设置界面接入新世纪五笔

Status: implemented

## Problem

引擎和宿主已经认识第三个五笔版本 `wubixsj`（[引擎与宿主接入新世纪五笔](2026-11-02-wubixsj-engine-host.md)）。macOS、Linux 和 HarmonyOS 的设置界面是共享 React（`packages/ui`），这一层还只认 86 和 98：

- 版本选择只有两项，选不到新世纪。
- 各处写的是 `profile === "wubi98" ? "98…" : "86…"`。偏好里存着 `wubixsj` 时，这种三元会把它静默显示成 86，编译器不报错：语言卡片写「五笔 86」，触屏方案标题写「86 五笔」，引导页写「默认 86 版」，查码页把 86 那一列标成当前。
- 查码只画 86、98 两列，宿主已经回 `wubixsj` 字段也看不到。
- 宿主的个人词库已经有 `wubixsj` 种类，词库页的下拉、导出和导入校验都不认识它。

计划在维护者的 `~/msime-reports/wubi-xinshiji-plan.md` 第 5 节「共享 React 设置界面」，命名和取舍在第 2、8 节。

## Decision

- **一个类型、一份顺序、一个兜底**：`settings/wubi-preferences.ts` 的 `WubiProfile` 加 `wubixsj`，`Preferences.wubi_profile` 改为复用它。`wubiProfiles` 定下列出顺序（86、98、新世纪）。`knownWubiProfile` 把缺省和不认识的值读成 86，与 client-core 的宽松读取一致。
- **每处文案都是 `Record<WubiProfile, string>`**，用 `knownWubiProfile` 取键。以后再加版本，漏改的地方编译不过，而不是显示成 86。界面名按计划第 2 节：
  - 正序「86 五笔 / 98 五笔 / 新世纪五笔」：版本下拉、触屏方案标题、查码列名、词库种类；
  - 倒序「五笔 86 / 五笔 98 / 五笔 新世纪」：语言卡片；
  - 引导页「形码 · 当前新世纪版」。
- **版本下拉**仍是返回 Fragment 的普通函数 `wubiProfileOptions`，只加第三个 `<option>`。鸿蒙手机的 `SheetSelectRow` 靠它读出面板选项。下拉的 `value` 也过一遍 `knownWubiProfile`，不认识的值显示 86，而不是空白。
- **查码始终三列**（第 7 节第 10 条）。`WubiTextCodes.wubixsj` 是可选字段：认识新世纪之前的宿主不回它，读成空数组，显示为「—」。三列在手机上变窄，「文字」表头加了 `whitespace-nowrap`，否则会折成竖排的两行。
- **个人词库种类**：`LocalDictionaryKind`、个人词导入的 `kind`、导出顺序、文件名、编码提示和报错都加上 `wubixsj`，编码规则与 86/98 相同（1 到 4 个字母）。
- **鸿蒙上词库下拉暂不列新世纪**（`shownLocalDictionaryKinds`）。鸿蒙的设置应用和输入法各有一份 Engine 日志，「我的词条」经 `msime_client_user_words_mirror` 交换。这条通道与 WebDAV 共用种类映射，`word_kind(WubiXsj)` 是 `None`。在设置应用里加的新世纪词到不了键盘，所以先不让用户在那里加。鸿蒙平台的 PR 让交换认新世纪时，把这一处放开。
- **Tauri 安卓账号快照校验**（`android_account.rs`）的种类白名单加 `wubixsj`。服务端现在不发这个种类，加上是为了打开同步后不因白名单拒绝整份快照。
- **不改的**：
  - 云词库的种类标签和面板（同步暂不打开，第 8 节）；
  - 社区资源的种类（第 13 条，不按版本拆）；
  - `helpcode-preferences.ts`（第 6 条）；
  - `apps/harmony/src/main.tsx`（只透传）。

## Alternatives considered

- **只把三元改成 `switch`，不抽 `knownWubiProfile`。** 每处少一次函数调用，读起来也直白。不用它，是因为四个文件都要各自写「不认识就按 86」，再加版本时容易有一处漏写默认分支。`Record` 加一个统一的读法，交给编译器检查穷尽。
- **`WubiTextCodes.wubixsj` 声明成必填的 `string[]`。** 计划里就是这样写的，类型更干净。不用它，是因为认识新世纪之前的宿主确实不回这个字段，声明成必填会让 `item.wubixsj.length` 这种写法通过类型检查、到运行时才出错。可选字段逼着读的人写 `?? []`。
- **词库下拉在鸿蒙上照常列新世纪。** 一套代码，不用分平台。不用它，是因为用户在鸿蒙设置应用里加的新世纪词进不了键盘，界面却说加好了。这比暂时不提供更糟。

## Consequences

- macOS、Linux 和鸿蒙的共享设置页能选新世纪五笔，查码看得到三套编码，选着新世纪时各处标题不再显示成 86。
- 偏好里出现更新的版本写来的未知值时，界面按 86 显示，不会空白，也不会崩。
- 鸿蒙设置页的词库下拉暂时没有新世纪。导入 JSON（`PersonalDictionaryImportCard`）在鸿蒙上仍接受 `wubixsj`，这样导入的词同样只在设置应用那份日志里。两处等鸿蒙的词条交换认新世纪时一起处理。
- 新世纪的选择暂不随账号同步，这一点在宿主层（`settings_sync`），界面上没有提示。

## Verification

- `apps/desktop` 下 `pnpm typecheck`、`apps/harmony` 下 `pnpm typecheck`：通过。
- `pnpm exec vitest run`（`apps/desktop` 全量）：883 个文件、5886 条全部通过。新增和改写的用例：
  - 版本下拉三项、选新世纪、未知值按 86（`input-scheme-details-section.test.tsx`）；
  - 鸿蒙手机面板列出三项（`action-sheet.test.tsx`）；
  - 语言卡片列出「五笔 新世纪」并保存 `wubixsj`（`language-card.test.tsx`）；
  - 触屏标题「新世纪五笔」、未知值按 86（`touch-keyboard-scheme-selection.test.ts`）；
  - 查码三列，标出当前列，旧宿主缺字段时为空（`wubi-code-lookup-section.test.tsx`、`wubi-code-lookup-state.test.tsx`）；
  - 引导页「形码 · 当前新世纪版」（`onboarding-page.test.tsx`）；
  - 词库种类、导出顺序、文件名、导入校验、报错（`tests/dictionary/*`）；
  - 鸿蒙 2in1 的词库下拉不列新世纪（`dictionary-lookup.test.tsx`）。
- `rustfmt --check` 检查了 `android_account.rs`。安卓的 Tauri 目标没有在本机编译，改动只是 `matches!` 里多一个字符串。
- 截图是在本机 Chromium 里用合成宿主渲染共享组件截的。没有编原生宿主：本机磁盘不足 50 GB，而设置应用要在本机的图形会话里打开。查码图里 86/98 的码查自本机的 `msime-wubi.db`，新世纪那一列取自引擎 fixture，因为真实码表还没生成。
