# Agent Note: 集合待发项直达 Engine 有界批次

Status: implemented

## Problem

集合词条先写入个人词库队列，再由键盘同步到 Engine，完整导入会为同一词条重复写队列状态、生成请求和读取集合。个人队列的 128 项上限是正常编辑的契约，直接放大它会改变维护窗口和失败回退语义。

## Decision

Host API 在取得维护租约后，把集合待发项交给 client-services 提供的直达回调。服务层仍持有集合锁、回收旧个人队列的 `AlreadyPresent` 回执、更新 `preexisting.json` 和原子保存 `outbox.json`；Host API 负责把有界编辑转换为 Engine 批量事务。集合直达批次最多 4096 项，普通 `dictionary_edit_batch` 仍为 128 项。

集合请求 id 由 `collections-v2-` 加集合 id、操作、种类、编码、正文和权重的 SHA-256 派生，同一待发载荷重试保持不变，载荷变化产生新 id。Engine 逐项返回 `Written` 或 `AlreadyPresent`；成功项从待发队列移除，后者记入集合所有权回执。逐项失败沿用稳定 id 回退到旧个人队列；不属于集合的备份词和零预算调用继续走旧 128 项路径。

## Alternatives considered

- 把普通个人编辑上限从 128 放大：会改变手工编辑和键盘同步的事务边界，因此新增专用集合入口。
- 只把集合 outbox 改成更快的序列化：不能消除个人队列和 Engine 之间的重复请求与落盘，仍不足以达到吞吐目标。
- 让 services 直接依赖 Engine：违反服务层与引擎的架构边界；回调由 Host API 组装 Engine 类型。

## Verification

- client-services 集合测试 31 项通过，新增测试覆盖 172 项直达批次、稳定请求 id、单项失败回退个人队列和 outbox 清空。
- Host API 字典批次测试 16 项通过、3 项忽略；普通 128 项超限测试仍通过，零预算旧路径仍保留 128 项边界。
- 严格 Clippy（`msime-engine`、`msime-client-services`、`msime-host-api` 全 targets）通过。
- 将集合批次上限从 2048 提高到 4096 后，33555 词合成 Rime 集合在同一 macOS 调试构建、空主库和精确索引下，直达 scoped 路径 2 轮、总 3090 ms（导入 408 ms、同步 2681 ms、最长同步 2008 ms），约 10859 词/秒；该测量未隔离宿主负载，但达到 issue 的每秒万行目标。
- Android API 28（`emulator-5566`）已用 `build-apk.sh` 构建发布 APK，并通过 `smoke.sh ... --core`；其中 `DictionaryImportDeviceSmoke` 经安装的 JNI 导入并重载 33555 条 Rime 词条，完整核心套件通过。
- Android API 35（`emulator-5580`）同一 APK 通过默认完整 `smoke.sh`；其中同样通过 33555 条 Rime 词条导入与重载，系统输入、实时偏好和剪贴板生命周期验收均通过。

## Consequences

正常集合导入减少个人队列中间态和重复批次；失败项仍可在个人词库界面看到并重试。直达事务可能比普通 128 项事务持锁更久，预算仍在完整批次后检查；零预算保持旧路径以固定边界。outbox 尚未迁移为不可变页和确认位图，崩溃恢复、跨版本迁移及目标吞吐仍由[稳定集合意图提案](../../proposed/bug-fix/2026-10-19-durable-collection-delivery.md)覆盖。

## Related

- [个人词库导入按有界批次共用事务](2026-10-18-personal-dictionary-batches.md)：普通编辑上限和逐项保存点保持不变。
- [保留已有词条的导入回执](2026-10-20-preserved-import-receipts.md)：直达路径复用相同的 `AlreadyPresent` 所有权语义。
- [分离引擎与个人队列的导入吞吐测量](../testing/2026-10-19-dictionary-throughput-layers.md)：提供直达路径与队列路径的下界比较。
