# Agent Note: 批量解析联系人读音，避免键盘聚焦阻塞

Status: proposed

## Problem

Android 导入电话本后，输入会话在主线程创建。宿主装载联系人名单时，对每个未填写拼音的纯汉字姓名分别调用 `hanzi_to_pinyin`；该函数每次打开词库，对未命中的姓名依次查询最多 26 张无 `value` 索引的词表。名单最多 1000 条。以发布包中的公开词库和合成未命中姓名测得 50 条耗时约 1.6 秒；同一词库按批次查询 1000 条的等价 SQL 耗时约 0.63 秒。清空名单后这段同步查询消失。[Android 官方 ANR 说明](https://developer.android.com/topic/performance/anrs/diagnose-and-fix-anrs)指出输入事件超过 5 秒不响应会出现 ANR，因此大名单有明确的键盘可用性风险。

## Proposal

在 Engine 的读音查询层增加批量入口，宿主装载名单时一次传入所有姓名。按姓名长度分组，一张词表用有界数量的 `IN` 参数查询一批姓名；500 个参数低于 [SQLite 较早版本默认的 999 个上限](https://www.sqlite.org/limits.html)。仍按原有词表顺序、表内权重和编码字典序选择整词读音，未找到的姓名再用已缓存的单字读音回退。无效姓名维持空读音。联系人磁盘格式与权限边界不变，设置页也无需更改。

## Existing note audit

- [Android 联系人候选](../../implemented/bug-fix/2026-10-21-android-contact-candidates.md)：保留其独立装载、候选顺序和刷新决定；本篇只缩短装载的读音查询。
- [Android 联系人导入](../../implemented/feature/2026-10-17-android-contacts-import.md)：保留显式权限和本机快照，导入页不增加读音计算。
- [名单手机号保留](../../implemented/bug-fix/2026-10-26-mentions-keep-phone.md)：存储字段及其他设置页的原样保存契约不变。
- [汉字回退缓冲](../../implemented/testing/2026-10-11-hanzi-fallback-lazy-storage.md)：保留先校验所有单字、再分配结果的规则；批量入口复用这一回退。

## Alternatives considered

- 导入时预先写入所有姓名读音：设置页可在工作线程预处理，键盘聚焦会很快；但老版本已保存的空拼音名单仍会在升级后的第一次聚焦阻塞，其他宿主写入的空拼音名单也没有统一处理。批量查询直接覆盖所有来源。
- 将输入会话创建移到 Android 工作线程：能避免主线程阻塞，但共享会话句柄绑定创建线程，当前按键和选词都在主线程使用，迁移会改动整个输入法线程契约。

## Acceptance criteria

- 合成词库中批量读音与现有单条读音逐项相同，覆盖整词、多音字、单字回退、重复姓名、无效姓名和未命中姓名。
- Android 导入联系人后，输入会话保持可用；大名单查询不随人数线性重复扫描整份词表。

## Risks

批量查询按长度和每批参数数目拆开，须核对旧版 SQLite 参数上限和排序规则；Android 模拟器需跑联系人导入与键盘设备用例。
