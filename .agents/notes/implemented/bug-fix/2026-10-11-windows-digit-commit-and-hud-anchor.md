# Agent Note: Windows 数字选词后结束组字，输入模式提示只认 Show/Move 报的锚点

Status: implemented

## Problem

2026-10-11 在 Windows 11（26200，2560×1440，96 DPI）真机上验收 #6762，用 SendInput 往一个 WPF 文本框里打字，查出两处产品问题。

- **数字选词后，下一次组字把刚上屏的词换掉。** 打 `pingguo` 按 1，文本框里是「苹果」；再按 `n`，只剩 `n`。`shi` 按 2 得「时」，接着 `shi` 按 3，「时」变成「事」。空格选词没有这个问题。全拼也建宿主会话，数字键走 `_HandleCandidateWorker` 的宿主分支：`_AddCharAndFinalize` 在组字还开着时只把候选写进组字范围，随后只关候选列表，只有补成对标点时才结束组字，于是 TSF 组字一直罩在刚上屏的词上，下一个字母开始的新组字对同一个范围 SetText。装回 #6762 之前的安装包，现象完全一样，是 2026-09-12 的 `005d8820b7` 引入的。
- **输入模式提示总出现在屏幕左上角（43,94）。** 把测试窗口挪到 (900,600) 再切换，位置不变。WPF 窗口是系统 DPI 感知，`InputModeHudWindow::system_caret` 不用它的系统光标，于是退到组字锚点；而那个锚点取的是 `CandidatePresentation::x/y`。按键包不带坐标，`candidate_presentation` 每个按键都把 x/y 写成线上结构的默认值 (100,100)，要等随后的 Move 才改回来，组字结束后不再有 Move。按 (100,100) 摆出来的窗口正好是 (43,94)。

## Decision

- 宿主分支的数字选词在 `_AddCharAndFinalize` 成功后一律走 `_HandleCompleteCommitFirst` 结束组字，和空格经 `_HandleCandidateFinalize` 上屏一样；成对标点照旧在组字结束之后再入栈、移回光标。同时像 `_HandleHostRawCommit` 一样记下智能标点用的末字。`paired_punctuation_wiring` 钉住「结束组字在记下成对标点之前」以及旧的「只有补全时才结束」的写法不再出现。
- `CandidatePresentation` 另记一份 `caret_anchor_known/x/y`，只由 Show/Move 包写，同一租约里的按键投递和释义、云候选到达时的重建（`refresh_view`）都沿用上一份，新租约从没有开始。第一版只在按键投递里沿用，真机上提示退到了屏幕下方居中：打开英文释义后每一页都会经 `refresh_view` 重建一次，锚点随之丢掉；在 Server 里临时记下 Show/Move 包，确认它们带着 (911,667) 之后才找到这一处。输入模式提示的兜底只认这一份；`x/y` 本身不动，候选窗自己的定位逻辑不受影响。`candidate_mailbox` 用例覆盖按键不改锚点、Move 写入锚点、跨按键与释义重建沿用、新租约清空。
- 顺带修本地安装流程：`Invoke-LocalInstall.ps1` 传给 `Prepare-PackageFiles.ps1` 的声明目录是绝对路径，后者无条件接在 `RepoRoot` 后面，拼出 `E:\repo\D:\...`。现在绝对路径照用，`package-files.ps1` 改用绝对路径调一次。

## Alternatives considered

- **让按键投递直接沿用上一次的 x/y。** 一处改动就让所有读 `x/y` 的地方都拿到真实锚点，看起来最彻底。没有采用：候选窗的游戏兜底定位（`game_candidate_anchor`）和「锚点无效时等下一个 Move」的判断都以按键投递后的 `x/y` 为输入，改它会动到候选窗的定位时序，而这次真机只证明了提示的位置错，候选窗的位置是对的。另记一份锚点只影响提示。后来按键投递改为沿用 `x/y`，理由见 [按键投递沿用同一租约里报过的候选窗位置](2026-10-13-windows-candidate-position-across-keys.md)；这里另记的锚点仍在用。
- **让 `system_caret` 也接受系统 DPI 感知的窗口**（在窗口自己的感知上下文里 `ClientToScreen`，再 `LogicalToPhysicalPointForPerMonitorDPI`）。能让 WPF、WinForms 这类应用直接用系统光标。没有放进这次：锚点兜底修好后这些应用的提示已经落在组字位置附近；改光标换算要在高 DPI 的多显示器上验证，这台机器是单屏 100%，验不了。
- **数字选词改走 `_HandleCandidateFinalize`。** 能和空格共用一条路径。没有采用：宿主会话拥有组字时那条路读的是 Server 的回复，而数字要向宿主会话自己选词，两边的候选身份不一定一致；只补上结束组字这一步更小。
- 数字键按槽位取到的是哪一页，取决于高亮推出的页码，见 [翻页展开长出新生页时高亮必须前进](2026-10-24-windows-paging-expansion-highlight.md)：那边修的是页码本身停在旧页，这里修的是选完之后组字没结束，两处都会让数字键的观感出错，但入口不同。

## Consequences

- 数字选词之后接着打字不再吞掉上一个词，影响所有走宿主会话的方案和所有 TSF 应用；以前在 WPF 之外不明显，是因为用户换焦点或点别处时应用会自己结束组字。
- 系统 DPI 感知、没有每显示器光标的应用里，提示摆在最近一次组字的锚点旁；同一租约里还没组过字时摆在屏幕下方居中。
- 真机验收里另外两处现象不是产品问题：UI Automation 读不到元素、注入的鼠标点不动工具栏，都是因为测试进程的完整性级别低于带 uiAccess 的 Server；改用提权进程后候选窗和工具栏的元素树完整，拖动工具栏会写出位置文件并在 Server 重启后保持。WPF 里 Alt+数字不上屏释义，是 WPF 在按住 Alt 时不把按键交给 TSF，Tab 预选加数字可以替代。

## Verification

- `clang++ -std=c++17 platforms/windows/tsf/tests/input/paired_punctuation_wiring.cpp` 编出后对改动前的源文件运行失败、对改动后的通过。
- 真机（Windows 11 26200，MSVC 构建、本机测试证书签名的安装包）：WPF 文本框里 `pingguo` 1 再 `nihao` 空格得到「苹果你好」，`shi` 2 `shi` 3 得到「时事」，改动前分别是「你好」和「事」。窗口挪到 (900,600)、组字锚点 (911,667) 时切换中英文，提示窗口在 (854,661)，卡片水平居中于锚点、在锚点下方 10 像素；改动前固定在 (43,94)。
- Windows 交叉构建、Wine 测试与 `scripts/verify-local.sh --quick` 由推送门禁和 CI 跑。
