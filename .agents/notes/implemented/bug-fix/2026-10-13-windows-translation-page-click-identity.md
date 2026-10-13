# Agent Note: Windows 释义页的点击带上释义页编号

Status: implemented

## Problem

Ctrl+Enter 在高亮候选有多条释义时打开释义页（`ReplyComposer::commit_candidate_translation`）。这一页不是 Engine 的候选：它拷贝 Engine 当前 view，把候选换成各条义项，id 沿用 Engine 列表的 `session` 和 `generation`，序号从 0 起。释义页打开期间 `select_candidate` 不再查 Engine 候选，直接上屏 `translation_page_items_[index]`。

鼠标点击的身份是 `CandidateClick{lease, session, generation, index}`，按候选窗最近一次画出的那一帧拼出（`CandidateWindow::hit`），Server 在 `SessionController::request_selection` 里对照 `session`、`generation` 和画出来的候选核对。用户瞄准 Engine 列表 A 的第 i 行点下去，点击还在 `CandidateClickWorker` 里时 Ctrl+Enter 换上了释义页 B：B 的 `session`、`generation` 和 A 一样，第 i 行的身份也一样，于是每一道核对都通过，上屏的是 B 的第 i 条义项，不是用户点的那个候选。反过来也一样：释义页关掉、Engine 列表在同一 `generation` 下重新画出来时，瞄准 B 的点击会选中 A 的同一位置。右键菜单的置顶、删除、固定排位，以及维护快捷键的删除候选，走的是同一组身份，在释义页上会落到底下 Engine 列表同一序号的候选上。

## Decision

释义页有自己的编号，随 view、展示和点击一路带着，Server 两处都核对：

- `ReplyComposer` 每打开一页取一个新编号（`translation_page_serial_`，只增不减），写进这一页 view 的 `translation_page`；`translation_page()` 返回当前打开的那一页的编号，没有打开时为 0。
- `CandidatePresentation::translation_page` 从 view 读出（`candidate_presentation` 和 `candidate_presentation_from_view` 都读，Engine 的 view 没有这个字段就是 0）。释义页上的行 `actions_available` 为 false，菜单不给置顶、删除、固定排位。
- `CandidateClick::translation_page` 由 `CandidateWindow::hit` 和读屏执行（`invoke_accessible`）从画出来的那一帧拷进去；`CandidateMenuTarget` 拷整个点击，菜单动作也带着它。
- `SessionController::request_selection` 和 `request_candidate_action` 新增 `translation_page` 参数（`WindowsServer` 同步），读屏前后两次取到的展示都要求编号相等；候选动作在 `translation_page` 不为 0 时直接拒绝。
- `ReplyComposer::select_candidate` 要求点击带来的编号等于 `translation_page()`，这一道在输入线程上、和按键同一个顺序里核对，挡住「SessionController 核对完之后、输入任务执行之前换了页」的竞争。`FocusedSession::candidate_action` 在释义页打开时一律不做。

已有的 `session`、`generation`、租约和画出来的候选核对都保留，编号是在它们之上再加的一道。

### 释义页开着时，画面和 Server 的状态保持一致

释义页是盖在 Engine 列表上的一层，从打开到下一个按键把它关掉为止（`translation_page_key`：方向键、翻页键原样回这一页，数字和空格上屏义项，其他键关掉它）。代码的本意是翻页在这一层上不起作用，所以鼠标和滚轮也照此处理，而不是翻完再把释义页关掉：

- 翻页不起作用。`CandidatePage` 带上发起那一帧的 `translation_page`；`SessionController::request_page` 拒绝不为 0 的请求，前后两次取到的展示的编号也要和请求一致；`FocusedSession::page_candidate` 在 `translation_page()` 不为 0 时一律不翻，挡住按列表那一帧发起、在释义页打开之后才到的请求。释义页的 view 去掉 Engine 列表的 `page`、`page_count`，候选窗不在它上面画页码和翻页箭头。
- 异步刷新不盖掉释义页。`CandidateMailbox::refresh_view`（释义、云候选、AI 候选、候选动作之后的重建）在画着的是释义页时什么也不做；关掉释义页的那个按键的投递照常带回 Engine 的列表，期间应用到会话里的释义也随它一起显示出来。
- 云候选、AI 候选到达会让 Engine 换代次，而画面上仍是释义页，所以 `ReplyComposer::select_candidate` 在释义页打开时按释义页 view 里的 `generation` 核对点击，不按 Engine 的当前代次。

