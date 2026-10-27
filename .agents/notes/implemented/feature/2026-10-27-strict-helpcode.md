# Agent Note: 一码辅助码严格筛选

Status: implemented

## Problem

全拼和双拼的一码辅助码以前只把命中的候选前移，未命中的候选仍留在候选列表中。用户无法选择排除这些候选的严格模式；双码辅助码已有严格筛选，行为需要保持不变。

## Decision

新增独立的 `strict_filter` 偏好，默认关闭，并沿用旧配置的缺省值。宿主把该值传入会话和查询请求；全拼和双拼一码在开启时只保留至少一个汉字命中该辅助码的候选，关闭时继续使用原有重排逻辑。双码始终使用已有严格筛选逻辑。设置页和 Android 输入设置页都提供同名开关，并明确说明一码关闭时只重排、开启后排除不匹配项，而两码一直严格筛选。

双拼在普通候选、模糊音、纠错替代路径和句中约束候选合并之后统一筛选末尾一码，引导键触发的 `end_helpcodes` 也遵守该开关。原编码的声母不合法时，使用模糊音替代路径识别出的末尾一码，避免未匹配候选从补查分支回流。

该字段使用可选序列化并跳过 `false`，因此旧配置无需迁移，默认值也不会改变已有用户的候选排序。

## Alternatives considered

- **把一码行为直接改成严格筛选**：实现简单，但会改变所有现有用户的候选列表和选择顺序，违反保持默认值的要求，因此不采用。
- **复用双码开关或按辅助码长度隐式决定**：无法让用户独立选择一码行为，也无法在设置页准确表达状态，因此不采用。

## Consequences

用户可以分别在全拼和双拼中开启一码严格筛选；关闭时保持兼容，双码行为不变。新增偏好贯穿 client-core、host-api、Engine 和移动端设置界面，增加了一个跨模块配置字段。复用主仓既有的规定 `target/` 缓存完成 Rust 验证，避免在系统盘不足 50 GiB 时新建完整构建树。Android 模拟器上的设置入口与输入行为仍待设备验收。


## Verification

审查回归 `strict_single_helpcode_filters_direct_triggered_and_fuzzy_rows` 在修复前失败：开启双拼严格筛选后仍留下未匹配候选。修复后覆盖末尾小写／大写码、引导键的 `end_helpcodes`、非法声母的模糊音替代路径、句中约束与末尾码冲突以及同一引擎反复开关时的缓存结果，全部通过。大写码在匹配前按双拼既有规则转小写，原有首末字排序仍由词典维护。

`cargo test -p msime-engine --locked`：2673 项通过，57 项按配置忽略，golden 31 项通过；`cargo clippy -p msime-engine --locked --all-targets -- -D warnings` 通过。上述命令用 `--target-dir /Volumes/Workspace/ime/msime/target` 复用已有缓存。

`verify-local.sh --quick` 已运行到 Android Java、wasm 与仓库契约检查；之后因系统盘不足 50 GiB 停止会创建全新 Linux Docker 构建树的阶段，未宣称完整门禁通过。首次共享缓存检查拿到另一 worktree 的旧 `HelpcodePreferences` 元数据；不改源码内容，串行关闭编译缓存并强制本分支源的时间戳重算后，`cargo check --workspace --all-targets --exclude msime-desktop --locked` 通过。固定 NDK 环境下的 Android 目标 `cargo check -p msime-desktop --target aarch64-linux-android --lib --locked` 串行复验也通过；Linux 原生宿主与设置应用仍以 PR CI 为准。
