# Agent Note: 全拼宿主预编辑使用引擎分词

Status: implemented

## Problem

#9406 的 macOS 候选预编辑设置为 `pinyin`，输入 `nihao` 仍显示连写。底层全拼请求已经带有分词，宿主出口却沿用原始 `preedit`，普通候选窗、展开候选窗和拼音样式的行内预编辑都无法消费它。

## Decision

宿主快照在普通全拼组字时复用 `pinyin_segmentation_with_cases`；原始编辑串、光标位置和提交保持原样。专用英文与本地模式不分词。底层公开 `Session::snapshot` 继续提供参考回放使用的原始快照，宿主出口使用自己的显示契约；两者共用快照构造，不复制分词算法。双拼仍遵循既有显示偏好。

全拼完整性判断忽略尾部隔音符，显示出口保留该符号。否则已有纠错显示函数会把完整的 `nihao'` 重切为 `nih'ao`，并丢掉用户输入的尾部分隔。纠错的字母继续使用原文，例如 `sahnghao` 显示为 `sahng'hao`。

## Existing notes audit

[全拼显示分段按需拥有](../../implemented/testing/2026-10-10-pinyin-display-cow.md)持有显示分词构造与借用规则，本次直接复用；[双拼快照避免覆盖前编辑文本](../../implemented/testing/2026-10-10-shuangpin-snapshot-preedit.md)持有按需构造顺序，继续保持；[预编辑偏好与样式共用叶子类型](../../implemented/simplification/2026-10-15-preedit-preferences.md)持有界面样式范围，本次不变更其默认值；[macOS 行内组字显式声明预编辑样式](../../implemented/bug-fix/2026-10-19-macos-inline-preedit-caret.md)持有原生选区与标记样式，本次不修改。其余候选布局与皮肤笔记不持有分词决策。

## Alternatives considered

- **在 macOS 读取候选编码拼接分词。** 只改一个宿主，但候选可能只覆盖部分拼写，也可能是纠错、补全或异步来源；这会把分词规则复制到宿主，并让其他宿主保留同一缺陷。使用引擎已经构造的显示分词。
- **让底层公开快照一律改成分词显示。** 出口更少，但会改动参考引擎的原始快照契约及其 golden 回放；宿主显示出口承担显示格式，底层参考契约继续保留。

## Verification

新增宿主回归在旧实现因 `nihao` 不分词而失败。三项回归覆盖自动分词、显式隔音符、纠错字母与尾部隔音符、字面大写、光标前后缀、原始提交、专用英文、本地模式与双拼显示偏好。远端 macOS arm64 上 `cargo test -p msime-engine` 的 2521 项单测及 31 项 golden、`cargo test -p msime-input-runtime` 的 365 项全部通过；保留 56 项原有忽略。引擎全目标 Clippy（`-D warnings`）、Rust 格式检查及本机笔记检查通过。macOS 原生 `shortcut` 与 `candidate-row-layout` 两项通过；尚未安装输入法进行真实应用输入验收。

第三次完整 quick 成功退出，用时约 24 分钟；Linux 原生宿主的 111 个注册测试为 0 失败、1 项原有跳过，Rust workspace、Android、两种 WASM、Android Java、Linux 设置壳、macOS 原生和共享 Apple 桥的编译阶段通过。远端缺少 Node/pnpm、MinGW 和 HarmonyOS 打包依赖，对应阶段明确跳过；笔记检查在本机完成，不把这些环境跳过称为目标平台验收。

保留前两轮失败证据：首次 Python 3.9 环境失败与语音请求超时经既有 Python 3.14 逐项复核通过；修正 PATH 后的整轮运行完成所有编译阶段，但 Linux `linux-clipboard-tool-parity` 的两次子进程超过原有 3 秒时限。同一产物在新容器中单独复核通过。第三轮使用独立临时目录、`MSIME_CHECK_JOBS=2` 和仅作用于本轮容器的 CPU 相对权重 4096，没有改动断言或时限；剪贴板本轮通过，三项主题测试耗时降至约 43、20、92 秒。异常耗时的具体原因仍未确定。