## Alternatives considered

- **给释义页一个不同的 `generation`** — 不用新增字段，点击结构不变，现有的 `generation` 比较自然就把两张表分开。没有采用：`generation` 是 Engine 的代次，`select_candidate` 和 `confirm_ui_delivery` 都拿它和 Engine 的 view 比；合成一个代次要么可能和 Engine 之后真实的代次撞上（瞄准释义页的旧点击又能选中 Engine 的新列表），要么得占用高位做标记，等于在 Engine 的身份里塞进宿主自己的含义。
- **让点击带上 `render_serial`** — 每一帧都有唯一编号，什么样的换页都分得开。没有采用：释义、读音、云候选在同一 `generation` 上到达时都会换编号，用户点下去的一刻刚好到一条释义，点击就被拒，要多点一次；这次要分开的只是「Engine 列表」和「释义页」两张表。
- **释义页打开时干脆不接受鼠标点选** — 改动最小。没有采用：释义页的行画得和候选一样，点它上屏是现有行为，键盘数字和点选应当一致。
- **任何东西把画面换成 Engine 列表时就关掉释义页**（翻页、释义刷新都算） — 画面和状态同样一致，也不必让 `select_candidate` 认释义页的代次。没有采用：打开释义页的那个按键自己的回复带着一次释义查询（`attach_online_query`），`TranslationWorker` 不去重，结果回来时就是一次同代次的刷新，于是开着翻译服务时释义页刚画出来就会被关掉；方向键、翻页键在释义页上本来就不起作用，翻页让它关掉也和键盘不一致。

## Consequences

- **收益**：瞄准 Engine 列表的点击不会上屏释义页同一位置的义项，瞄准释义页的点击也不会选中 Engine 列表；释义页上不再出现会作用到底下 Engine 候选的菜单动作。
- **代价与已知上限**：点击、翻页和选择接口多一个参数，所有调用方（含测试）一起改。释义页开着时到达的释义、云候选不会马上显示，要等按键关掉释义页；释义页上滚轮和翻页什么也不做。

## Verification

- `platforms/windows/tests/runtime/session_workers.cpp`（`windows-session-smoke`）：Unicode 模式组出「中」，给它挂上三条义项后 Ctrl+Enter 打开释义页；按列表那一帧拼出的点击被拒、不写出任何帧，两种候选动作都被拒；释义页自己的点击上屏它点的那条义项。拿掉 `SessionController` 和 `ReplyComposer` 里的编号核对后，这一段在 Wine 下失败。
- `platforms/windows/tests/core/ui_selection.cpp`：没有打开释义页时带编号的点击被拒。
- `session_workers.cpp` 的第二段：日期时间模式（Shift+T、`rq`，每页两个，有好几页）打开释义页，释义页不带页码；按列表那一帧和按释义页那一帧发起的翻页都被拒，画面仍是释义页，数字键 2 上屏画着的 `middle`。拿掉翻页的拦截后在 Wine 下失败。
- `platforms/windows/tests/ui/candidate_mailbox.cpp`：画着释义页时同代次的释义刷新、换代次的云候选和候选动作重建都不盖掉它，按键投递换回 Engine 列表。拿掉 `refresh_view` 的拦截后在 Wine 下失败。
- `platforms/windows/tests/input/gloss_column_keys.cpp`：释义页开着时 AI 候选到达、Engine 换了代次，按释义页拼的点击照常上屏 `hi`。改回按 Engine 代次核对后失败。
