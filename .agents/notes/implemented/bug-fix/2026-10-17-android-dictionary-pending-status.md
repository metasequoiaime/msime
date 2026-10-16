# Agent Note: Android 词库显示待同步状态

Status: implemented

## Problem

Android 词库接口会在命名词库写入被键盘会话占用时返回 `pending` 条数。词库页和详情页此前只显示已落盘的 `entryCount`，用户看不到仍在队列中的词条，也无法判断导入是否仍在继续。#8333 报告了候选缺失，但当前证据不足以断定所有设备上的缺失都由同一条队列路径造成，因此这次先补足可见状态。

## Decision

`DictionaryCollectionsStore` 提供统一的 `pendingDescription(int)` 展示格式：零或负数不显示，正数显示分组后的待同步条数。Android 词库列表的副标题和命名词库详情的启用开关副标题都复用这个方法；详情页已有的说明继续解释键盘收起后会自动分批写入。内置词库没有命名集合和待同步字段，保持原有文案。

## Alternatives considered

- **只在详情页显示队列说明。** 详情页最容易放下完整解释，但列表页仍会让用户误以为导入已经完成，无法在返回列表后确认状态，因此不采用。
- **把待同步条数直接拼到各页面。** 改动表面更小，但会让逗号、分组和零值规则在多个页面漂移，因此集中到数据入口的纯展示方法。
- **把 `pending` 直接并入 `entryCount`。** 这样能显示一个总数，但会把已落盘数量和待写入数量混为一谈，且无法表达队列仍在等待键盘会话结束，因此不采用。

## Consequences

列表和详情现在都能明确显示待同步数量，用户可以判断词库导入仍在等待键盘空闲；数量使用与其他 Android 数量文案相同的千位分组规则。代价是新增一条稳定的中文展示契约，若后续产品文案改变需要同步纯函数测试和两个页面。该改动不改变队列刷新、键盘同步时机或引擎候选行为。

验证覆盖 `DictionaryCollectionsStoreSmoke` 的零值、单条和大数量格式，以及 `bash platforms/android/check-host.sh` 的 Android JVM smoke、宿主契约、资源检查和现有 policy smoke。
