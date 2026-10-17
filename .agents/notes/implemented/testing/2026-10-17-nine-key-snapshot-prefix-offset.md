# Agent Note: 九键快照复用锁定拼写前缀长度

Status: implemented

## Problem

`NineKeySession::snapshot` 先用 `locked.join("'")` 构造预编辑文本；选过左列首字母且仍有未锁定数字时，又重复拼接同一锁定前缀，只读取其字节长度来计算替换位置。

## Decision

保存第一次 `join` 结果的字节长度，并用它计算首字母在预编辑字符串中的字节偏移。锁定拼写、切分、首字母替换、快照字段和排序上下文均保持原有行为。

## Historical audit

已检索九键快照、预编辑缓冲、首字母显示和重复拼接相关笔记。九键读音缓冲和其他方案的快照优化有各自所有权契约；本片只消除同一快照函数内的重复前缀构造，不改变跨模块接口。

## Alternatives considered

- 继续重复 `join`：实现最直观，但首字母路径每次快照多一次字符串申请。
- 保存完整锁定前缀并复用：需要改变预编辑构造流程或额外复制，收益不超过保存长度；本片只保留已有字符串的长度。
- 重新扫描 `preedit` 找首字母位置：仍需扫描，且会把已知的锁定边界重新推导一遍；直接使用首个 `join` 的字节长度更准确。

## Verification

固定两段锁定拼写和首字母的快照分配从旧实现 5 次降为 4 次；新增测试门槛为不超过 4 次。快照相关 11 项测试通过；完整 engine/runtime 为引擎 2255、golden 31、runtime 318 通过（引擎 40 项既有忽略），Clippy（`-D warnings`）、Rustfmt、笔记检查和 `git diff --check` 通过。`verify-local.sh --quick` 的 Android host Java、WASM、Rust workspace 和仓库级检查通过；Windows 测试有 11 项宿主构建差异，Apple bridge 构建失败，quick 以失败结束。

## Consequences

首字母显示路径少一次短字符串申请和拼接扫描；没有首字母或没有未锁定数字的快照语义不变。收益局限于九键快照首字母分支，不能据此宣称九键整体 16 ms 目标完成。
