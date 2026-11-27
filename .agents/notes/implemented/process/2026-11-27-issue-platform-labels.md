# Agent Note: 直接在 GitHub 提交的 issue 自动补平台标签

Status: implemented

## Problem

Issues 页的共享视图（Windows、macOS、Linux、Android、iOS、HarmonyOS）按 `platform/*` 标签加 `pc` / `mobile` 筛选，漏标的 issue 在任何一个平台视图里都看不见。PR 的平台标签由 `pr-triage.yml` 按改动路径自动打；issue 的平台标签只有两条来源：官网需求表单提交时由 msime-web 打上，其余靠维护者手动补。直接在 GitHub 上提交的 issue（Bug 模板、功能建议模板、空白 issue）没有任何自动来源。2026-11-27 统计最近 100 个 issue，有 38 个没有任何平台标签，其中二十多个的标题或「系统与输入法版本」一栏里写明了平台，例如 #11896「Rust 版水杉输入法中文模式 + 云候选时……」写着 Windows，#11821「ArchLinux下候选框高度不正常」。

## Decision

新增 `.github/workflows/issue-triage.yml`，在 issue `opened` 和 `edited` 时运行：

- issue 已有任意平台标签（`platform/*`、`pc`、`mobile`）就什么都不做。官网和维护者打的标签说了算，workflow 只补空缺，从不删、也不在已有标签旁边追加。
- 只在标题、Bug 模板的「系统与输入法版本」和功能建议模板的「输入法版本」三处找平台名（Windows / Win10 / Win11、macOS / Mac、Linux 及 Ubuntu、Fedora、Debian、Arch、fcitx、ibus、Wayland、Android / 安卓、iOS / iPadOS / iPhone / iPad、HarmonyOS / 鸿蒙），认出几个打几个。拉丁字母的平台名按词边界匹配，避免把 `search`、`machine` 误认成 Arch、Mac。
- 不认 `pc`、`mobile` 这类大类标签：标题里的「电脑」「手机」多数是在描述场景，不是限定平台。

模板不变，仍按[功能建议模板按场景拆问，平台不进模板](2026-10-10-feature-request-template-fields.md)不设平台字段。

## Alternatives considered

- **两个模板各加一个必填的「使用的平台」多选框，workflow 按选项打标签**：最准确，选项和标签一一对应，不需要从自由文本里猜。这次最初就是这样写的，读到 10-10 的笔记后放弃：官网需求表单读取这些模板现场渲染，用户在表单顶部已经选过平台，模板再加字段会让他们选两次、两处还可能不一致，要么官网特判这个字段 id，等于在两个仓库之间新增一条隐式契约。要翻转那篇笔记的决定，需要维护者另行拍板。
- **扫描整篇正文**：覆盖面更大，功能建议模板的场景描述里也常写到平台。但正文里拿别的平台作对比很常见（「macOS 上没问题」「像安卓版那样」），按正文匹配会把这些平台也打上，漏标变成错标，比不标更难发现。
- **已有平台标签时也追加新认出的平台**：能补全只标了一半的 issue。但官网和维护者打的标签是有意为之，自动追加会和人的判断打架，而且用户每次编辑 issue 都可能再追加一次。

## Consequences

直接在 GitHub 提交、且标题或版本栏写了平台的 issue 会在提交后几秒内带上平台标签，进入对应的共享视图；写不出平台的 issue（例如「五笔同词不同码的自学习把全码候选提前」）照旧等维护者补。用户编辑 issue 去掉某个平台时，已打的标签不撤。版本栏同时写了开发机和目标平台时会两个都打，例如 #11912 的 iOS 构建问题会带上 macOS。

## Verification

用 2026-11-27 最近 100 个 issue 回测同一套规则：59 个已有 `platform/*` 标签的 issue 里，41 个认出的平台与已有标签完全一致，18 个认不出、不打，误标 0 个；38 个没有平台标签的 issue 里，二十多个能认出平台。`actionlint` 检查 `issue-triage.yml` 通过。workflow 本身还没有在真实 issue 事件上跑过，合并后的下一个新 issue 是第一次。
