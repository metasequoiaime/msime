# Agent Note: Android 符号分类顺序自定义

Status: implemented

## Problem

Android 符号面板的分类顺序写死在 `SymbolPanelModel` 中。用户只能横向滚动到固定位置，无法把常用的「数学」或「网络」等分类移到更容易触达的位置；分类标题本身也不能作为持久化身份，因为后续可能调整显示文字或增加分类。

## Decision

符号分类使用稳定的小写 ID，显示标题继续由 `SymbolPanelModel.Category` 提供。`SymbolCategoryOrderPolicy` 以 CSV 保存 ID 顺序，限制文本长度，忽略未知和重复 ID，并把缺失分类按默认顺序追加。上下移动只修改内存中的不可变列表，设置页点击保存后才写入 `AndroidLocalSettings.SYMBOL_CATEGORY_ORDER`；该键只保存在本机，不进入账号同步。

`SymbolPanelModel.categories(recents, order)` 根据策略重排分类，未来新增且不在旧设置中的分类自动追加到末尾。`SymbolPanelView` 每次打开时读取输入法进程当前的本机设置，重建底部分类按钮；初始分类通过「常用」的稳定 ID 查找，所以移动「常用」也不会打开错误页面。设置页的保存、恢复默认和取消交互与工具栏排序相同，但使用独立的 `SymbolCategoryOrderSheet`；排序方式已由上下箭头改为按住把手拖动，见[排序面板拖动](2026-10-31-android-order-sheets-drag.md)。

## Alternatives considered

- **把显示标题直接作为设置值**：实现最简单，但标题改动会让已有设置失效，也无法安全处理本地化或同名分类，因此采用稳定 ID。
- **把顺序放进共享账号设置**：可以在设备间同步，但分类顺序与底部触达习惯强相关，且会扩大同步契约；本次保留为 Android 本机设置。
- **只在启动输入法时读取顺序**：能减少重建，但设置页保存后必须重启输入法才能看到结果；面板打开时读取当前快照的成本低，行为更直接。

## Consequences

用户可以在 Android「键盘 → 符号分类顺序」中调整全部分类，并在保存后下一次打开符号面板时生效。旧设置、未知 ID、重复 ID、超长值和未来新增分类都有确定的回退路径。代价是分类按钮顺序变化时需要重建底部按钮；目录数据仍按当前分类 ID 分页读取，未改变 Engine 协议。

验证包括 Android 主机门禁的 246 个 JVM smoke、150 个设备套件源码编译检查，以及 `compileFullDebugJavaWithJavac` 的完整 Android Java 编译；`SymbolCategoryOrderPolicySmoke` 覆盖 CSV 容错、边界移动、默认值、本机属性和模型重排。
