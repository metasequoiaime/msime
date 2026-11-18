# Agent Note: Windows 延迟按键分类保留 Shift 数字符号语义

Status: implemented

## Problem

#11576 报告 Windows 小鹤双拼组字时输入 `ceui#` 或 `ceui$` 导致候选窗口反复闪烁。Server 的 `digit_selects_candidate` 在普通模式仅让无修饰数字键选词，正常候选分类也先处理标点；但 `_ClassifyDeferredKeyDown` 的普通数字分支只看物理键码，把 Shift 产生的符号也归为 `FUNCTION_SELECT_BY_NUMBER`。回放因此进入选词处理器，而 Server 实际继续输入字面符号，两端对同一个键的意图不一致。

## Decision

延迟分类保留 Unicode、算式和网址模式的先行规则；普通数字分支仅在修饰键为空时选择候选。Shift 数字符号继续进入原有标点分类，与正常候选路径一致，宿主会话和 Server 沿共享引擎的既有规则接收字面符号。不修改输入模式、偏好或出厂默认值，不扩展符号表，不调整等待预算。

## Historical audit

- [拼音特殊符号组字](2026-10-22-pinyin-literal-symbol-composition.md)部分重叠：该决定已经让共享引擎接收 `#$%&`；本次修正 Windows 排队分类，不改变符号是否结束组字的产品约定。
- [数字选词软超时退路](2026-11-05-windows-digit-reply-miss-local-slot.md)部分重叠：该退路仅适用于真正的选词请求；本次让 Shift 符号不再误入选词处理器，不通过增加预算掩盖分类分歧。

## Verification

`test-windows-deferred-classification.py` 编译并执行实际延迟分类、入队和排空方法，先在修复前得到 32 条分类失败：全拼/双拼、即时分类/屏障回放和数字行八个 Shift 符号的组合全部误入数字选词；`@` 继续优先进入邮箱，不受影响。修复后检查符号进入标点处理器（`@` 保留邮箱入口），原字符与 Shift 修饰键完整保留；另覆盖裸数字、Unicode 的 Shift 数字以及算式/网址的拼写与选词边界。未进行设备、模拟器或真实编辑器操作，不把分类测试当作报告者 Windows 环境的窗口表现复验。

远端 macOS 验证 `20261118-011304` 退出 0：分类回归 475 项通过，Windows 本机可移植测试 208 项通过，笔记三项校验、Rust 工作区、共享 Apple bridge 及 x86_64/i686 pipe-only 编译通过，quick 门禁通过。完整 Windows 构建和 x86 交叉语法检查缺少工具链/既有配置而跳过，其他平台按本次 Windows 范围跳过，不记为通过。

## Alternatives considered

- **在数字选词处理器里识别符号后回退**：能够让错误分类继续工作，但回放投影、异步回复类型和 Server 请求意图仍不一致；正确做法是在分类时保留按键本意。
- **将所有 Shift 数字改为输入**：对 `#/$` 足够直接，但会破坏 Unicode 的 Shift 选词及算式/网址的既有槽位规则；因此只限制普通模式分支。

## Consequences

普通 Shift 数字符号不会再被排队路径误当选词，裸数字和本地模式保持原行为。实际输入、候选刷新和系统编辑器表现仍需在目标环境复验；本次以明确的分类分歧和先失败后通过的行为回归作为源码修复证据。
