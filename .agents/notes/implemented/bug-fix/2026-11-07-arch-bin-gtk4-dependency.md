# Agent Note: Arch 二进制包声明本地翻译的 GTK4 依赖

Status: implemented

## Problem

Arch 二进制包从 RPM 重打包时，`msime-linux-local-translation` 已经随包安装，但 `PKGBUILD` 只声明了 GTK3，没有声明该程序实际链接的 `gtk4`。Arch 构建容器的 `ldd` 检查因此报 `libgtk-4.so.1 => not found`，nightly 的 Arch job 在 `package()` 阶段失败。

## Decision

`msime-bin` 的运行时依赖加入 `gtk4`。本地翻译程序是发布包的一部分，依赖必须写在 Arch 包的 `depends` 中，让 pacman 在安装前准备对应的共享库；不把 GTK4 作为可选依赖，也不从包里移除本地翻译程序。

## Alternatives considered

- **把本地翻译程序从二进制包移除** — 最强的理由是可以避免新增运行时依赖。没有采用：这会让发布包缺少已经随 RPM 构建并在设置页提供的本地翻译能力。
- **把 `gtk4` 放进 `optdepends`** — 最强的理由是允许不使用本地翻译的用户少装一个库。没有采用：`msime-linux-local-translation` 是包内可执行文件，安装包的动态库闭包必须由硬依赖保证，不能让安装后程序直接缺库。

## Consequences

Arch 安装器会额外安装 GTK4 运行库，`msime-bin` 的本地翻译入口可以在干净系统中启动；不改变 RPM、Debian 或源码包的依赖。包体积和安装依赖略有增加，这是该功能可运行所需的代价。
