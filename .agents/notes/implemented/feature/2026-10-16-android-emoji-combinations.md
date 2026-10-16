# Agent Note: Android 表情面板增加组合表情

Status: implemented

## Problem

#8193 要求在表情面板加入附件中的 165 个 Emoji、汉字及组合附加符组成的聊天短语。附件与 yuyansdk 固定提交 `c99f9b7c451b7b77d379dab7d55fdbfb93b04b12` 的 `EmojiconData.kt` 完全一致，目标是 `icon_emojibar_hot`，不是其颜文字、普通 Emoji 或微信长文。

检索了表情目录、符号目录和颜文字笔记。[符号与颜文字目录](../../implemented/feature/2026-10-08-android-symbol-catalog-and-emoji-candidates.md)仍复用 Engine 已有数据，本次组合短语不在该目录，属于部分重叠；[表情分类滑动 PR #8190](https://github.com/metasequoiaime/msime/pull/8190)仍持有分类切换手势，本次只增加分类和宽格显示。

## Decision

将原始列表单独放在 `shared/emoji-combinations/`，保留其 Unicode 序列和顺序；Android 表情面板追加「组合表情」，工作线程从随包资源读取并按原有 64 项游标分页，不改词库、JNI 或候选混排。普通 Emoji 继续按单字形过滤；组合表情本来就由多个字形组成，不用整串 `hasGlyph` 判可见。组合分类以及含组合的最近页采用宽格，并按实际文字宽度跨格，保持完整文字和点击上屏。

数据使用上游 BSD-3-Clause，保留固定提交、源文件 SHA-256、版权及全文许可；两条 Android 打包入口均带数据和通知，原生关于页可查看通知。只提取目标列表，不引入 Kotlin、图片、字体或微信文案。其他平台暂不增加入口。分类列表首项仍是普通笑脸，新增分类不改变默认入口。

## Alternatives considered

- **把列表塞入 Engine 的 SQLite 目录**：其他平台能直接沿用现有目录接口。但需要改变外部词库构建与发布，才能让当前 Android 用户看到新增短语；当前请求只补 Android 面板，数据作为共享文本资源保留复用入口，不新增输入算法或改词库锁。
- **仍用普通 Emoji 的八等分和整串字形过滤**：最大程度复用现有网格。但 `8️⃣🅱️Q了` 等是多个字形，整串 `hasGlyph` 会把它们全部删掉，固定窄格也会裁切；因此需要独立显示规则。

## Testing

`EmojiCombinationCatalogSmoke` 覆盖原始序列、165 项完整性和分页边界；`EmojiCombinationLayoutDeviceSmoke` 检查安装包的全部条目、许可通知及不同宽度的实际 Android 排版；`EmojiCombinationDeviceSmoke` 在 API 35 真实编辑器检查原样上屏、分页尾项、最近记录重启恢复及普通表情。API 28 仅做原生排版与资源专项，系统无障碍树不暴露面板，不将它记作编辑器交互验收。

本地 `check-host.sh` 的 237 项 JVM 冒烟和 134 个设备测试源编译通过；`assembleFullRelease` 构建通过，最终 APK 的数据与许可均和源码逐字节一致。API 35 最终包通过组合表情真实编辑器专项和既有 `EmojiPickerDeviceSmoke` 回归，覆盖原始 Unicode 上屏、分页尾项、最近记录重启恢复、普通表情插入与删除。

已完成的设备验证：API 28、35 上全部 165 项在 240/320/411 dp 及宽窄往返中均有完整文字宽度和点击动作；安装资产含完整 BSD 声明，关于页的通知列表包含该文件。解析使用最小 API 支持的逐行拆分和有界输入流，避免 `String.lines()` 与 `InputStream.readAllBytes()` 在 API 28 缺失。

本片只改 Java 与随包文本，设备 APK 重新编译并签名，复用 #8094 工作树已验收的生成原生库和 staged 词库，未复制源码或宣称重建 Rust。Tauri 开发合包仅接线并校验脚本语法，未构建合包；发布产品的原生宿主是验收对象。

## Consequences

旧 Android 字体可能缺少单个新 Emoji 或组合附加符，不能把多字形整体当成一个缺失字形过滤；不附带额外字体。宽格会减少同屏条数，最近页含组合时也使用宽格。完整 Unicode 序列必须保留，不能用正则清洗变体选择符、连接符或组合附加符。
