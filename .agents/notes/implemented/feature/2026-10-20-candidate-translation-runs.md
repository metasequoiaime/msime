# Agent Note: 候选释义分段与 Linux 原生画布配色

Status: implemented

## Problem

固定参考 `msime-windows@dd018f79858314ed542a47c528608752b13bd261` 的 `server/src/ipc/candidate_translation_policy.h` 与 `ui/src/CandidateList.cpp`。共享清单已保留释义、词性和义项分隔符颜色，运行时却只输出一个释义字符串，Linux 画布又把它并入正文，因而不能绘制这些颜色。

## Decision

分段规则放在 `client-core::translation`，运行时在候选视图中附加可选的 `translation_runs`：`start`、`length` 为同一 `translation` 字符串的 UTF-8 字节范围，`role` 为 `pos` 或 `separator`，正文不额外标记。识别来源规定的十三种英文词性前缀、ASCII 与全角分号，保留全部文字与空白；没有分段时省略字段。不改变缓存、候选身份、上屏或翻译请求。

共享主题输出规范化的释义与四个分段槽位；用户选中底色的可读文字规则同样约束选中释义分段。Linux 保留原来的 classicui 文本，只让原生画布消费正文、释义、插件后缀及分段。Pango 在同一布局中处理字形与范围，释义用正文字号的 0.78 倍，竖排并列、横排放在正文下方；测量、选中背景、命中使用同一尺寸。透明的释义色按来源回退为正文色乘 0.62，分段透明色跟随本行释义，按下态使用选中配色。

### 旧笔记审计

- [Linux 外观](../../implemented/feature/2026-10-20-linux-canvas-appearance.md)：部分重叠，保留窗口、预编辑、选中条与菜单决定，本篇补充释义数据和绘制契约。
- [共享清单](../../implemented/feature/2026-10-19-windows-skin-schema-parity.md)：部分重叠，沿用已校验颜色与固定来源。
- 检索命中的词库查询页、桌面表达面板和会话存储提案：不涉及候选释义范围，保持原有决定。

## Alternatives considered

在 C++ 画布里识别词性最容易接到 Pango，但会让其他宿主再复制一份业务规则，Unicode 偏移也容易漂移。因此由共享 Rust 层发布与原字符串同源的范围，宿主只校验范围和绘制。

只给整行设置释义色可以复用现有绘制，但候选正文也会变色，不能消费词性与分隔符槽位，因此改用 Pango 分段属性。

## Testing

- 共享分段表覆盖全部词性、两类分号、连续分隔符、Unicode、空白及非词性文本；运行时和 Host API 的视图输出同源范围，候选身份与文字保持一致。run `20261020-074500` 的共享分段、主题、运行时释义及 Host API 回归通过；run `20261020-075110` 的运行时全套 326 项通过，四个相关 crate 的 all-targets clippy 在 `-D warnings` 下通过。
- 共享配色覆盖规范化、选中背景取色器与透明回退；原生像素测试覆盖普通、选中、按下、悬停、横竖排及非法 UTF-8 范围。run `20261020-074500` 的原生候选行与像素测试 2/2 通过。
- run `20261020-075110` 的六组真实 GTK 横竖排／2 倍缩放／合成器验收，加原生候选行和像素测试共 8/8 通过，覆盖本地 socket 异步释义、独立配色、热更新与点击仅上屏正文。小字号分号按前景与已知底色的抗锯齿混合线检查；截图已实际查看。
- 完整 quick 首轮发现源码契约仍按旧 `candidateRowText` 名称定位；契约改为 `candidateDisplay` 后仍检查后缀在释义之后并保留 `DontCommit`。复跑因构建机系统临时盘空间不足中断，使用既有 `target/` 承接 `TMPDIR` 后重跑；run `20261020-080405` 的完整 quick 通过：Linux 原生 93/93、Rust 工作区、Android target／Java、WASM、Linux 设置壳、macOS 原生与 Apple 桥接均通过。Windows 当前宿主策略 171 项通过、2 项按宿主差异排除、62 项需 Windows 构建；Windows 原生／MinGW／pipe 与 HarmonyOS HAP 因工具链或打包前置缺失跳过。macOS Tauri 工作区项因未暂存 bundle 跳过，Linux 设置壳独立检查；参考仓库比较、词库再生成与可选检查沿用脚本跳过范围，不计作设备验收。

## Consequences

共享元数据让各宿主能消费同一分段规则，代价是有分段释义的视图体积增加；空字段省略。Pango 的范围按 UTF-8 字节，越界或落在编码中间的范围一律忽略。长文本换行、工作区约束与逐行命中由[Linux 画布布局](2026-10-20-linux-canvas-layout.md)补充；此切片不代表材质、翻页、展开面板或 Wayland／IBus 自由画布完成。
