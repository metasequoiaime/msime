# Agent Note: Linux 可配置的辅助码前导键

Status: implemented

## Problem

#9177 的 Linux 设置只有辅助码方案，没有触发按键配置。全拼和双拼需要分别保存前导键，并在真实 IBus、Fcitx5 按键路径上生效。

## Decision

每族 `HelpcodePreferences.trigger_key` 保存空串或 `;`、`/`、`\`、反引号、`=` 中的一键。空串继续现有 Shift 加字母；配置前导键后仍兼容 Shift。设置向 Linux、HarmonyOS 与 macOS 暴露（鸿蒙的接线见[鸿蒙辅助码按键接通](../../implemented/bug-fix/2026-10-31-harmony-helpcode-entry.md)，macOS 见下文「macOS」一节）。共享偏好层校验可选值，旧文档缺字段时为空。

Engine 只在普通拼音组字、光标位于末尾且辅助码开启时接收前导键，接下来至多两个字母适配为既有大小写辅助码协议；过滤、排序、消费仍由既有算法拥有。全拼使用 `niFR`，双拼使用 `nifR` 表达顺序为 `fr` 的两位码：双拼的首码大写会交换双码顺序。首位单码保持大写，删掉次码后也恢复首码大写，保留优先匹配末字的既有单码语义。前导键不写入原文。未输入码时退格只退出前导状态；已输入码时退格回删码。取消、选择、提交、光标移动、切换方案、失焦和关闭辅助码都结束状态。微软双拼未完成的半音节优先接收分号韵母。

快照通过已有 `spelling_symbols` 把当下可接收的前导键交给宿主，复用 IBus、Fcitx5 的字符优先路由，不复制算法。网址、本地快捷模式、英文和其他方案维持现有键义。

## macOS

#10625 起 macOS 也打开 `helpcode_trigger_key`。macOS 宿主不需要新的按键代码：`trigger_key` 和其他辅助码偏好一样由 host-api 从共享偏好读进 `EngineOptions`（建会话和偏好热更新两条路径），宿主不另传；`InputController` 在 `=` 翻页、以词定字、智能标点、配对标点和小键盘标点之前都先查 `spelling_symbols`，列着的键落到末尾的 `msime_client_character`，runtime 再交给 Engine。当初只开 Linux 是因为 #9177 只要求 Linux，并且当时只在 IBus、Fcitx5 上做了按键验收，不是 macOS 有已知障碍。出厂默认值仍是空串（Shift 加字母），没有改动。

## Existing notes

[公共辅助码偏好](../../implemented/simplification/2026-10-16-helpcode-preferences.md)部分重叠，本次扩展模型但不改变归属；[大写拼音边界](../../implemented/bug-fix/2026-10-19-uppercase-literal-pinyin-boundaries.md)部分重叠，本次显式触发复用大写辅助码，保留字面大写规则；[Linux 随包码表](../../implemented/bug-fix/2026-10-18-linux-packaged-helpcodes.md)负责资源，不涉及按键配置。未找到拥有前导键决定的旧笔记。

## Alternatives considered

- 只提示 Shift 用法可以说明已有能力，但不能满足自定义触发键需求。
- 在 Linux 两宿主中记录前导状态，改动可局限 C++；但会复制输入状态和退出条件，无法由共享引擎回归守住边界。
- 允许任意按键和组合键可提供最大自由，但字母、分词符、数字候选键及 Ctrl 组合已有专属语义；本次限定五个普通标点，并保留微软分号韵母优先级。

## Verification

引擎回归使用 `fr` 这种不对称码，覆盖全拼、五种内置双拼方案、五个前导键、单码/双码与既有 Shift 输入等价、删码重输以及选词和退出边界。不能用两位相同的码验证顺序，否则会漏掉双拼倒序问题。

偏好测试实际保存并重新加载两族独立设置，校验旧配置缺字段和非法键。设置页测试穿过 Linux 能力位、表单修改和保存调用，检查另一族偏好未被覆盖。Host API 回归覆盖会话构造和切换方案时的独立配置。

Linux 原生专项通过 IBus 的隔离 D-Bus `ProcessKeyEvent` 和 Fcitx5 的 `keyEvent` 输入全拼 `ni/fr`、微软双拼 `ni;fr`，检查「拟」的候选、空格上屏、退格和分号韵母。IBus 同时检查候选窗口可见，防止隐藏前保留的旧候选造成假通过。这一层使用真实框架对象，不代表已在用户的桌面编辑器中验收。

变基后在远端构建机运行四 crate 严格 Clippy、引擎/偏好/输入运行时测试、67 项设置 UI 测试及前端构建；均通过。IBus 与 Fcitx5 的辅助码专项通过。完整 quick 中 Linux 设置壳、Android 交叉编译和 Java 宿主、macOS 原生及 Apple bridge 编译通过，但 Linux 全套门禁失败，不能报告 quick 通过。

失败分为三组：Host API 的货币标点和表达式符号集合两项断言在 `4b4fa9f7c9` 上单独复现相同失败；`ibus-chaizi` 在撤回全部本 PR 改动、重新构建共享库和原生测试后，仍报告关闭拆字时 Shift+C 被接收，恢复后辅助码专项通过；另七项 Fcitx5 daemon/在线请求测试因固定容器缺少 Python `dbus` 模块失败。没有通过增加豁免或修改这些断言把它们变绿。源码恢复后逐文件 SHA-256 核对一致。macOS CI 的三个快捷键偏好覆盖缺口已由后续 develop 的 #9228 修复，当前分支已同步该提交；上述基线与 quick 结果对应同步前的源码。

macOS 由 `shortcut` 套件守住：假会话用例在候选窗开着、`=` 翻页和智能标点都打开时，逐个按五个前导键，断言它们作为字符送进 Engine，不翻页、不走标点接口；Engine 没发布 `=` 时它照旧翻页。`shortcut-helpcode-trigger` 用真实会话，偏好里设好前导键后按真实键盘事件打 `ni`、前导键、`f`，断言前导键不上屏、组字仍是 `ni`，打出的码与 Shift+F 得到同一个组字，全拼和小鹤双拼各跑五个键；没设前导键时 `/` 照旧结束组字。这一层没有词库，候选筛选由上面的引擎回归覆盖；没有在真实编辑器里按过。

## Consequences

前导键会改变组字中的原标点功能；设置说明明确其作用范围，未组字时不吞键。配置缺省为空，现有 Shift 输入协议保持兼容；新增持久化字段由共享偏好层拥有，Linux 两个宿主不各自记录辅助码状态。当前只提供五个标点和至多两位码，任意组合键不在此契约内。
