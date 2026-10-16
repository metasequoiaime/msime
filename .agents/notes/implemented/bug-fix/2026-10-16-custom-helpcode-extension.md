# Agent Note: 自定义辅助码目录与加载统一扩展名规则

Status: implemented

## Problem

设置目录使用不区分 ASCII 大小写的 `.txt` 扩展名发现规则，因此 `plain.TXT` 可以显示为 `custom/plain`。Engine 与 client-core 的路径查询却只拼接 `plain.txt`，在 Android/Linux 等区分大小写的文件系统上找不到表。目录中同一主干有多个扩展名大小写变体时，还会产生相同 schema 的重复选项，显示元数据与实际加载内容可能不一致。

## Decision

保留 `custom/<stem>` 标识和主干大小写，接受 `.txt` 的全部 ASCII 大小写变体；优先普通的精确小写 `.txt`，缺少时按扩展名字典序选择普通文件。八种扩展名有固定次序，路径查询最多尝试八个候选，不递归遍历目录。目录每个主干只显示一次，元数据来自同一个胜出文件。未知 schema、缺失表、损坏表、有界读取和链接拒绝沿用现有语义。Engine 不依赖 client-core 或 client-base，避免为文件名解析引入客户端业务及 OpenCC 依赖；两个已有路径入口由跨 crate 的真实加载回归核对一致性。

## Alternatives considered

- 目录只列小写 `.txt`：最贴近现有 Engine 拼路径规则，但收窄已经向用户提供的格式支持，复制自 Windows 的 `.TXT` 表仍然无法用。
- 把文件扩展名写进 schema：可以唯一标识每个文件，但改变持久化的 `custom/<stem>` 契约，已存偏好与不同文件系统之间的迁移都要额外处理。
- 新建公共文件名 crate：能共享少量解析逻辑，但为本次修复增加依赖图和发布单元；保留现有分层，用 host-api 接入测试约束两端更合适。

## Verification

Android API 28 的旧实现下，两项 client-core 回归失败：`.txT` 已列出但无法解析路径，`.txt` 与 `.TXT` 形成两个相同 schema；真实 Engine 的扩展名加载回归以 `Unknown helpcode schema` 失败。修复后，API 28 上 17 项 Engine 辅助码专项通过（两项依赖 checkout 资源绝对路径的整表测试留在主机运行）；主机完整引擎测试 2171 项、31 项 golden 通过，33 项按既有配置忽略。host-api 的 C ABI 目录发现到真实 Engine 加载回归通过。API 28、35 完整核心套件、主机 quick 门禁和原生 APK 构建已通过。API 35 的 `smoke.sh --core` 通过安装、词库准备、输入、100000 条辅助码导入和剪贴板保存生命周期。API 28 的完整 `smoke.sh --core` 在既有 `limited composition typing` 模拟器状态问题处失败，旧版同样复现该失败；API 35 全量 smoke 在既有 `ChineseLanguageReturnDeviceSmoke` 无响应处停止，未将这些结果归因于本改动。

## Consequences

扩展名支持与目录发现一致，`custom/<stem>` 持久化格式不变；主干不折叠大小写，非普通文件与不支持的扩展名不进入目录。普通小写表只需检查一次，其他变体最多检查八次；目录排序后每个主干只读一份显示元数据。路径入口保留在既有 crate 中，由真实接入回归约束选择规则，避免给纯引擎加入客户端业务依赖。目录元数据与 Engine 加载之间仍可能发生文件更新，本次不提供原子目录快照或热重载。

## 历史笔记检索

检索自定义辅助码、`custom_helpcode`、扩展名与目录读取。已有[目录句柄读取](2026-10-09-helpcode-read-directory-handle.md)部分重叠，继续保留其元数据打开边界并互链；[目录 hook](../simplification/2026-10-14-helpcode-catalog-hooks.md)与[移动端插件导入](../feature/2026-10-12-mobile-helpcode-table-import.md)分别持有 UI 生命周期和另一种插件存储格式，与本次文件名解析无关，不替代它们。Rust 官方 [`read_dir`](https://doc.rust-lang.org/std/fs/fn.read_dir.html)说明顺序不稳定，[`DirEntry::file_type`](https://doc.rust-lang.org/std/fs/struct.DirEntry.html#method.file_type)不跟随叶子链接；使用仓库已有的普通文件过滤与有界读取模式。
