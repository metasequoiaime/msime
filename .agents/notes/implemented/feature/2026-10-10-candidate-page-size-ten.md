# Agent Note: 每页候选数上限放宽到 10，按宿主能力截断

Status: implemented

## Problem

#6679：共享偏好 `candidate_page_size` 只接受 1–9，各平台的设置页也只给到 9。issue 要求至少 10 个：数字行有 0–9 十个键。

上限 9 写在很多地方：client-core 的校验和错误文案（鸿蒙按这句文案认错误码）、input-runtime 和 host-api 的页大小检查、网页引擎的 `MAX_PAGE_SIZE`、一页候选的翻译批量（腾讯、学习译文、翻译计划都是最多 9 条）、macOS 原生设置与云外观校验、Linux 的候选数量菜单，以及每个宿主「数字键选第几个」的映射。

Windows 宿主更深：TSF 只把 `1`–`9` 分类为选词键，`0` 在组字中不经过选词；候选窗口、`CandidatePresentation`、`CandidateCardSize`、`ReplyCodec` 都把多于九个候选的一页当作非法输入拒绝。只放宽共享校验的话，用户在 macOS 上选 10，同步到 Windows 后候选窗口就画不出来了。

## Decision

- client-core 新增 `preferences::MAX_CANDIDATE_PAGE_SIZE = 10`，校验、错误文案（`candidate page size must be between 1 and 10`）、input-runtime、host-api 的 `msime_client_set_candidate_page_size` 和页内翻译批量都用它。网页引擎不依赖 client-core，`MAX_PAGE_SIZE` 写字面量 10，测试核对两边相等。
- 新增宿主能力 `HostCapabilities.max_candidate_page_size`，取值来自 `HostPlatform::max_candidate_page_size`：
  - macOS、Linux、Android 是 10；
  - Windows、iOS、鸿蒙是 9。
- host-api 按本库编译到的平台（`compiled_platform`，原来只给主题目录用）截断会话实际用的页大小：建会话、偏好变更后重建 Engine、宿主覆盖页大小，三处都截断。共享文档存了 10，只排得下九个的宿主仍拿到 9，文档本身不改写。
- 共享设置页的滑块上限取 `max_candidate_page_size`，宿主没报告时按 9（和以前一样）；存着的值超出范围时把滑块放宽到它，不改写，和原来处理 1、2 的方式一致。
- `0` 选第十个：
  - 只在每页十个时才算选词键（input-runtime 的 `candidate_digit_slot`、macOS 的 `CandidateDigitSlotOnPage`）。每页不到十个时 `0` 照旧交给 Engine 或应用，现有用户的 `0` 键行为不变。
  - Android 的数字行把 `KEYCODE_0` 映射到第十格，调用方原本就按 view 里有没有这一格决定选不选，没有时把键交出去。
  - Linux 的 IBus 和 Fcitx5 早就把 `0` 映射到第十格。
  - 网页引擎在每页不到十个时本来就吞掉 `0`，现在每页十个时它选第十个。
- 第十个候选的序号标 `0`，和选它的键一致：macOS 候选面板、Linux IBus 和 Fcitx5、Android 实体键盘候选条都这样标。
- 被注音、Unicode 模式等列为拼写的 `0` 仍是输入，不是选词键（`spelling_symbols` 的检查在映射之前）。

## Alternatives considered

- **只放宽共享校验，各宿主各自跟进。** 最强的理由是改动最小，Windows 的事留给 Windows。没有采用：共享偏好会随账号同步、MCP 和共享设置页写入，任何一端存下 10，Windows 的候选窗口就按「多于九个」整页拒绝，属于数据驱动的崩坏，不能留给以后。
- **这次就把 Windows 也做到 10。** 理由是 Windows 是主力平台，issue 很可能来自 Windows 用户。没有采用：TSF 的按键分类走延迟投影的影子状态，不知道当前页的大小；要让 `0` 在每页十个时选词、其余时候不变，得把页大小送进 TSF 的状态机，再放宽候选窗口和编码里的四五处校验。这台机器上没有 MinGW 和 Windows，这些改动没法编译也没法跑 Wine 测试，盲改 TSF 的风险大于收益。留给单独的 PR。
- **把 0 无条件当作第十格（页里没有就吞掉），和 1–9 一样。** Linux 一直是这样，规则更整齐。没有采用：macOS、Android 和桌面 runtime 上，`0` 在组字中现在会交给 Engine 或应用；每页九个以下（默认 6）的用户按 0 的结果会变，而他们没有改任何设置。
- **把上限做成 Windows 也能在设置里选、只是显示 9。** 会让设置页上的数字和候选窗口对不上，不如在能力里直接说清楚上限。

## Consequences

- 收益：macOS、Linux、Android（实体键盘）可以每页十个候选并用 0 键选第十个；共享校验、设置页、MCP 文档、网页引擎一致放宽到 10。
- 代价：
  - 平台之间出现差异：Windows、iOS、鸿蒙最多 9 个。设置页按宿主能力显示，不会出现选了不生效的情况；别的设备同步来的 10 在这三个宿主上按 9 显示。
  - `HostCapabilities` 多了一个必填字段，用 `deny_unknown_fields` 解析它的旧代码要随之更新（仓库里只有 client-core 自己的往返测试）。
  - 鸿蒙按文案认错误码，文案改了，`PreferencesErrorCode` 一起改。
  - Linux 翻译覆盖层（选候选的释义列表）仍按每页最多九个分页，存的是 10 时那一层显示九个，选词不受影响。
- 未验证：macOS 云外观同步的服务端字段范围（`platform.macos.candidate_page_size`）不在本仓库，服务端若只收 1–9，macOS 上选 10 后云同步会被拒。
- 后续：Windows 要做到 10，需要 TSF 的影子状态带上页大小、`KeyEventSink` 把 `0` 在每页十个时分类为选词键、服务端 `ServerSession` 的数字路由接受 `0`，再放宽 `CandidateWindow`、`CandidatePresentation`、`CandidateCardSize`、`ReplyCodec` 的九个上限，最后把 `HostPlatform::max_candidate_page_size` 的 Windows 改成 10。
