# Agent Note: Windows 翻页后的键盘选词照 Server 的回复在宿主会话里选同一个词

Status: implemented

## Problem

用户报告：Windows 10/11 上用双拼，候选窗里选的词和实际上屏的不一样，翻几页之后更容易遇到。

Windows 有两个引擎会话。候选窗由 Server 进程的会话画（`src/candidate/CandidateWindow.cpp`），TIP 进程里另有自己的宿主会话负责组字和上屏（`tsf/EngineSessionAdapter.h`）。[按文字定位](2026-10-22-windows-host-candidate-identity-by-text.md)之后，数字键和空格的规则是：选哪个词以 Server 对这个键的回复为准，再按文字到宿主会话里找到它选中。这条规则只做了一半：

- 数字键只认 `Normal` 回复（整段上屏）。数字选中只消耗一段读音的候选时，Server 回的是 `NeedToCreateWord`（「剩下的原文\t已选的词段\t预编辑」），TIP 没有取它，退回宿主会话 presenter 上按键位置的那一项（`_HandleCandidateSelectByNumber` 先 `_SetSelectionInPage` 再选高亮项）。两边的页对不上时，那是另一个词。
- 空格的 `NeedToCreateWord` 把整条带制表符的载荷当作候选文字去找，永远找不到，宿主会话从不跟着前进，之后两边的组字就分开了。
- 选过一段之后，最后一次 `Normal` 回复的文字是「暂存的词段 + 这次的候选」，拿它去宿主会话的候选里找同样找不到；数字键这时什么都不写，Server 却已经清空了组字。
- 只在宿主会话的当前页上找。Server 那一页和宿主会话那一页不同时，用户选的词在宿主会话里不在当前页上。
- Server 什么都没选时（按键位置超出它那一页，回 `NavigationIgnored`），TIP 仍按自己那一页选了一个上屏；反过来宿主会话这一页比 Server 短时，TIP 在 `_SetSelectionInPage` 失败后直接返回，回复没人读。

两边的页为什么会不同：鼠标滚轮和候选窗的翻页箭头只翻 Server 那一页（`FocusedSession::page_candidate`，代码注释里写着它会让宿主会话停在另一页，只对注音这类纯键盘列表拦下）；两个会话各自学习后顺序会分开（`bushang` 那次就是）；TIP 的 presenter 数编号时还把 Server 不显示的云占位席位算作一格。

为什么翻几页之后更容易遇到：双拼的长读音第一页多是覆盖整段读音的整词，回复是 `Normal`，按文字定位能选对；往后翻，候选越来越多是只消耗前一段读音的短词，回复是 `NeedToCreateWord`，正好落进按位置选的那条路；而滚轮翻过页之后两边的页一定不同，位置上必然是另一个词。合成词库复现：小鹤双拼 `nihc`，滚轮翻到第三页按 4 选「霓」，宿主会话第一页的第 4 个是「妮好」，整词直接上屏。

## Decision

宿主会话拥有组字时，Server 对一次键盘选词的回复是唯一依据，TIP 在自己的会话里照着选同一个词，用宿主会话自己的身份，不再按自己那一页的位置选。规则集中在 `tsf/HostServerSelection.h`：

- `ServerReplyDecidesSelection`：`Normal`、`NeedToCreateWord`、`NavigationIgnored`、`OutofRange` 都是 Server 已经替这个键做了决定。只有读不到回复（超时、管道断开）时才退回本地高亮项。
- `ServerSelectedPiece`：从回复里取出这次选的那段文字。`Normal` 取整段，`NeedToCreateWord` 取第二段，两者都减掉宿主会话已经暂存的词段（`phrase_prefix`）；暂存的词段没变说明什么都没选，返回空。
- `SelectHostCandidateByText`：先在宿主会话当前页上找唯一的一个，没有就到这一代的完整候选列表里找唯一的一个，用列表自己的代次 `select_any`；同形的不止一个时 **never** 猜。文字落在占位席位上时吃掉按键、组字不动，与之前相同。

接线（`tsf/UI/CandidateListUIPresenter.cpp`）：

