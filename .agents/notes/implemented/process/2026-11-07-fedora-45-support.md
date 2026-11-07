# Agent Note: Fedora 45 RPM 支持基线

Status: implemented

## Problem

Fedora Linux 45 已发布，但 Linux RPM 的构建镜像、发布说明、下载指引、源码定义渲染和 Fedora 环境回归仍写死 Fedora 44。继续使用旧基线会让 Fedora 45 的编译器、Pango、Fcitx5 和运行库差异只能在发布后才暴露；用户也会从下载页得到过期的系统范围。

## Decision

RPM 构建和安装检查统一使用固定摘要的 Fedora 45 官方镜像。CI 的 Fedora 原生门禁、RPM 源码定义渲染和下载指引同步改为 Fedora 45；OBS 发布配置加入 Fedora 45，发行版文档把 Fedora 45 写入 COPR/OBS 目标矩阵，同时如实记录两个外部项目当前仍只有 Fedora 43/44，Fedora 45 的外部 chroot 需另行启用。Fedora 44 的历史故障笔记和历史复现记录保留原样，不把旧环境改写成新环境。

Fedora 45 的镜像索引摘要固定为 `sha256:a7d72ba2745b100a3351087472e5ee7b2450af7cf331e7673007e3817612839c`，避免 `fedora:45` 滚动标签改变构建输入。

## Alternatives considered

- 继续用 Fedora 44 构建，只在说明中宣称支持 Fedora 45：构建产物没有经过 Fedora 45 的依赖解析和原生 CTest，无法证明新发行版支持，否决。
- 同时保留 Fedora 44 和 Fedora 45 两套 RPM 构建：会产生两套不同链接基线和发布附件，当前发布工作流与下载页只有一套 RPM，且没有独立的版本选择契约；先以 Fedora 45 作为发布基线，Fedora 44 作为 COPR/OBS 兼容目标保留。
- 使用未固定的 `fedora:45` 标签：镜像内容会随上游重新发布而变化，不能作为可复现的发布输入，否决。

## Consequences

Fedora 45 的发行版差异会在 PR 的 Fedora 门禁和发布 RPM 检查中提前暴露，下载页和文档不再把 Fedora 44 写成当前 RPM 构建基线。Fedora 44 不再是发布 RPM 的构建基线；仓库已经把 Fedora 45 纳入 OBS 目标矩阵，COPR/OBS 外部项目完成 chroot 设置后还需重新验证发布流程。
