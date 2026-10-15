# Agent Note: 换一对有备份的 Sparkle 密钥

Status: implemented

## Problem

[macOS 输入法本体经 Sparkle 自己更新](../feature/2026-10-15-macos-sparkle-input-method-updates.md)最初沿用了 MSIME-Apple 时期的密钥：公钥 `rSAufajnup+T+d+I4LTs4EAhe5M8bwHemWDKao3CB/E=` 随 macos-v0.48.6 的构建发出去过，私钥是仓库 secret `SPARKLE_ED_PRIVATE_KEY`（2026-09-04）。合并之后去找这把私钥做备份，找不到任何副本：

- GitHub secret 只能写不能读。
- 维护者 MacBook 与 Mac Studio 的登录钥匙串里都没有 Sparkle 条目，Sparkle 默认账户 `ed25519` 下也没有。
- 两台机器上 Claude Code 与 Codex 的会话记录里找不到生成它的那次操作：9 月初的记录已经清理掉了。
- 维护者确认自己手上也没有。

没有副本就意味着 secret 一旦丢失或被覆盖，所有已经装上的输入法再也收不到更新；而且没人能确认这个 secret 与那把公钥是一对，只能等第一次打包时才知道。

沿用它的唯一理由是 0.48 那批旧安装，而它们已经够不着：它们的 `SUFeedURL`（`metasequoiaime/MSIME-Apple/releases/latest/download/appcast.xml`，更早的是 `houko/MetasequoiaImeMac/...`）现在都 301 跳到 `metasequoiaime/msime/releases/latest/download/appcast.xml`。本仓库的 latest 发布在六个平台之间轮换，那里没有 macOS 的 appcast。

## Decision

换一对新密钥，并在启用之前备份：

- 用 Sparkle 2.9.6 的 `generate_keys --account msime-sparkle` 生成。新公钥 `nbQCVPlpQU41gXudWsKCjnVrB32GbdcukhMlEIlxujo=` 写进 `platforms/macos/sparkle-public-key.txt`。
- 私钥一份在维护者 MacBook 的登录钥匙串（账户 `msime-sparkle`），一份由维护者存进密码管理器，再写入仓库 secret `SPARKLE_ED_PRIVATE_KEY`，覆盖旧值。导出到磁盘的临时文件在写入 secret 之后删除。
- 写入之前用 OpenSSL 3 从导出的种子推出公钥，与 `generate_keys -p` 打印的一致；打包时 `package-release.sh` 还会再比一次。

这把密钥此后不再更换。已经装上的输入法只认 `sparkle-public-key.txt` 里的公钥，换掉它就等于让它们全部收不到更新，只能等用户手动装下一个 DMG。

## Alternatives considered

- **继续沿用旧 secret** — 最强的理由是它和 0.48 版发出去的公钥是同一对，将来要是能给那批安装送一份 appcast，用的必须是它。否定：那批安装的 feed 已经指向不会有 macOS appcast 的地址，要触达它们就得在本仓库每个平台的 latest 发布上都挂一份 macOS appcast，这个代价不值得为一批旧安装付；为它保留一把没有备份、也确认不了是否配对的私钥，风险压在今后的每一个版本上。
- **只在 GitHub secret 里放私钥，不另做备份** — 最强的理由是副本越少泄露面越小。否定：这正是旧密钥落到今天这一步的原因，secret 读不出来，丢了就无法补救。

## Consequences

- **收益**：签名私钥有维护者能取回的备份，与公钥是否配对在启用前就核对过；发版时 `package-release.sh` 的配对检查不再是一次赌博。
- **代价**：0.48 那批旧安装只认旧公钥，永远收不到 Sparkle 更新；它们原本就因 feed 地址失效收不到，所以实际上没有新增损失。本机钥匙串和密码管理器多了两份私钥，任何一处泄露都要再换一次密钥，而换密钥的代价就是上面那一条。

## Verification

- `generate_keys --account msime-sparkle -p` 打印的公钥与 `sparkle-public-key.txt` 相同，也与导出种子推出的公钥相同。
- `gh secret list` 中 `SPARKLE_ED_PRIVATE_KEY` 的更新时间是写入新值的时间。
- 第一次带 secret 的 macOS 打包通过 `package-release.sh` 的公钥核对。
