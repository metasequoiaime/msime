# Agent Note: Fedora 45 RPM 支持基线

Status: implemented

## Problem

Fedora Linux 45 已发布，但 Linux RPM 的构建镜像、发布说明、下载指引、源码定义渲染和 Fedora 环境回归仍写死 Fedora 44。继续使用旧基线会让 Fedora 45 的编译器、Pango、Fcitx5 和运行库差异只能在发布后才暴露；用户也会从下载页得到过期的系统范围。

## Decision

RPM 构建和安装检查统一使用固定摘要的 Fedora 45 官方镜像。CI 的 Fedora 原生门禁、RPM 源码定义渲染和下载指引同步改为 Fedora 45；发行版文档把当前 RPM 构建基线写成 Fedora 45，并把 COPR、OBS 的目标 chroot 更新为 Fedora 44/45。Fedora 44 的历史故障笔记和历史复现记录保留原样，不把旧环境改写成新环境。

Fedora 45 的镜像索引摘要固定为 `sha256:a7d72ba2745b100a3351087472e5ee7b2450af7cf331e7673007e3817612839c`，避免 `fedora:45` 滚动标签改变构建输入。

## Alternatives considered

- 继续用 Fedora 44 构建，只在说明中宣称支持 Fedora 45：构建产物没有经过 Fedora 45 的依赖解析和原生 CTest，无法证明新发行版支持，否决。
- 同时保留 Fedora 44 和 Fedora 45 两套 RPM 构建：会产生两套不同链接基线和发布附件，当前发布工作流与下载页只有一套 RPM，且没有独立的版本选择契约；先以 Fedora 45 作为发布基线，Fedora 44 作为 COPR/OBS 兼容目标保留。
- 使用未固定的 `fedora:45` 标签：镜像内容会随上游重新发布而变化，不能作为可复现的发布输入，否决。

## Consequences

Fedora 45 的发行版差异会在 PR 的 Fedora 门禁和发布 RPM 检查中提前暴露，下载页和文档不再指向过期的 Fedora 44。Fedora 44 不再是发布 RPM 的构建基线；COPR、OBS 是否已在外部服务启用 Fedora 45 chroot 仍需外部项目设置完成，仓库内只能同步其声明和构建定义。
