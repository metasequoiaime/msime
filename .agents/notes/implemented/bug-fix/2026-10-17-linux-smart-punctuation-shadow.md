# Agent Note: Linux surrounding text 不可用时保留智能标点前文

Status: implemented

## Problem

IBus 与 Fcitx5 在客户端不提供 surrounding text，或客户端暂时把它标为无效时，把智能标点前文当成空。数字或 ASCII 字母后输入 `.`、`,`、`:` 便会走中文标点路径，例如版本号 `0.10.1` 可能变成 `0。10。1`。macOS 和 Windows 已有只在本会话内维护的前文影子，Linux 两个宿主此前没有对应状态。

## Decision

两个 Linux 宿主各自维护一个最多 1024 字节的 UTF-8 末尾影子，只记录当前焦点会话由输入法自己提交的文本。surrounding text 有效时仍以文档字符为准；只有 surrounding text 无效且影子仍有效时，才用影子的最后一个 Unicode 字符判断 `smart_punctuation_direct_digit` 和 `smart_punctuation_direct_letter`。

影子在失焦、会话重置、应用接管按键、光标移动和删除时清空；IBus 收到有效 surrounding 更新时也清空，Fcitx5 则让有效 surrounding 优先于旧影子。输入法提交文本后重新记录末尾。输入法替换自己刚提交的标点时先从影子删除末尾字符，再记录替换文本。有效 surrounding 的文档起点、选择状态和越界光标也都优先代表宿主状态，不回退旧影子；只有真正无效或未发布 surrounding 时才使用影子。面板直接提交的文本同样记入影子。应用接管按键时只在按下事件清空，释放事件不影响刚由输入法提交的文本。这样不会把未知文档内容或已经移动后的旧 caret 位置当成可确认的 ASCII 前文。

IBus 在无 surrounding text 的情况下也用影子核对重复智能标点；Fcitx5 保持原有约定，把可以保留 ASCII 的标点交给应用。[提交前文时效](2026-10-20-fcitx-punctuation-after-commit.md)进一步规定：Fcitx5 的旧文档在发送提交前失效，客户端提交后新发布的有效文档仍优先，不能把提交前快照误认作新光标状态。

## Alternatives considered

- **直接把 `ai_context` 当作两个宿主的前文。** 它主要服务候选重排，生命周期和输入法提交语义并不等同于 caret 位置；把它直接用于标点会在光标移动、应用直通按键或有效 surrounding 更新后误判，因此采用独立、可失效的影子。
- **没有 surrounding text 时一律按中文标点。** 这能避免误判，但会让终端、Electron 等不能提供 surrounding text 的客户端无法输入版本号和 ASCII 标点；保守失效的影子能覆盖输入法自己确认过的前文。
- **追踪应用收到的每个直通 ASCII 按键。** IBus/Fcitx5 无法证明应用确实插入了该字符，也无法处理应用改写或拦截按键；影子只接受输入法自己的提交，未知文本继续交给 surrounding text。

## Consequences

- Linux 客户端不提供 surrounding text 时，输入法自己刚上屏的数字或字母后输入智能标点会保持 ASCII，两个宿主与 macOS、Windows 的行为一致。
- 光标编辑或应用直通后影子会被丢弃，缺少文档信息时宁可使用中文标点，也不会把未知字符当成 ASCII。
- 影子只存在进程内，不改变 Host API、偏好文件、候选上下文或隐私字段；上限为 1024 字节，非 ASCII 末尾不会被截成半个 UTF-8 字符。

## Verification

- `platforms/linux/tests/runtime/engine_smoke.cpp` 和 `platforms/linux/fcitx5/tests/native.cpp` 都覆盖：输入法生成 ASCII 数字、发送真实 Space release、使 surrounding text 无效、随后输入句号；Fcitx5 还覆盖有效空文档起点和面板 ASCII 文本；有效空文档起点在退出 Unicode 输入前发布，避免退出按键先清掉影子而让断言失去针对性。
- `git diff --check` 已通过。Linux 容器验收未能在本机启动，因为 Docker daemon 不可用；提交前由 Linux CI 编译并运行 IBus/Fcitx5 原生测试。
