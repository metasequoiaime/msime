# Agent Note: Nix 下助手配置里的 msime-mcp 用 profile 链接，store 路径条目按过期处理

Status: implemented

## Problem

设置页的 MCP 页把设置窗口旁边的 `msime-mcp` 写进 Claude Desktop、Cursor 的配置。Nix 装的设置窗口在 `/nix/store/<哈希>-msime-fcitx5-<版本>/bin` 下，写进去的是带版本哈希的路径：系统升级后助手仍跑旧版本，旧版本被垃圾回收后条目失效。用户手里已经有这样写进去的条目，路径是哪一版不确定，文件可能已经不在了，条目里还可能带着用户开过的 `--allow-write` 等权限参数。

## Decision

`crates/host-api/src/mcp_clients.rs` 在运行时决定写哪个路径、怎么认已有条目，Tauri 设置页与 Windows 的 FFI 入口共用：

- **写什么路径**（`stable_command_in`）：`server_command` 在 `/nix/store` 下时，取 PATH 上第一个自己不在 store 里、`canonicalize` 后就是这个文件的同名链接，即 Nix profile 里指向当前包的链接（经 `programs.msime` 装时是 `/run/current-system/sw/bin/msime-mcp`）。只认同一个文件：用户级 profile 在 PATH 上排在系统前面，里面可能另装了旧版本。找不到时写 store 路径。
- **怎么认已有条目**（`existing_entry`）：命令是 store 里同名程序的条目，不论哪个哈希、文件在不在，只要运行时选项和其余参数对得上，就算本程序写的，并标为 `stale`。`status` 把它报成已连接、`stale: true`，权限开关显示条目原有的权限；`install` 遇到它时把命令换成这次的、权限按调用方给的写，权限没变也写，结果是 `Updated`，不需要 `replace`。
- **设置页**（`packages/ui/src/settings/mcp-connect.tsx`）：`stale` 的助手页显示「更新」按钮和说明，点了按当前开关写入。

store 的判断都接收 `store` 参数，单测用临时目录或假路径模拟，不依赖跑测试的机器是不是 NixOS。

## Alternatives considered

- **不认 store 路径条目，让用户确认替换** — 这是本 PR 第一版的做法，规则最简单：store 路径一律当作别的条目，替换一次就换成 profile 链接。但替换按设置页记住的开关写（默认全开），条目原有的权限会被换掉，而用户在替换确认框里看不出来。
- **继续用 `same_program` 认出当前版本的 store 路径，在 `Updated` 分支里改写命令** — 评审提出，改动小、不动 UI。但 `same_program` 只认得出本次切换之后写进去的条目；升级后常见的是旧哈希的条目，它们照样被当作别的条目。认出来以后页面显示已连接、按钮是灰的，用户没有理由去点，迁移不会发生。
- **在 Nix 打包时指定写进配置的路径** — 不依赖运行时查找。但包不知道自己会被链进哪个 profile（系统、per-user、`nix profile`、home-manager），只能写死 `/run/current-system/sw/bin`。
- **PATH 上第一个解析后落进 store 的同名程序都可以** — 这是本 PR 第一版的选法，不要求是同一个文件。评审指出 `~/.nix-profile/bin`、`/etc/profiles/per-user/<用户>/bin` 排在系统 profile 前面，另装的旧版本会被选中，助手跑旧版本，设置页还显示已连接。

## Consequences

- **收益**：Nix 用户写进助手配置的命令随系统切换指向当前版本；已有的 store 路径条目点一次「更新」就迁过去，权限不变，也不弹替换确认。
- **代价与已知上限**：`McpClientStatus` 多了 `stale` 字段，TS 类型跟着改。命令在 store 里、文件名也是 `msime-mcp` 的条目一律算本程序写的，用户手写的同形条目也会被认作过期并提示更新。`/nix` 是指向别处的符号链接或用了自定义 `storeDir` 时，设置窗口的真实路径不以 `/nix/store` 开头，退回写 store 路径；这类用户多起来时重访。

## Verification

`cargo test -p msime-host-api --lib mcp_clients` 的 `a_server_in_the_nix_store_is_registered_through_the_profile_link` 与 `a_store_path_entry_is_stale_and_updated_to_the_profile_link`，在任何平台上都跑；`apps/desktop/tests/settings/mcp-connect.test.tsx` 的「an entry an earlier Nix version wrote is offered as an update that keeps its permissions」。
