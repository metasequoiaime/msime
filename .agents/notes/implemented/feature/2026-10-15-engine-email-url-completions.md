# Agent Note: 引擎网址模式补全邮箱后缀、网址开头和顶级域名

Status: implemented

## Problem

产品要求：打 `@` 时候选里联想主流邮箱后缀；打 `http` 之类时候选里给出 `http://`、`https://` 等补全。要在引擎里做，让每个平台的候选栏不改宿主就能显示。

此前网址模式只在组字原文恰好是 `www`、`http`、`https`、`ftp` 且按下 `.` 或 `:` 时进入，模式里没有候选，只有一行显示整段网址的兜底行。组字后按 `@` 走标点路线：runtime 先结束组字上屏首选，再写 `@`，引擎没有机会把它当成邮箱。只有 Android 在系统标记的邮箱输入框里由宿主自己给后缀（[Android 邮箱后缀](2026-10-09-android-email-suffixes.md)）。

## Decision

### 进入邮箱

全拼、双拼、五笔（`SchemeType::detects_urls`）里，组字原文非空、只有 ASCII 字母和数字、光标在末尾、不在本地模式或专用英文时，`@` 把组字转成网址模式，预编辑是 `<原文>@`。规则在 `local::url::opens` / `entry_keys`：触发词表 `TRIGGERS` 不变，邮箱的键单独是 `EMAIL_KEY`。组字时 `spelling_symbols` 因此多一个 `@`（`www` 发布 `.@`，`ni` 发布 `@`），runtime 才会把 `@` 当字符送给引擎，而不是先结束组字。带撇号的原文（`ni'hao`）不算，`@` 照旧结束组字。空闲时的 `@` 仍是名单模式的入口，不变。

退格的逆操作扩展到 `@`：删掉末尾的 `@` 退回原来的组字。退回现在要求删掉的是末尾那个字符（`caret == text.len()`），`abc@def` 删掉中间的 `@` 留在网址模式；进入网址模式总是在末尾追加一个键，所以只有删末尾才是逆操作。

### 网址模式里的候选

列表的第一行始终是用户打的整段网址（兜底行），补全排在它后面：

- 邮箱后缀（`url::email_completions`）：预编辑是 `<本地部分>@<已打的域名>` 时，给出以 `@<已打的域名>` 开头（不分大小写）的后缀，按 `EMAIL_SUFFIXES` 的顺序；已打的域名已经是完整后缀时不给。规则与 Android `EmailSuffixPolicy.match` 相同，后缀表是同一份、同一个顺序，`scripts/test-email-suffixes.py` 核对。选中即上屏整个邮箱并结束网址模式。
- 顶级域名（`url::tld_completions`）：预编辑以 `www.` 开头或含 `://`，正在打的主机名（去掉可选的 `www.`）非空、只有字母数字和 `-` 时，给出加上 `.com` `.cn` `.net` `.org` `.com.cn` 的五行。选中不上屏：以它为新预编辑留在网址模式，用户还可以接着打路径。

整段网址留在第一行，是因为空格、失焦和结束网址的标点都上屏高亮的那一行（默认第一行）。补全排在前面的话，`https://localhost` 按空格会变成 `https://localhost.com`，失焦时也会悄悄补上 `.com`。补全只在明确选中（点按、方向键加空格）时才用。

数字在网址模式里是输入，不选候选：`SPELLING_SYMBOLS` 已经列出数字和 Shift+数字行的全部符号，引擎收下数字，runtime 和宿主按 `spelling_symbols` 不把它们当选词键。补全显示着时也一样，选补全靠点按或方向键。

### 组字里的网址开头

组字原文是 `https`、`http`、`www`、`ftp` 的前缀且至少两个小写字母时（`ht`、`htt`、`http`、`https`、`ww`、`www`、`ft`、`ftp`），混排列表里接上 `https://`、`http://`（`https` 只给 `https://`）、`www.`、`ftp://`（`url::scheme_completions`）。第一行是词典、用户词典或英文词典的词时补全排在它后面（五笔 `www` 的“众”仍是首选），否则排在最前面。选中补全不上屏，以它为预编辑进入网址模式（`InputSession::url_continuation` → `enter_url_mode`）。

结束组字（`finish_composition`：标点、失焦、宿主的结束键）时高亮停在补全上不算选中：上屏补全之外的第一行，没有就上屏原文。五笔四码 `http` 不加补全，因为 runtime 的顶字在第 5 个字母上选第一行，补全领先时 `http` 加 `s` 会变成 `https://s`，而 `http` 加 `s` 本来直接进入网址模式。

### 行的来源和重排

所有补全行的来源是 `CandidateSource::Fallback`，不是 `Generated`：runtime 的整句读法降位（`runner_up_order`）把第一个 `Generated` 行的字数当作词网格整句的宽度，同宽的补全会被当成整句挪位，混排列表里真正的整句读法也会因此认错宽度。不新增来源值：来源是宿主 ABI 的数字，新增一个要改各宿主和 `candidate-sources` 契约，而这些行在宿主看来就是普通候选。补全行的 `candidate_answers_key` 为假（补全不是回答了按键，而是越过它），重排模型不拿它们比较，也不会把它们提到前面。补全行的识别不靠来源单独判断，而是按当前原文、预编辑重算一遍补全再比对（`is_url_completion`、`url_continuation`），不在会话里另存状态。

