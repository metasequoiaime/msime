# Agent Note: 本机快捷短语清空使用共享事务

Status: implemented

## Problem

快捷短语清空原先由 React 逐条调用 `edit`，失败后再由界面层逆序补写。每次编辑都是独立事务，批量操作中途失败会短暂或永久留下半个词库，回滚还可能再次遇到存储故障；界面层也会把内置快捷短语列表误当作可删除的用户行。

## Decision

本机清空通过 host-api 的 `clear_quick_phrases` 操作进入共享 Engine。Engine 在一次独占维护事务中读取 `user_dictionary_operations` 里 `dictionary='quick'` 且 `user_inserted=1` 的行，并复用单条删除的日志写入和输入上下文清理逻辑；任意一行失败时整个事务回滚，内置 `quick_parases` 行不参与删除。操作返回实际删除条数，React 只负责确认、调用和刷新，不再实现逐条删除或回滚。

`clear_quick_phrases` 与 `edit`、`import`、`reset` 一样需要设置页先让输入宿主释放词库。移动端个人词库队列没有独占维护入口，继续拒绝该操作；支持该入口的桌面 host-api 通过共享契约调用。

## Alternatives considered

- 保留 React 逐条删除并补强回滚：改动较小，但无法保证跨条目的原子性，也会把来源筛选和恢复责任留在界面层。
- 让 Engine 暴露一个不限数量的批量编辑列表：可复用现有编辑协议，但仍需界面先列出用户行，且容易重新引入内置行误删；专用按种类清空操作能在持锁事务内完成筛选。

## Consequences

- 本机清空只影响用户快捷短语，内置短语和其他词库保持不变；事务失败时词库保持原状。
- 新增了共享 host-api 操作和 C 契约文档，桌面 React 适配器可以直接显示实际删除数量。
- 移动端暂不提供该独占操作；移动设置页若接入时应复用同一语义，不能在队列层复制逐条回滚逻辑。
- 云端清空仍沿用[快捷短语本机与云端清空边界](../feature/2026-11-12-quick-phrase-clear.md)中的快照 CAS 规则，本笔记只约束本机实现。

## Verification

- `cargo test -p msime-engine user_dictionary::personal::tests::clear_quick_phrases`：事务成功、保留内置行和失败回滚用例通过。
- `cargo check -p msime-host-api --tests`：共享 host-api 及其测试目标编译通过。
- React 词库管理测试覆盖确认取消、成功返回删除条数和共享操作失败提示。
