# Agent Note: 选择推进移除未使用分段副本

Status: implemented

## Problem

`SelectionTransition` 同时保存 `current_segmentation` 和 `current_segmentation_with_cases`。候选选择推进会按日文、五笔、双拼、全拼继续或完成分支给前者复制请求分段，但生产代码只把带大小写的字段交给造词进度生成预编辑，普通分段字段没有任何读取。每次选择仍为无效字段创建拥有型字符串。

## Decision

删除 `SelectionTransition.current_segmentation` 字段及五个分支中的赋值，只保留实际用于造词预编辑的 `current_segmentation_with_cases`。保留 `full_pure_pinyin`、规范读音、继续状态、五笔标记和带大小写分段的内容与所有权。候选选择结果和造词进度接口不变。

## Alternatives considered

- 保留字段但改成借用引用：字段仍无生产读取，增加生命周期和跨引擎借用约束，不能解决无效状态。
- 只把字段赋值延后到调用方：仍需确定性地生成一个永不读取的值，无法消除分配。
- 重构整个 `SelectionTransition` 为分支枚举：会扩大改动面；本片只移除已确认无读取的字段。

## Verification

在冻结的 `origin/develop` 基线，旧实现的全拼和双拼候选选择分配分别为 15 和 24。先将预算收紧为 14 和 23，旧代码真实红测失败；删除字段后 15 个 `selecting_a_*` 测试通过，两个预算变为 14 和 23。引用检索确认生产树中只剩 `current_segmentation_with_cases`；保留字段内容、容量和独立所有权的候选选择测试继续通过。

完整引擎、golden、Clippy、Rustfmt、笔记检查和 `scripts/verify-local.sh --quick` 在 `origin/develop` 的 `0c5559bcc` 组合基线上验证。光标插入分配断言原先为 207；临时恢复本片全部源码改动后，冻结基线 `a67c25279` 的同一测试仍实际为 206，因此将该已有预算漂移同步为 206，不计入本片收益。未声明发布构建、RSS 或真实宿主时延收益。

## Consequences

进入选择推进的路径移除一个未使用的分段字符串副本；全拼和双拼非空分段基准各少一次分配。绕过选择推进的候选及空分段不计入这一分配收益。日文、五笔、双拼、全拼继续与完成组字的实际预编辑字段、规范读音和剩余输入逻辑保持不变。
