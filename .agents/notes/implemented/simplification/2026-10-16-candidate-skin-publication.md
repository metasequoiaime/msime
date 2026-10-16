# Agent Note: 候选窗口皮肤发布状态独立复用

Status: implemented

## Problem

候选窗口皮肤已有独立发布对话框，但本机目录、预检、自动补预览图、素材授权、发布草稿和提交状态仍由 JSX 函数持有。其他共享界面不能单独使用发布流程，验证异步生命周期必须渲染整个表单。

## Decision

`use-candidate-skin-publication.ts` 持有发布状态及动作，客户端只要求 `packPreview`、`addPreview`、`addLicense`、`publish`。目录读取、图片读取、打开目录和成功回调仍由调用方注入。对话框保留 DOM、文案、IME 事件和能力存在与否的显示条件，包入口补充导出 props 类型。

公开与私有预检、保留手工名称、分类变更重建重试 ID、授权不改变 ID、目录与预检 owner、补预览图和授权的独立操作代次、同步防重复及回调顺序保持。素材授权选项在状态模块导出，界面与默认值共用同一份列表；分类变更动作迁入 hook。

## 旧笔记归属

- [键盘皮肤发布](../../implemented/simplification/2026-10-16-community-skin-publication.md)部分重叠：沿用状态与对话框分离的边界；候选皮肤的打包、可见性、预览生成与许可证补写独立持有。
- [插件发布](../../implemented/bug-fix/2026-10-16-community-plugin-publication.md)部分重叠：沿用最小客户端能力和独立状态测试；候选预检原已包含客户端 owner，本次不改变代次含义。
- [发布回车](../../implemented/bug-fix/2026-10-14-community-publish-ime-submit.md)部分重叠：保留组合事件与普通回车的共享处理入口。
- [建议字段](../../implemented/simplification/2026-10-12-community-publication-suggestions.md)、[皮肤库读取](../../implemented/simplification/2026-10-12-community-skin-library-generation.md)和 Windows 原生社区页笔记无关：分别持有 Rust 默认字段、键盘皮肤库与原生宿主实现，不改变它们。检索未找到持有候选发布状态抽取的既有决定或过时提案，无需吸收或拒绝旧篇。

## Alternatives considered

- 继续只复用完整对话框：导入最简单，已有组件测试也充分，但无界面的共享入口仍不能复用发布流程，异步状态验证仍绑在控件上；增加独立 hook。
- 与键盘皮肤、插件统一成参数化发布状态机：可集中草稿和部分提交接线，但候选皮肤有自动生成预览、补许可证与可见性预检，通用状态机会扩展大量可选参数。真正共有的草稿、校验和异步工具已经共享，保留业务边界。
- 为测试注入第二个预览渲染器：能避开 Canvas，但已有 `renderSkinPreview` 是公共工具且现有测试在模块边界替代它；本次沿用该边界，不扩展产品接口。

## Verification

迁移前后 52 项基线测试通过。独立 hook 验证目录、预检、预览与许可证补写、重试、同步防重复、owner 替换和卸载；最小客户端的对话框测试验证空目录关闭和嵌入设置表单时根节点保持 `div`。既有四个代次源码门禁迁到实际持有者，未删除断言。

完整社区套件通过：81 文件、367 项。桌面类型检查与生产构建、Harmony 类型检查、改动文件格式/lint、notes、diff 检查通过。机械比对确认状态体仅授权常量更名和注释中文化，JSX 仅分类回调委托；独立审查无本次引入问题。

`bash scripts/verify-local.sh --quick` 通过，包括仓库契约、Rust 工作区、Android 目标与共享 Apple 桥接编译。Linux Docker、Windows MinGW 和 Harmony 原生打包环境缺失，相关阶段跳过，其他原生阶段按范围跳过。

## Consequences

发布流程可脱离完整对话框独立使用和测试；对话框显式接入状态及动作，接口接线更多，需要既有组件测试和机械比对覆盖。宿主仍负责实际磁盘读写，本次只迁移既有委托及代次守卫，不改变调用顺序、写入参数、权限和目录规则，也不声明设备验收。异步任务仍只丢弃过期回包的界面写入，不新增取消宿主操作的语义。