### Windows TIP

TSF DLL 在 Server 回答前就要给按键分类，`platforms/windows/tsf/Global/LocalModeKeyPolicy.h` 保留了网址进入规则的拷贝。这份拷贝同步加上邮箱的键（`UrlEmailKey`、`IsEmailLocalPartAndKey`），`UrlModeAfterDeletion` 同样只在删末尾时退回；`scripts/test-windows-expression-symbols-parity.py` 多核对一项 `EMAIL_KEY` 与 `UrlEmailKey` 相同。

## Alternatives considered

- **补全排在整段网址前面（第一行就是 `zhangsan@qq.com`）** — 最贴近“打 @ 就联想后缀”的直觉，按空格直接得到最常用的后缀。但空格、失焦和结束网址的标点都上屏第一行，顶级域名补全领先时 `https://localhost` 会被改成 `.com`，打一半的域名也会被替换，这是悄悄改用户打的字。网址模式里数字不能选词，键盘上的选择本来就要靠方向键，让默认行保持用户打的内容更稳。
- **新增 `CandidateSource::UrlCompletion`** — 语义最明确，选择、重排和宿主都能按来源识别。代价是来源数字是宿主 ABI：macOS、Windows、Android、iOS、鸿蒙、Linux 都按数字做事，`scripts/test-candidate-sources.py` 和 Windows 的右键菜单策略也要跟着改。这些行在宿主看来与普通候选没有区别，引擎按原文重算就能识别，不值得为此动 ABI。
- **用 `Generated` 作来源（与组字里插入的日期时间行一样）** — 最省事。但 `Generated` 是 runtime 整句读法降位的依据（`LATTICE_SOURCE`），补全行会被当成整句读法挪位，同一列表里真正的整句读法也会认错宽度。
- **选中顶级域名直接上屏** — 实现最简单，与邮箱后缀一致。但网址后面常常还要接路径（`github.com/...`），上屏后就接不上了；留在网址模式再按空格上屏只多一步。
- **`@` 只在原文不是完整拼音时才进入邮箱** — 能保住 `xiexie@` 这类打完拼音直接接 `@` 的旧行为。但邮箱的本地部分本来就常是拼音名（`zhangsan`），分不出来；误进入后退格一次就回到原来的组字，可以选回汉字。

## Consequences

- **收益**：全拼、双拼、五笔的所有平台在组字后打 `@` 都有邮箱后缀候选，打 `ht`、`www` 之类有网址开头的补全，打主机名时有顶级域名补全，不需要宿主改动（Windows 只同步了按键分类规则的拷贝）。后缀表与 Android 宿主是同一份，由脚本核对。
- **代价**：拼音后直接按 `@`（`xiexie@`）不再上屏“谢谢@”，而是进入网址模式；退格一次回到组字。组字时 `spelling_symbols` 多一个 `@`，快照多一次分配（两处分配预算测试各加一）。`ww`、`ft` 这类短组字的候选里多一两行补全。
- **已知上限**：Windows TIP 自己维护键击缓冲和网址模式标记，只跟踪按键，不从 Server 的视图同步。键盘打出的邮箱（`@` 进入）两边一致；但点选组字里的 `https://` 或网址模式里的顶级域名时，Engine 的预编辑整段换掉而 TIP 的缓冲和网址标记没有跟上，之后的 `.`、数字可能在 TIP 里被当成翻页或选词。这条路径在 Windows 上没有验证；要支持它，TIP 需要在选择回复后按视图的 `local_mode` 和 `editing_text` 重建缓冲。

## Verification

- `cargo test -p msime-engine --locked`：`local::url::tests` 覆盖进入键、网址开头、邮箱和顶级域名补全的规则；`session::tests` 的 `url_at_after_a_local_part_offers_email_suffixes`（进入、20 个后缀、按已打域名缩小、完整后缀不给、数字是输入、选中上屏、空格交还 runtime）、`url_email_backspace_over_the_at_returns_to_the_composition`、`url_at_needs_a_plain_local_part`、`url_scheme_completions_enter_url_mode_when_picked`、`url_scheme_completion_is_not_taken_by_finishing`、`url_wubi_keeps_its_word_ahead_of_the_scheme_completion`，原有网址测试只改了 `spelling_symbols` 和补全行带来的期望。golden 基准不涉及网址，没有变化。
- `cargo test -p msime-input-runtime --locked`：`url_email_suffixes_leave_digits_and_space_to_the_address`、`url_completions_picked_through_the_runtime_keep_composing` 走真实引擎验证数字不选词、`SelectHighlighted` 上屏打的网址、选补全不上屏。
- `scripts/test-email-suffixes.py`、`scripts/test-windows-expression-symbols-parity.py`；`platforms/windows/tsf/tests/input/local_mode_key_policy.cpp` 用 clang 在本机编译运行通过。没有在任何平台的真实宿主里装机验证。

## Related notes

- [Android 邮箱后缀](2026-10-09-android-email-suffixes.md)：同一份后缀表和匹配规则，Android 在邮箱输入框里由宿主补全；邮箱输入框进框时切英文，引擎的网址模式不参与，两者不会同时出现。
- [网址退回组字复用剩余文本](../testing/2026-10-08-url-reversion-buffer.md)：退回组字的路径（`restore_composition_from_url`）不变，退回条件多了“删的是末尾”。
