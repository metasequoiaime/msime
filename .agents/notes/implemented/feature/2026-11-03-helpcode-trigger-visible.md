# Agent Note: 辅助码前导键在组字区显示

Status: implemented

## Problem

#10731：设了辅助码触发键（`helpcode.trigger_key`，例如 `;`）之后，输入时按下 `;`，组字区什么也不显示；还没输码时按退格把它删掉，同样没有任何反馈。用户看不出自己是否已经进了辅助码。

原因在 Engine：前导键只是会话里的 `helpcode_entry` 状态，按[Linux 前导键](2026-10-22-linux-helpcode-trigger-key.md)的决定不写进原文，快照里也没有任何字段反映它。三个提供触发键的宿主（macOS、Linux 的 IBus 和 Fcitx5、HarmonyOS）画的不是 `preedit` 就是 `editing_text`，两者都不含前导键。行内样式 `tsf_preedit_style` 出厂是 `raw`，画的是 `editing_text`，所以光改 `preedit` 不够：默认设置下行内仍看不见，只有候选窗的拼音行能看到。

## Decision

只改显示，前导键仍不进原文。`editing_text`、`caret_position`、回放比对用的 `Session::snapshot().preedit`、退格、选词、退出和辅助码筛选都不变。

- **Engine 显示快照**：`SessionSnapshot::helpcode_trigger` 在前导键还在等辅助码（已输 0 或 1 位）时是 `HelpcodeTrigger { key, position }`，`position` 是它在 `editing_text` 里的字节位置，其余时候为 `None`。`display_snapshot` 和给宿主的快照把它插进 `preedit`，放在末尾已输入的辅助码前面：`ni;`、`ni;F`、双拼拼音分词的 `ni;f`。引擎为隔开辅助码插的 `'` 由前导键顶替（`ni'F` 显示成 `ni;F`），用户自己打的 `'` 保留（`ni';F`）。`preedit` 末尾对不上辅助码时不插。
- **两位码齐了就不再显示**：`helpcode_entry` 要到下一键才清掉，如果跟着它显示，`ni;FR` 会在打下一个字母时变成 `niFRh`，分号看起来像被那个字母吃掉了。所以两位码齐了就显示成原来的 `niFR`；退格回到一位码时分号重新出现。
- **视图契约**：host 快照和 input-runtime 的 `View` 透传 `helpcode_trigger`，JSON 形如 `{"key": ";", "position": 2}`，没有时省略这个键。`preedit` 已经带着前导键；画 `editing_text` 的宿主把 `key` 插在 `position` 处，`caret_position` 不小于 `position` 时光标后移 `key` 的长度。前导键只在光标位于末尾时生效，按键原文是 ASCII，同一个字节位置在各宿主的字符单位里都成立。契约写在 `msime_client.h` 的 `View.helpcode_trigger`。
- **宿主**：macOS `TextClient` 的 raw 分支，Linux 的 Fcitx5 和 IBus 画原文的分支（共用 `platforms/linux/src/core/HelpcodeTrigger.h`），HarmonyOS 的 `drawnReading` / `drawnCaret`（经 `HelpcodeTriggerPolicy`）。鸿蒙的读音行和 2in1 原始按键样式画的都是 `drawnReading`，所以一处改动同时覆盖两处。拼音样式和候选窗的拼音行读 `preedit`，不用改。Windows、Android、iOS 不提供触发键，没有改；engine-capi 的 C 结构和网页引擎也不带这个字段。

## Alternatives considered

- **把前导键写进原文**：最直观，所有画原文的地方自动看得见。但 `editing_text` 是编辑和光标的坐标系，回车（`CommitRaw`）会把 `;` 上屏，辅助码的大小写协议、回放金样例和光标映射都按「原文不含前导键」写成，改动面远大于显示问题本身。
- **只改 `preedit`（分诊时的方案）**：改动只在 Engine。但出厂行内样式是 `raw`，画的是 `editing_text`，默认用户在行内仍然看不见，只解决了候选窗拼音行。
- **给宿主一个现成的显示串（如 `editing_display`）**：宿主直接画，不用自己插。但宿主的光标都按 `caret_position` 算，显示串还要再配一个光标字段；而且 Linux 的光标按字节、IBus 按标量、鸿蒙按 UTF-16，`key` 加 `position` 加一条「光标不在它前面就后移」的规则，比多一个随时要和 `editing_text` 对齐的字符串更不容易出错。
- **跟着 `helpcode_entry` 一直显示到下一键**：实现最简单。但分号会在打新拼音的那一键突然消失，见上。
- **换一种样式区分前导键**：用户也希望这样。但需要各宿主给 marked text、fcitx::Text、IBusText 和鸿蒙预上屏文字分别加段属性，三端各一套，这次不做。

## Existing notes

[Linux 前导键](2026-10-22-linux-helpcode-trigger-key.md)拥有前导键的输入语义，部分重叠：本篇只加显示，「前导键不写入原文」仍然成立，那一篇加了指向本篇的链接。[鸿蒙辅助码按键接通](../bug-fix/2026-10-31-harmony-helpcode-entry.md)负责鸿蒙把前导键送进 Engine 的路由，与显示无关。[直接、间接与句中触发](2026-10-22-shuangpin-helpcode-trigger-modes.md)的段标记不经 `helpcode_entry`，不受影响。

## Consequences

收益：设了触发键的用户在默认设置下，行内和候选窗拼音行都能看到 `ni;`，退格后它跟着消失。显示规则只在 Engine 一处，宿主只按契约插一个字符。

代价与边界：

- 脚本插件经 `Composition.preedit` 读到的预编辑串会带着前导键，与屏幕上画的一致；光标仍按原文算。
- Linux 候选辅助行只在 `preedit` 等于 `editing_text` 时画 `|` 光标标记，带前导键时两者不等，这段时间不画标记。光标本来就在末尾。
- 前导键没有单独的样式，和拼音一样画。

验证：Engine 单测覆盖全拼、小鹤和微软双拼的三种双拼显示（原始按键、原始按键分隔、拼音分词），从按下前导键、一位码、两位码到逐个退格的每一步，以及微软半音节的 `;` 仍是韵母、用户自己打的 `'` 和别的前导键；回放快照的 `preedit` 不含前导键。host 快照测试核对 `preedit_caret` 落在末尾，input-runtime 测试用真实 Engine 核对视图 JSON。宿主各有一条测试：macOS `TextClientTest` 的 raw 和 pinyin 两种样式及非法字段，Linux `linux-helpcode-trigger`，鸿蒙 `helpcode-trigger.test.ts`。没有在真机或真实编辑器里按过，留给设备验收。
