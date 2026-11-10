# Agent Note: Fcitx5 切换输入法时保留中文组字

Status: implemented

## Problem

Fcitx5 的 `InputContextSwitchInputMethod` 停用路径会关闭水杉会话。普通全拼和双拼的预编辑由插件维护，切换到另一个输入法时 Fcitx5 不保证把这段客户端预编辑交给编辑器；直接关闭会话会丢掉用户已经输入的读入串。韩文、注音、越南文和藏文等 `commits_on_blur` 方案已有独立的失焦上屏规则，不能与普通中文路径叠加。

## Decision

Fcitx5 在 `InputContextSwitchInputMethod` 事件中，如果当前是普通中文组字且 `editing_text` 非空，先调用 `MSIME_COMMIT_RAW`，把用户输入的原始读入串交给当前编辑器，再关闭会话。`commits_on_blur` 方案继续调用 `MSIME_FINISH_COMPOSITION`，保持其已有的方案文字上屏语义并避免重复提交。普通焦点离开仍由 Fcitx5 的客户端预编辑机制处理，不在本分支重复上屏。

## Alternatives considered

- 依赖 Fcitx5 或前端自动提交客户端预编辑：行为取决于前端能力和输入框实现，不能保证普通中文预编辑在切换输入法时进入文档。
- 对所有方案统一调用 `MSIME_COMMIT_RAW`：会破坏韩文、注音、越南文和藏文已有的失焦上屏语义，并可能造成重复提交。

## Consequences

Fcitx5 中输入 `nihao` 后切换到另一个输入法会保留并上屏 `nihao`，不会自动选择高亮的中文候选。只改 Fcitx5 的输入法切换事件；水杉内部中英文切换和 IBus 行为保持不变。当前仍需在真实 Fcitx5 容器中补跑切换输入法的编辑器冒烟，确认具体前端不会再次提交同一段客户端预编辑。