- `_SelectHighlightedHostCandidate` 改为接收回复类型和原始回复，由上面三步决定选哪个。数字和空格都经过它。
- 数字键（`_HandleCandidateWorker`）对所有决定性回复都照它选；宿主会话里找不到 `Normal` 回复的词时，和空格的兜底一样写 Server 的文字并丢掉宿主会话的组字（`_CommitServerExactText`），文档里不会少一个词。
- `_HandleCandidateSelectByNumber` 在宿主会话这一页比按键位置短时，仍去读回复跟着选，只是读不到回复时不再退回高亮项。

上屏的一致性只依赖文字和宿主会话自己的列表，与两个会话是否同页、是否同序无关。

## Alternatives considered

- **让宿主会话跟着 Server 的滚轮翻页** — 最强的理由是它对症：滚轮是两边分页对不上最确定的来源，同步之后连「按位置」都能对上。否决：Server 翻页时 TIP 没有编辑会话可用，要新增一条 Server → TIP 的翻页消息和对应的 owner 校验；即使做了，学习和云候选造成的顺序差异照样让同一页上的内容不同，按位置选仍会选错。按文字跟随与分页状态无关，两种来源一起解决。
- **Server 在回复里附上候选身份或读音长度，TIP 按它选** — 能区分同形不同读音的候选。否决：候选身份（代次、下标）是各会话自己的，跨进程套用正是 [2026-10-22](2026-10-22-windows-host-candidate-identity-by-text.md) 改掉的做法；附加读音长度要改固定 416 字节回复帧的载荷格式，而同形候选在同一读音下极少出现，现有的「不唯一就拒绝」已经够用。
- **数字键整个改走空格的 `_HandleCandidateFinalize`** — 一条路处理所有回复类型。否决：理由与 [2026-10-16](2026-10-16-windows-space-selects-in-host-session.md) 相同，日语的转换回执和 `TransportUnavailable` 的重放让空格必须保留自己的入口；两条路已经共用 `_SelectHighlightedHostCandidate`，只需让它理解所有回复类型。

## Consequences

- **收益**：数字键、二三候选键和空格在任何页上都上屏用户看见的那个词，翻页方式（滚轮、翻页箭头、键盘）、两个会话的顺序差异和云占位席位都不再影响结果；分段选词之后宿主会话的暂存词段和剩下的读音与 Server 一致，最后一次选词上屏一次完整的词。
- **代价**：完整候选列表里同形的不止一个时拒绝，数字键退回写 Server 的文字（`Normal`）或什么都不选（`NeedToCreateWord`）；当前页上找不到时多一次 `msime_client_all_candidates`，可能让宿主会话放出扣着的尾部、代次前进，随后的重画照常处理。
- **未处理**：繁体输出时 Server 的 `Normal` 回复是转换后的文字，而宿主会话的候选是简体，按文字找不到，现在由上面的兜底写 Server 的文字（结果正确，但宿主会话的组字被丢掉）。`NeedToCreateWord` 的暂存词段不经转换，不受影响。没有在真实 Windows 上验收。
- **重访信号**：如果以后要按身份而不是文字区分同形候选，或 Server 开始对候选文字做宿主会话没有的变换，需要的是共享的候选身份或把变换也放进 TIP，而不是放宽文字匹配。

## Verification

- `tests/input/shuangpin_paged_selection.cpp`（`windows-shuangpin-paged-selection`）：两份真实会话加合成词库 `tests/input/fixtures/msime-shuangpin-paging.db`，覆盖滚轮翻页后选单字再空格、滚轮翻页后选宿主会话当前页上没有的整词、键盘翻页后两种选词、Server 最后一页按键越界时宿主会话不选，以及回复解析的纯规则。把其中的跟随逻辑换成修复前的做法（`Normal` 只在当前页找整段文字，其他回复按位置选）时，五个场景里四个失败：滚轮翻页选「霓」变成「妮好」上屏、整词在宿主会话当前页上找不到、分段选词后的空格对不上、越界按键照样选了词。
- `msime-tsf-space-host-selection-wiring` 与 `gloss_column_wiring` 改为核对新接线；对修复前的 `CandidateListUIPresenter.cpp`，前者 13 条断言失败。
- `scripts/test-windows-native-run.py` 登记了新测试的伴随源文件和词库参数，在 macOS 上整套运行。TSF DLL 本身只做了 MinGW `-fsyntax-only` 检查，完整的 Windows 构建交给 CI。
