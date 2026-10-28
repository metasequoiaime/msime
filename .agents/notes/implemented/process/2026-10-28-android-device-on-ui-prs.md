# Agent Note: 改键盘或设置界面的 PR 在 API 35 上跑一遍设备验收

Status: implemented

## Problem

[PR 上只编译和测试](2026-10-09-no-packaging-on-pull-requests.md)把 `android-device` 挪到合并之后，代价写在那篇的 Consequences 里：设备上才看得见的问题要等合进 develop 才报出。到 2026-10-28，这个代价反复兑现，而且多数不是权限、SELinux 这类运行时问题，而是 PR 改了界面文字或交互，设备验收还在找旧的样子：

- 2026-10-26 皮肤删除验收失败一整天，由 #9926 修复；
- 2026-10-27 05:35–19:05「eighteen quanpin」失败，由 `b731116489` 修复；
- 2026-10-27 #9924 把「我的」皮肤卡读屏文字改成「皮肤 X，浅深均可选 · 外观固定，点击使用」，`SkinDeleteDeviceCheck` 还找「皮肤 X，点击使用」，develop 推送和 nightly 都停在「open native saved skins」（修复见 #10089）。

这些 PR 的作者在合并前完全看不到失败；合并后 develop 的 `Native Platform CI` 长期是红的，又让真正的新问题混在里面没人看（见 #10088 的 `2026-10-28-ci-red-tracking.md`）。

## Decision

- `ci-platforms.yml` 的 `Decide platform scope` 多输出一个 `android_device`：PR 改了 `platforms/android/java/`、`platforms/android/res/`、`platforms/android/AndroidManifest.xml`、`platforms/android/editions/`、`platforms/android/tests/device/` 或 `android-device.yml` 本身时为真。只改共享 Rust、`packages/ui` 等共享路径不点亮它，那些改动由合并后的 push 和 nightly 兜底。
- 新增 `android-device-pr` 作业，只在 `pull_request` 且 `android_device` 为真时调用 `android-device.yml`，矩阵只有 API 35 的整套验收。
- `android-device.yml` 新增 `matrix` 输入（JSON 数组），缺省仍是 API 28 的 `--core` 加 API 35 整套；`release-android.yml`、develop 推送和 nightly 不传，行为不变。
- 它不是必需检查，失败不挡合并。作者在合并前看到红灯，自己判断是验收要跟着改、还是改坏了产品；模拟器间歇失败也不会卡住无关 PR。

本篇部分取代 [PR 上只编译和测试](2026-10-09-no-packaging-on-pull-requests.md) 里「`android-device` 不在 PR 上跑」这一条；那篇其余决定（web 产物、macOS 打包挪到合并后）仍然成立。

## Alternatives considered

- **维持只在合并后跑**：PR 最省。没有采用，因为改界面文字打破验收这类问题在合并后才红，三天里重复了三次，每次都要另一个人回头定位、另开 PR 修。
- **PR 上跑完整的 API 28 + 35**：覆盖最全，最强的理由是 API 28 曾抓到 Android 9 启动崩溃。没有采用：两台模拟器各 40–60 分钟，而且 API 28 只跑 `--core`，界面验收本来就只在 API 35 上；API 28 的运行时问题仍由合并后的 push、nightly 和发版门禁兜住。
- **设为必需检查**：能强制修好再合。没有采用，因为设备套件还有未归因的偶发失败（见 #10089 的范围外列表），必需检查会让它卡住无关 PR；先不挡合并，等套件稳定后再考虑。
- **共享路径也点亮**：能抓住共享 Rust 改动在 Android 上的运行时问题。没有采用：几乎每个 PR 都碰共享 crate，等于把整套设备验收放回所有 PR，正是 2026-10-09 那篇要避免的成本。

## Consequences

- **收益**：改键盘或设置界面的 PR 在合并前就能看到哪条设备验收被打破，作者自己修，不再把 develop 和 nightly 打红。
- **代价**：这类 PR 多一个 GitHub 托管 runner 上约 40–60 分钟的作业（公开仓库不计费，不占 Studio）；模拟器偶发失败时 PR 上会多一个红叉，需要作者看一眼。
- **仍然没有覆盖**：只改共享 Rust 的 PR 在 Android 设备上的运行时问题，以及 API 28 上的问题，仍要等合并后。

## Verification

`actionlint`、`bash scripts/run-checks.sh`、`pnpm run verify-notes`、`git diff --check` 通过。本 PR 改了 `android-device.yml`，会在自己的 PR 上触发 `android-device-pr`，可直接看到 API 35 单台矩阵是否按预期展开、develop 推送是否仍是两台。
