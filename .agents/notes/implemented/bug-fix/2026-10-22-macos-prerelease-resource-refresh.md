# Agent Note: macOS 预发布构建号与随附资源刷新

Status: implemented

## Problem

macOS 设置应用原来只比较输入法 bundle 的 `CFBundleShortVersionString` 和 `CFBundleVersion`。预发布工作流反复使用仓库里的 `build-number.txt`，因此同一营销版本的 beta、nightly 和重跑可能得到相同的构建号；如果随附的输入法资源发生变化，启动检查会把新 bundle 当成已安装副本而跳过复制，导致用户继续使用旧资源。

## Decision

发布工作流把全局工作流运行 ID 归一化到 macOS 构建号范围后传给 CMake，手动或 nightly 也可以通过可复用工作流的 `build_number` 输入明确指定。CMake 将构建号编码到 `CFBundleVersion` 的低九位十进制范围（1–999999999），保留版本号部分的跨版本排序，并拒绝超出范围的值。本地构建没有传入覆盖值时继续读取 `platforms/macos/build-number.txt`。

发布包还在输入法 `Info.plist` 中写入源码修订 `MSIMEInputSourceRevision`。设置应用比较到相同数值版本但修订不同、且新 bundle 带有该标记时刷新输入法；没有标记的旧开发构建不会因为缺少元数据而被反复替换。这样资源随源码变更时可以在不依赖构建号的情况下被识别，同时不改变“已安装副本更新时不降级”的规则。

## Alternatives considered

- **只让 nightly 手工递增 `build-number.txt`。** 最强理由是改动最少；但 nightly 是自动运行的，手工文件无法覆盖重跑和并发发布，也会把临时发布状态写入源码。
- **每次启动都计算整个 bundle 的哈希。** 最强理由是能直接反映所有文件内容；但输入法 bundle 含大型可执行文件和框架，启动时递归读取会增加延迟，签名框架的符号链接还需要定义稳定的遍历规则。发布时已有源码修订，足以标识随附资源的变更。
- **只比较 `CFBundleVersion`，扩大构建号后仍不记录资源修订。** 最强理由是保持安装器逻辑简单；但同一构建号的本地重打包或资源修复仍会被跳过，不能覆盖 issue 报告的刷新缺口。

## Consequences

发布构建的 `CFBundleVersion` 数值会比旧的三位构建编码更长；数值仍是整数，Sparkle 和设置应用的比较器继续按数值排序。运行号上限为 999999，超过时工作流会明确失败而不会产出可能无法比较的包。资源修订只在发布打包路径写入，开发构建仍可省略该键。

验证覆盖 CMake 的构建号校验、bundle 修订的刷新决策单测、发布工作流的输入传递和笔记门禁；macOS 原生打包需在有 Xcode 与签名环境的发布 runner 上完成。
