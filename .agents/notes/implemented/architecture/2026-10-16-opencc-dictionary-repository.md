# Agent Note: OpenCC 数据迁入词库仓

Status: implemented

## Problem

#7908 的 OpenCC 臺灣表连同已有简繁表留在 `client-base`，但词库源数据的维护仓是 msime-dictionary。仅在 msime 内另建数据 crate 仍然没有拆出词库的维护归属。跨仓迁移还必须保住离线输入、字节可复现、插件与拆字生成器，以及六个平台的许可证分发。

## Decision

七份 OpenCC 文本和上游 `LICENSE` 按原字节归 msime-dictionary 的 `sources/opencc/`；该仓的 `upstream.lock.json` 与 `NOTICE.md` 记录来源、生成地区词组表的方法和许可。msime 不增加数据 crate，也不保留词表副本。转换算法与 API 留在 `client-base`。

`resources/opencc.lock.json` 固定 msime-dictionary 完整提交、OpenCC 上游提交、八份输入的大小与 SHA-256。`client-base/build.rs` 调用 Python 3.10+ 的 `scripts/fetch_opencc.py` 在构建主机准备数据，`include_str!` 仍把验证过的七张表编入各宿主。下载来自固定提交的 raw URL，不跟随 main；先限长，再校验，最后原子替换。默认缓存在 `target/opencc/<commit>/`，每次使用检查大小和摘要。`MSIME_OPENCC_OFFLINE=1` 禁止下载；`MSIME_OPENCC_SOURCE_DIR` 可指定离线检出的 `sources/opencc/`，它也必须逐字节匹配。运行时没有新增文件访问或联网。

OpenCC 是构建期内嵌输入，不进入按需安装的数据库生命周期，所以这是 msime 通常只消费词库 Release 附件的一项明确例外，两个仓的说明同步登记，不新增发布附件。配套 PR 先合词库仓，再把下游锁更新到其合并后的提交，不让默认分支引用未合并的历史。数据已由 msime-dictionary #80 合入，本仓固定其实际合并提交 `232e6dad2fbe788ad8adeb24bfa801ee1ec02c84`。构建器的来源登记与该仓的记录同步，避免读取新 checkout 时拒绝 OpenCC 记录。

插件常用字与拆字表生成器复用同一个下载和校验入口，检查模式缺少缓存时按既有约定跳过，不自动联网。迁移时同时补齐 macOS、iOS、Android 和 HarmonyOS 原先遗漏的 OpenCC 许可证打包。许可证原文继续作为 `resources/licenses/OpenCC-Apache-2.0.txt` 随应用提交和打包，不让声明收集额外依赖网络；构建时核对它与锁定的上游许可证一致。`.gitattributes` 将该文件固定为 LF，避免 Windows 的 `core.autocrlf=true` 将许可证改成 CRLF，破坏原始字节校验并使宿主构建失败。词库构建器的上游名表同步登记 OpenCC，以便读取增加该记录的词库仓 checkout。

## Alternatives considered

- **msime 内增加独立数据 crate**：Cargo 能直接跟踪依赖，不需要首次下载，构建最简单；但词表仍在客户端仓，不符合词库迁出到 msime-dictionary 的要求，因此不采用。
- **运行时下载 OpenCC 表**：可以减小二进制、独立升级数据；但会给离线输入引入安装状态与失败恢复，改变现有转换契约，因此保留构建期内嵌。
- **增加词库 Release 附件**：能沿用数据库分发路径；但 OpenCC 是编译所需的原始文本，在首次发布之前下游无法构建验证，也会把源码迁移绑到新的发布产物契约。本次直接锁定源码提交，并明确记录这个构建输入的例外。
- **在 Rust build script 内另写下载器**：不依赖 Python；但会与现有 Python 生成器形成两套下载与校验实现。仓库已有 Python 构建工具，复用一个入口更容易保证输入相同，代价是直接 Cargo 构建也要求 Python。

## Consequences

词表在词库仓维护，msime 的 API、转换内容、静态初始化和分配预算不变。首次构建需要下载约 1.1 MB 文本；无网络时必须提前准备缓存或离线源目录。构建失败会报告缺少或摘要错误，不退回其他数据版本。Nix 用 `opencc-inputs.nix` 在固定输出下载阶段取文件，再通过离线源目录交给三个 Rust 包，Cargo 沙箱本身不联网。Python 是新增的直接 Cargo 构建前置；`MSIME_PYTHON` 可指定解释器路径。

更新锁或下载器必须触发所有受影响宿主的检查，macOS/iOS 的路径筛选和本地 quick 范围同步覆盖它们。构建主机的缓存校验不等于 Android 运行时文件读写，本改动没有新增设备端文件路径或权限操作。

## Related notes

检索现有 OpenCC、简繁、词库、插件与拆字笔记：[client crate 分层](2026-10-15-client-core-split.md)、[臺灣变体](../feature/2026-10-15-traditional-chinese-variants.md) 部分重叠，保留各自决定并更新数据归属；[拆字模式](../feature/2026-10-13-builtin-chaizi-mode.md)和[插件合集](2026-10-12-script-plugin-collection.md)保留生成语义，改用同一锁定输入。前缀扫描与分配优化、标点及候选渲染笔记与数据归属无关。本地未推送的数据 crate 实现已撤回，不作为已落地架构保留。

## Verification

七份词表和许可证与迁移前逐字节一致。基础层 42 项、核心层 386 项单测通过；宿主转换分配预算 2 项、来源校验 14 项、下载与离线校验 8 项通过，其中 Git 临时仓用 `core.autocrlf=true` 和 `core.eol=crlf` 重新检出许可证，验证原始字节不变；该回归用例在缺少 LF 属性时失败。使用新增上游记录的词库 checkout 离线构建 Hanja 成功（555 个音节、28,416 条读音），说明共享构建器接受这组记录。iOS 从固定发布清单准备生成工程的输入，XcodeGen 生成成功，23 项工程配置测试通过。静态缓存、离线源目录均通过同一份锁校验；拆字表离线不变量通过，缺少其余上游缓存的重新生成检查按原规则跳过。

本地 quick 门禁通过：完整契约检查、Rust workspace、Android 与 Wasm 编译、Android JVM、Windows x64 交叉编译与桌面外壳、macOS 原生宿主、共享 Apple 桥、Windows x64/i686 pipe-only 均通过；Windows 本机可运行套件 162 项通过。Linux 因缺少 Docker、HarmonyOS 因缺少完整构建环境而跳过；Nix 无本机构建环境，未实测沙箱构建。未进行设备安装或运行时验收。本次文件下载与校验发生在构建主机，没有改变 Android 应用端的文件权限或目录操作。
