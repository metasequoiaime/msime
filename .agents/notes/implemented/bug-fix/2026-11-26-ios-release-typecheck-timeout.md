# Agent Note: iOS Release 构建避开一次类型检查超时

Status: implemented

## Problem

develop 上的 iOS Release 构建从 `66876da6ba`（#11842）起编不过，连续两轮 nightly 都停在同一个位置：

```
platforms/ios/App/Sources/keyboard/KeyboardKeyLayoutSettingsView.swift:327:12: error: the compiler is unable to type-check this expression in reasonable time
```

`twentySixKeyRows` 把整行内容写在一个 `enumerated().map { index, cells in … }` 里：闭包里同时有 `cells.map`、`caps.map`、两次 `caps.insert`、一个跨四行的 `sliver` 算式，返回值又是带标签的三元组。Debug 构建还能过，Release 的整模块优化下类型推导预算不够，Xcode 26.3 直接放弃。

合并时看不到：`ios CI` 编的是 Debug，只有 nightly 的 Release 构建（`release-ios.yml`，`DEVELOPER_DIR=/Applications/Xcode_26.3.app`）会撞上。2026-11-25 与 11-26 两轮 nightly 都红在这一行（#11912）。

## Decision

把 `twentySixKeyRows` 从「一个大 `map` 返回元组」改成「先声明行数组，再 `for` 循环逐行赋值」：`caps`、`sliver`、`inset` 都显式写类型，每行 append 一个带标签的元组。逐行赋值的推导负担小，`reserveCapacity` 保持容量，顺序和每个键的取值与原来完全一致。

代码里那条说明写明这是编译器的类型推导限制、对应 #11912，免得后来的人把循环改回 `map`。

## Alternatives considered

- **只给闭包加显式返回类型**（`map { index, cells -> (caps: [Cap], inset: CGFloat, unit: CGFloat?) in … }`）：改动最小，最强的理由是它通常就能让推导收敛。不采用：闭包里那个 `sliver` 算式和两次 `insert` 仍在同一个推导上下文里，26.3 的预算是否够只能靠一次 CI 试出来；逐行赋值是从结构上拆开，不赌预算数值。
- **拆成三个 `if index == …` 的分支函数**：每行一个函数，推导负担最小。不采用：三行共享同一段 `caps.map` 与前两行的缩进规则，拆开会重复那段逻辑。
- **提高编译器预算或改用别的 Xcode**：不改代码。不采用：nightly 固定 26.3 与 CI 一致，换工具链会让本地与线上不一致，且同样的推导在更大的表达式上还会复发。

## Consequences

- 26 键预览行的行为不变：键序、第三排两端的 44pt 与透明占位、第二排缩进都保持。
- 代价是同一段逻辑从表达式变成循环，`map` 的「每行独立返回值」变成「append 到数组」，可读性略降，用注释补回了原因。
- 这类失败只在 Release 露出，Debug CI 与本地开发都不会提前发现，只能靠 nightly 兜底。

## Verification

本机没有 26.3（只有 27.x），类型检查超时无法在本机复现，因此验收走 CI：在 PR 分支上手动触发 `Release iOS`（`publish: false`，`testflight: false`），它用与 nightly 相同的 `Xcode_26.3` 和 `build-app.sh … simulator`，编过即说明推导预算回到正常范围。Debug 侧的 iOS CI（编译、单测、UI 测试）同时覆盖这次改写没有改变行为。
