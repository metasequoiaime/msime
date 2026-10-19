# Agent Note: 保留已有词条的导入回执可恢复

Status: implemented

## Problem

集合导入使用 `preserve_existing` 跳过用户词或已发布词时，原实现直接返回 `AlreadyPresent`，没有保存请求回执。进程在集合队列确认前退出、用户随后删除同名词、再用相同请求 id 重试时，集合会重新改写词条；相同 id 携带不同载荷也会被错误地当成成功。回执表写入失败还会被隐藏为成功。

## Decision

保留旧的 `Written` 回执载荷逐字不变，在相同载荷前增加唯一的 `already_present;` 前缀表示已确认保留。读取回执时，完整匹配旧载荷返回 `Written` 重试，匹配新前缀返回 `AlreadyPresent`，其他载荷仍返回 `RECEIPT_REUSED`。首次发现已有词条时，在同一保存点事务中写入带前缀回执；请求 id 为空时仍不写回执。保存失败沿用 `RECEIPT_NOT_SAVED` 并回滚该项，不能向集合层报告成功。

新前缀只由个人集合导入路径生成；捆绑词库使用自己的 `bundled;` 命名空间，旧调用者继续只接受 `Retry`。不增加数据库列或迁移，旧日志可以继续读取，新日志被旧版本拒绝复用而不会误报已写入。

## Alternatives considered

- 增加回执结果列或升级 schema：表达更直接，但需要跨平台迁移和旧版本读写兼容；前缀足以保持现有表结构。
- 继续把 `AlreadyPresent` 当作 `Written`：能少改代码，但用户删除词后重试会重新写入，且无法区分集合已确认与个人词已写入。
- 只在集合层保存完成状态：集合状态与引擎数据库回执可能跨进程失配，不能覆盖数据库提交后进程退出的窗口。

## Verification

- 修复前新增的三项回归均按预期失败：回执未保存、载荷变化被接受、回执触发器拒绝写入仍返回 `AlreadyPresent`。
- 修复后同三项回归通过，覆盖删除后重试保持跳过、载荷变化返回 `RECEIPT_REUSED`、SQLite 回执写入失败返回 `RECEIPT_NOT_SAVED` 且原词保留。
- `cargo test --locked -p msime-engine preserved_import_receipt -- --nocapture`：3 passed。
- 后续完整 engine/host-api 回归与 quick 门禁仍需在生产代码提交后执行；Android 原生设备验证尚未完成。

## Consequences

每个保留已有词条的请求现在多写一行回执，但重试不再依赖当前词库内容，且数据库提交与回执写入保持同一事务。新回执前缀要求旧版本不会误认新结果为 `Written`；回滚到旧版本时这些请求会报载荷冲突并保留待处理，优先于静默改写用户词。

## Related

- [个人词库导入按有界批次共用事务](2026-10-18-personal-dictionary-batches.md)：沿用批次保存点和逐项回执语义。
- [导入同步在有界作用域内复用集合状态](2026-10-19-collection-scoped-synchronization.md)：调用者仍在集合确认前依赖稳定回执。
- [稳定集合意图提案](../../proposed/bug-fix/2026-10-19-durable-collection-delivery.md)：后续专用待发存储仍须把此回执作为所有权先决条件。
