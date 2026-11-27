# Agent Note: 修 Cargo audit 的两条漏洞与一个 yanked 版本

Status: implemented

## Problem

`.github/workflows/cargo-audit.yml` 的每日定时（UTC 15:00）从 11-22 起连续红，最近四次 scheduled 运行全失败（45069359413、45309605830、45512995175、45721899368）。它不是必需检查，也不挂在每个 PR 上，所以红着不影响合并，但定时任务的唯一作用——发现新公告——也同时失效了四天。

`develop`（`8d78e8853f`）的 Cargo.lock 在 11-26 的 audit 里报 **2 条漏洞、7 条 warning**（714 个依赖）：

| 类型 | crate | 版本 | 公告 | 链 |
| --- | --- | --- | --- | --- |
| 漏洞 | `url` | 2.5.8 | RUSTSEC-2026-0592，`Url::origin()` 对嵌套 `blob:` URL 栈溢出 | 多处 |
| 漏洞 | `cookie` | 0.18.2 | RUSTSEC-2026-0594，fork 后私有 cookie nonce 可重复 | Tauri / tauri-plugin-dialog |
| yanked | `async-executor` | 1.14.0 | 版本被撤回 | Tauri 侧 |
| unmaintained | `chrono` | 0.4.45 | RUSTSEC-2026-0543 | `msime-client-core` 直接依赖 |
| unmaintained | `zbus_names` / `zvariant` / `zvariant_derive` | 4.3.4 / 5.15.0 | RUSTSEC-2026-0371、0370、0372 | `zbus` -> `tauri-plugin-single-instance` -> `msime-desktop`（仅 Linux） |

失败的只有前两行：`cargo audit` 在只有 warning 的情况下仍然退 0（日志写 `warning: 7 allowed warnings found`），退出码 1 来自 `error: 2 vulnerabilities found!`。

## Decision

只改 Cargo.lock 的补丁级版本，不动任何源码：

```
cargo update -p url -p cookie -p async-executor
```

结果：`url` 2.5.8 -> 2.5.9、`form_urlencoded` 1.2.2 -> 1.2.3、`cookie` 0.18.2 -> 0.18.3、`async-executor` 1.14.0 -> 1.14.1，共 4 个包。锁的 diff 只有版本号和校验和，没有新增或删除依赖，也没有顺带重解析别的包。

**没有给 `chrono` 和 `zbus*` 加 `.cargo/audit.toml` 的 ignore 条目。** 它们是 warning，不挡任何东西；ignore 清单在仓库里的定位是「记下被跳过的风险」，而现在跳过它们对红/绿没有任何影响，加进去只是把三条债务记在账上又不解决。等真正要因此卡住合并、或者确实决定继续用 `chrono` 时再加。

## Alternatives considered

- **连同升级 `wasmi`。** 最强理由是它也被 yanked，能让 audit warning 变少。但 2.0.3 改变燃料计量，`msime-pack-tool` 的 6 个精确金样出现 1 到 8 单位差异，需单独更新金样和解释，不应混入本 PR。
- **给 `.cargo/audit.toml` 补 `chrono` 与 `zbus*` 三条 ignore，让 audit 输出变干净。** 最强理由是现有清单里已经有同类 unmaintained 条目（`unic-*`、`glib`），补上以后日志只剩真漏洞。但 `chrono` 是 `msime-client-core` 的第一方直接依赖，而清单头部的约定写着「全部经 Tauri 进来、第一方代码碰不到」，补一条就得同时改掉那句约定，等于用一次依赖升级顺手改写风险分类的口径。该做的事是把 `chrono` 迁到已经在树里的 `time`（`host-api` 已在用它的 `local-offset`），那是要改 `world_clock`、`clipboard_log` 与其他 crate 的行为改动，属于另一件事。
- **只升 `url`，`cookie` 留给 Tauri 自己升。** `cookie` 只经 Tauri 进来，但它是补丁级升级且不改变任何 API，等 Tauri 升级要跟整个 Tauri 版本走，没必要让一条已修的公告继续挂四天。
- **把 audit 的定时改回只跑在 `develop` 推送时。** 定时任务正是用来发现「没人推代码时新公布的公告」，砍掉它就永远不会知道。

## Consequences

- `develop` 的下一次每日 audit 应当回到绿（`url`、`cookie` 两条漏洞消失，`wasmi*`、`async-executor` 的 yanked warning 消失），只剩 `chrono` 与 `zbus*` 四条 unmaintained warning。
- `url`、`cookie` 与 `async-executor` 都是补丁级升级，调用方没有源码改动。
- 本 PR 改了 Cargo.lock，会因此触发 PR 上的 `Cargo audit` 作业（`paths` 包含 Cargo.lock），它的结果就是这个修复的直接验收。

## Testing

- `cargo metadata --locked`：锁与各个 manifest 一致。
- PR 上的 `Cargo audit` 作业：应当从 `2 vulnerabilities found` 变为只剩 warning。
