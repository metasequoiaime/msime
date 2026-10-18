# Agent Note: macOS 卸载使用实时输入源状态

Status: implemented

## Problem

macOS 设置应用在卸载前用 `defaults export` 读取 `com.apple.inputsources` 与 `com.apple.HIToolbox` 的启用列表。用户刚在系统设置中移除水杉输入法后，偏好服务可能仍返回旧列表，卸载因此持续返回 `input_source_listed`，每次确认都重新打开系统设置，无法把输入法移入废纸篓。

## Decision

已安装的 macOS 输入法新增只读参数 `--input-source-enabled`。它在输入法自己的进程边界内调用 `TISCreateInputSourceList`，只报告本 bundle 顶层输入源的当前启用状态；设置应用卸载检查优先运行这个探针，只有输入法不存在、探针超时或输出无效时才回退到 `defaults` 列表。探针只读，不登记、启用或修改输入源。

卸载仍然保留 macOS 27 的系统边界：只要实时状态显示顶层输入源仍在，继续让用户在系统设置中移除；确认已移除后才移动 bundle 并停止输入法进程。孤立的输入模式条目不由顶层状态判定阻塞卸载。

## Alternatives considered

- **继续只读 `defaults export` 并增加等待或重试**：实现改动小，但等待不能保证 cfprefsd 与系统设置扩展的缓存同步，仍可能把旧条目误判为存在。
- **让设置应用直接链接 Carbon 调用 TIS**：可以去掉子进程，但会把 Carbon/CF 对象生命周期和系统框架依赖引入 Rust/Tauri 设置宿主；已有输入法 bundle 已拥有同一查询逻辑，新增只读命令能保持边界和测试方式一致。
- **按已启用模式列表是否为空推断状态**：无法覆盖只有顶层 `Keyboard Input Method` 条目的情况，会误放行卸载并留下系统设置条目。

## Consequences

设置应用每次卸载确认会短暂启动已安装输入法的只读探针，最多沿用现有 5 秒探针上限；旧版输入法不识别该参数时自动回退到偏好读取。输入法主进程增加一个不创建 `NSApplication`、不启动 IMK 服务的命令路径。新增的 C++ 命令解析测试和 Rust 输出解析测试覆盖了错误输入，macOS 真机仍需在系统设置中完成一次移除后确认卸载的端到端验收。
