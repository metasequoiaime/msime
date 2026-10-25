# Agent Note: CLI 完整会话与 MCP 用法

Status: implemented

## Problem

已有 `msime expand` 只能按编码查询本地词典，不能驱动一条完整的输入会话。需要给 demo、回归脚本和编辑器集成提供逐键输入、候选刷新、上屏结果和光标位置，同时让用户能查到 CLI 与 MCP 的实际用法。

## Decision

`msime session` 使用当前运行时选项在进程内创建一个完整的 Host API 会话，标准输入接受逐行 JSON 请求，标准输出逐行返回 transition JSON。会话复用 `host-api` 的 `LocalSession` 安全 Rust 包装，按键、编辑命令和候选选择都走现有 C ABI；返回的 `view` 负责提供候选、代次和光标位置。终端直接运行时自动授予读词库权限，管道或助手调用必须显式传入 `--allow-dictionary-read`。

MCP 仍由 `msime-mcp` 提供，CLI 的 `tools`、`call` 和 `prompt` 与 MCP 服务器共用工具实现。用户文档放在 `msime-docs/guides/cli-mcp.md`，README 增加入口。

## Alternatives considered

直接为 Windows、macOS 和 Linux 各自新增一套宿主 IPC，可以让 CLI 连接现存的输入法进程，但需要三套传输、鉴权和宿主生命周期协议，且会重复暴露已存在的 Host API 会话边界。本次先复用 Host API 会话，保持协议跨平台一致；后续若需要读取正在编辑器里的现存会话，再单独增加受保护的宿主 IPC，不把两种会话语义混在一起。

## Consequences

CLI 现在能以稳定的 JSONL 形式驱动完整本地会话，候选与光标来自真实运行时链路，且不改变现有 `expand` 的离线查询语义。`session` 会创建独立会话，不会读取编辑器当前已存在的组字状态；文档明确了这一点和权限边界。Host API 增加了供 Rust 工具复用的安全包装，避免 MCP 二进制直接使用裸指针。

验证：`cargo test -p msime-mcp-server --locked`（109 个测试全部通过），`cargo fmt --all` 和 `git diff --check` 通过。
