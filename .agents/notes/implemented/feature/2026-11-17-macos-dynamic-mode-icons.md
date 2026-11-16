# Agent Note: macOS 可选模式字形的配置与动态字典

Status: implemented

## Problem

#11521 希望在品牌标志与中、双、五、日、英等模式字形之间自行选择，以辨识菜单栏输入源和系统切换条。品牌标志是维护者在 [11 月 4 日的决定](2026-11-04-macos-input-menu-logo.md) 中明确选择的默认值，不能为了新增选择而翻转。独立状态项占另一个槽位，也不能替代系统输入源图标。

## Decision

本切片实现共享偏好 `input_mode_glyph_icons` 和 macOS 的公开 IMK `modes:` 动态字典接线。新装和缺字段均为 `false`，沿用品牌图标；明确存成 JSON `true` 才替换模式字典里的菜单和 palette 图标。只消费共享 Rust 已加载的文档，不扩展冻结的 AppKit 设置，也不增加 React 控件。该项仅用于本机 macOS，不加入设置同步白名单。

共享设置字段对齐检查的 `RUST_ONLY` 登记本字段并说明原生入口缺口，与迁移期其他原生设置采用同一规则；不增加一个无法交付的 React 控件来通过检查，不删除既有字段或断言。

`InputModeIcons.h` 负责纯字典转换，巨型控制器只读取最新共享快照并接线。`modes:` 查询时读取配置，不进入按键路径，不借机应用会话状态。以 superclass 的真实返回值为基础，仅在它含有已知 `tsInputModeListKey` 形状时替换图标；`nil` 或未知形状沿用原值，不凭猜测注入静态字典。图标名取现有固定字形表，且仅对本版本标识符的已知模式生效，资源不存在时保留原引用。名称、语言、排序、未知字段、静态 `Info.plist` 和独立状态项保持原行为。单方案版本复用 `.Hans` 注册主模式，字形按版本默认方案选择，英文仍是「英」；未知版本方案保留原图标。

[Apple 的 `modes:` 文档](https://developer.apple.com/documentation/inputmethodkit/imkstatesetting/modes(_:)) 只明确描述动态模式字典用于构建输入菜单；[公开的 `IMKModeDictionary`](https://developer.apple.com/documentation/inputmethodkit/imkmodedictionary) 是从 bundle 读取该字典的键。不能由此推导 TIS 注册缓存、菜单栏图标和 Ctrl+空格切换条都会读取或刷新这一字典。本切片不声称这些系统消费路径已经打通，不修改签名 bundle、不调用私有图标接口、不结束系统服务。

远端原生探针实际读出的 `IMKModeDictionary` 值是 `InputMethodServerModeDictionary`，与本仓库静态注册使用的 `ComponentInputModeDict` 不同。曾假设两者相同的行为测试真实失败，因此不能直接用该常量读取本仓库的静态模式，也不能将静态字典强塞进未知的动态返回形状。新测试使用实际 plist 的 `ComponentInputModeDict` 验证转换能力；产品侧是否收到这一形状仍是明确缺口。

## 历史决策审计

- [品牌图标与方案名](2026-11-04-macos-input-menu-logo.md)：部分重叠，保留其默认值与静态打包决定并互链；本切片只新增明确选择后的动态字典。
- [独立模式状态项](2026-10-12-macos-input-mode-status-item.md) 与 [带框字形修复](../../implemented/bug-fix/2026-11-02-macos-status-item-mode-glyph.md)：部分重叠，状态项职责、字形与默认值均保留。
- 其他检索命中的 Windows/Linux 模式标记、服务图标与词条数量笔记无关，不修改。

## Alternatives considered

- **直接把静态 plist 改回字形**：系统注册与切换条本来就读取它，覆盖面最明确。但会改变所有用户默认值，不能满足保留品牌默认并让用户选择的要求。
- **用户选择后改写已安装的 bundle**：可能让静态读取方在重新登记后看见选择。但破坏签名、影响更新并把本机偏好写进产品产物，不采用。
- **复用独立状态项**：能力现成且按模式切换。但多占菜单栏槽位，不能覆盖系统切换条，无法作为本 issue 的替代完成项。
- **在现有 React 或冻结的 AppKit 回退设置里加开关**：可以更快提供可点击入口。但违反原生迁移约定，入口随正式 macOS 原生设置迁移提供。

## Consequences

共享配置具备严格布尔类型、持久化和保留未知字段的语义，动态模式字典具有可独立测试的转换边界。品牌默认值、模式名称和状态项不变。

尚缺正式原生设置入口、实际 IMK 动态返回形状的接通证据，以及对系统注册图标和 Ctrl+空格切换条的有效配置路径。PR 使用 `Refs #11521` 并保持 Draft。这是缺少实现和系统接口证据，不把未操作设备当作缺实现；本任务按用户要求不安装、不启动产品、不操作设备或模拟器。

## Verification

- Rust 行为测试覆盖新装/缺字段品牌默认、严格拒绝非布尔值、显式选择的保存与重新加载、其他设置及未知字段保留。
- `input-mode-icons` 使用实际 plist 模板与资源，覆盖全部 11 个模式、10 个版本主模式、英文、顺序和扩展字段保留、原字典不变、未知模式/无效形状/缺资源回退。
- 本切片的测试与门禁执行结果在 PR 验证段记录；资源效果图只展示实际 TIFF，不作为系统刷新证据。
