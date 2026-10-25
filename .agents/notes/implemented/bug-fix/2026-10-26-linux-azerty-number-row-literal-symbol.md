# Agent Note: Linux 的 AZERTY 数字行在拼音组字中仍按物理键选词

Status: implemented

## Problem

legacy 包的运行时验收（`platforms/linux/tests/runtime/gtk_smoke.py`）在法语 AZERTY 布局下输入 `nihao`、按物理数字键 1，断言「French physical number row did not select candidate」失败，amd64 与 arm64 都是，#9755 和 develop 上的 Release Linux 因此红。

[特殊符号字面组字](2026-10-22-pinyin-literal-symbol-composition.md)让普通全拼、双拼在已有组字时接收 `# $ % &`，并把它们列进 `spelling_symbols`。两个 Linux 宿主在数字行选词之前先把 `engine_spelling` 的字符交给 Engine（IBus 的 `process_key`、Fcitx5 的 `FcitxState::key`），只为「拼写用数字的模式里 Shift 加数字行」留了例外（`shifted_number_row_picks`）。AZERTY 不按 Shift 时数字行 1 打出的就是 `&`，它被当成字面符号拼进组字，选词键失效。美式布局下数字行打出数字，不在列表里，所以只有 AZERTY 这类数字行上是符号的布局受影响。

`gtk_smoke.py` 只在 legacy 工作流里跑，而这个工作流只在它关心的路径变动时和每周定时触发；最后一次全绿是 2026-10-19，#9216 在 10-22 合入，中间没有一次运行碰到它。

## Decision

`SpellingSymbols.h` 的 `shifted_number_row_picks` 推广为 `number_row_picks`，两个宿主的调用点换成它。数字行的物理键在能选词时（数字行选词开着、不是九键、屏幕上有候选）选候选、不作为拼写交给 Engine，按物理键判断，不看布局在这个键上打出什么：

- 拼写用数字的模式（Unicode、表达式）里是 Shift 加数字行，规则不变。
- 其他模式里是不按 Shift 的数字行，哪怕打出的符号在组字中可拼（AZERTY 的 `&`）。之后两个宿主本来就按物理键选词（IBus 的 evdev 2–11、Fcitx5 的 XKB 10–19）。
- 方案自己拼写的符号（`scheme_spelling`：粤拼、藏文的撇号，注音、越南文的数字）和本地模式拼写的符号仍是输入，不选词。`engine_spelling` 里方案自身那一支提成 `scheme_spelling` 供两处共用，判断不变。

美式布局 Shift+7 打出的 `&` 按了 Shift，不算选词键，照旧按 #9216 拼进组字。

macOS 按物理键对应的数字字符查 `spelling_symbols`（`PhysicalCandidateDigitCharacter`），Windows 按虚拟键选词，都不受影响，不改。

## Alternatives considered

- **只排除 `# $ % &` 这四个字面符号**：改动最窄，直接针对 #9216。没有采用：把 #9216 的符号表复制进宿主，Engine 以后再给组字加可拼符号时会再坏一次；按物理键判断与 Windows 和两个宿主后面的选词代码是同一条规则。
- **让 Engine 不在组字中列出数字行上可能出现的符号**：宿主不用改。没有采用：Engine 不知道键盘布局，美式布局用户确实要在组字中输入 `&`（Shift+7）。
- **数字行在任何模式下都按物理键选词，包括方案自己拼写的符号**：规则最简单。没有采用：AZERTY 的数字行 4 打出撇号，粤拼和藏文拿它拼写，这样会让这些用户在候选打开时打不出撇号。

## Consequences

- AZERTY 等数字行上是符号的布局在拼音组字中恢复按物理数字行选词；这类布局上要在组字中输入 `&`，只能在没有候选可选时（例如关掉数字行选词），与 Windows 按物理键选词一致。
- 方案自身拼写的符号在数字行上仍是输入，粤拼、藏文的 AZERTY 用户不受影响。

## Verification

- `platforms/linux/tests/input/spelling_symbols.cpp`：新增拼音字面组字的用例（AZERTY 不按 Shift 的 `&` 选词；Shift、不在数字行、没有候选时仍是输入）和方案自身拼写的用例（藏文撇号、注音数字仍是输入）。以 `-DMSIME_EDITION_FULL -Wall -Wextra -Werror` 编译运行：新实现通过，换回旧 helper 后在 AZERTY `&` 的断言处失败。
- 在 Studio 上用 CI 同一套 `package-legacy-container.sh`（Debian 10、IBus 1.5.19、arm64），叠加 #9755 的聚焦提示修复跑完整的打包、ctest 和运行时验收：ctest 94 项通过，`gtk_smoke.py` 的法语物理数字行选词与其余运行时验收通过。未修复时同一套流程（只带 #9755）在 CI 的 amd64 与 arm64 上都停在这条断言。amd64 与 Fcitx5 宿主的运行时没有单独在本机跑，交给 CI。
