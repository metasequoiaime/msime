# Agent Note: 鸿蒙 bundleName 改为 app.msime.hmos

Status: implemented

## Problem

在 AppGallery Connect 为鸿蒙版建应用时，包名 `app.msime.harmony` 被拒：「应用包名中包含敏感词或者保留字符 "harmony"」。没有 AGC 应用，就无法申请「输入法应用内数据共享」（见 [共享沙箱方案](../../proposed/architecture/2026-10-09-harmony-ime-shared-sandbox.md)），也无法生成签名 profile。

## Decision

bundleName 改为 `app.msime.hmos`。这个名字在 AGC 被接受，也不与其他平台的标识冲突：Android 是 `app.msime.android`（AGC 要求鸿蒙包名不能与 Android 相同），Tauri 设置应用是 `app.msime.client`，iOS、macOS、Linux、Windows 各有自己的。

改动的地方：
- `AppScope/app.json5`；
- 键盘扩展判断「自己是不是当前输入法」用的 `BUNDLE_NAME`；
- 两个测试文件；
- README 里的命令和说明，并在 README 里记下改名原因。

鸿蒙版还没有发布，没有已安装的用户需要迁移。开发机和模拟器上装过的旧包名要先卸载，再安装、启用新包名。

## Alternatives considered

- **改回最早的 `app.msime.client`。** 理由是它曾经就是鸿蒙的包名，README 里的验证记录也用它。不用它，是因为它现在是 Tauri 设置应用的 identifier（`apps/desktop/src-tauri/tauri.conf.json`），同一个名字指两个产品，日志和文档里容易混淆。

## Consequences

- **收益**：可以在 AGC 建应用、申请共享沙箱能力、生成签名 profile。
- **代价**：开发环境里装过的旧包要手动卸载，`ime -e`、`bm` 等命令要换成新包名；以前日志里的 `app.msime.harmony` 和新日志对不上，需要知道这次改名。