iOS CI run `39943929288` 的真实键盘宿主测试发现三处旧显示断言：辅助码开启时仍期待 `shiY`，以及两个明确选择全拼方案的布局/键盘重入用例仍期待 `nihao`。实际快照分别为 `shi'Y` 和 `ni'hao`，与共享宿主的新分词契约一致；这三处期望现已同步，辅助码关闭后大写字面串 `shiY`、候选筛选、重入后「你好」候选以及布局尺寸断言仍保留。尚待新提交的 iOS CI 复核，不把这次失败当作通过。

Android 十八键双拼整合的完整设备回归还发现三处旧显示契约：放大编辑中间插删、设备信息复制准备和原生撤销恢复拼写的断言仍期待 `nihao`／`nhihao`。实际输入框已显示 `ni'hao`／`nhi'hao`，放大条仍是原始拼写。设备断言分别检查显示、原始串与光标；设备信息的禁止泄露断言同时检查原始及分节合成串。API 28、35 放大编辑专项已通过，完整 API 28 重跑在设备信息旧断言失败的原始证据保留；同步后 API 28、35 的完整复制专项均通过，包括菜单复制、剪贴板预览开关与后续输入；不将专项通过称为完整平台冒烟通过。第三轮完整 API 28 通过前述步骤及导入、数字页和导航后，在撤销的旧 `nihao` 断言失败；现场编辑器为 `ni'hao`，已同步该显示期望，保留真实提交、重做、删除、跨编辑器和密码隔离断言。同步后的 API 28 完整工具栏／撤销重做专项与其后全部 core 步骤以 0 退出；最终生产／测试输入的完整 quick 通过，连续完整设备冒烟仍不计通过。

#9675（Windows 0.3.0 报告候选窗仍显示 `pinyin`）：0.3.0 的 tag `windows-v0.3.0` 早于本改动，修复随本改动进入 develop。Windows 侧补的 `HostRawStyleText` 有可移植回归（`platforms/windows/tsf/tests/input/host_composition.cpp`），把它改回取 `preedit` 时该测试失败；Windows 候选窗拼音行的插入点换算 `preedit_display_caret`（`platforms/windows/src/candidate/PreeditDisplayCaret.h`）有可移植回归 `platforms/windows/tests/input/preedit_display_caret.cpp`，由 `scripts/test-windows-native-run.py` 运行；把它改回只在两串相同时给位置，18 项断言失败。`tests/input/preedit_caret.cpp` 经 `candidate_presentation_from_view` 覆盖同一换算，它要链接宿主库，只在 Windows CI 运行。TIP 的绘制代码和候选窗只能由 Windows CI 编译，没有在 Windows 真机上验收。

## Consequences

宿主共享同一份引擎分词显示；普通候选窗、展开候选窗和拼音样式行内预编辑通过既有 `preedit` 字段消费，无新增设置或协议字段。原始样式使用 `editing_text`，各平台默认值不变：macOS 和 Linux 的原始样式本来就画 `editing_text`；Windows TIP 的原始样式原先画自己宿主会话的 `preedit`，本次改动后全拼普通组字在那里也带上了隔音符，「原始按键」和「拼音分词」两种样式看不出差别（#9675），现由 `platforms/windows/tsf/HostComposition.h` 的 `HostRawStyleText` 在全拼普通组字时改画 `editing_text`，双拼（随「双拼预编辑」偏好）、韩文、注音、越南文、藏文、本地模式和专用英文仍画 `preedit`。宿主显示会增加隔音符，依赖预编辑长度的消费者继续使用独立原始编辑串与既有光标映射；Windows 候选窗预编辑行原先只在 `preedit` 与 `editing_text` 相同时画插入点（`candidate_presentation_from_view`），全拼分词后插入点消失；现由 `preedit_display_caret` 照 macOS `MSIMEPreeditCaretPosition` 的规则换算：两串去掉隔音符和空格后字母完全相同才换算，原始光标在音节边界时落在引擎插入的隔音符之前，紧跟用户自己打的隔音符时落在分隔符之后，两端保持在两端；展开的双拼、韩文、笔画、假名等预编辑不是原始字母，或光标超出 `editing_text`，仍不画插入点。按键回包那条路本来就不画插入点，不受影响；字面大写不做拼音分词。底层参考快照与宿主显示快照有明确区别，之后的快照改动需要同时覆盖这两条出口。
