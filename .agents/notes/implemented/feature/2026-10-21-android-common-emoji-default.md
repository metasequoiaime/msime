# Agent Note: Android 表情面板默认打开空白常用分类

Status: implemented

## Problem

#8987 希望打开表情面板就能使用自己的常用表情，「常用」位于最左，水杉娘在它右侧；没有使用记录时保持空白。目前水杉娘优先，最近分类没有记录时还会消失，用户每次都要重新切换。

## Decision

Android 的最近分类显示为「常用」，始终保留在最左，打开面板固定选中它。水杉娘清单可用时排第二，其他 Emoji 分类顺序不变；横划与分类栏共用同一份顺序，首尾不回绕。常用只读取原有本机使用记录，不预填、不清掉已有记录；空时显示使用提示，选过表情后再次打开即可看到。

## Historical audit

- [手机内置水杉娘](../../implemented/feature/2026-10-20-mobile-mascot-stickers.md)：部分取代 Android 的首项与默认页决定，保留其他平台、素材和发送方式，双方互链。
- [Android 表情横划](../../implemented/feature/2026-10-16-android-emoji-category-swipe.md)：部分取代最近分类仅在有记录时参与横划的决定，保留手势取消、分页及边界规则，双方互链。
- [组合表情](../../implemented/feature/2026-10-16-android-emoji-combinations.md)：保留 Unicode、分页与最近项记录，仅同步设备用例的默认入口。
- 桌面符号目录、共享面板激活、最近复制、符号历史及词库导出笔记不拥有表情默认分类，不修改。

## Alternatives considered

- **保持水杉娘默认，只交换按钮位置**：保留此前突出内置贴纸的决定，但仍要求用户每次切换，不符合本 issue 的明确要求。
- **预填常用表情**：首次打开立即有内容，但会把未用过的条目当作常用，违背默认空白的要求。

## Verification

旧宿主在 API 35 的新增首项与默认页检查失败（远端 `20261021-005018`），未以现有默认值充当预期。修复后的原生 Java 宿主 `assembleFullRelease` 通过；API 35 的 `EmojiPickerDeviceSmoke` 已通过空历史、首项与次项、默认页、贴纸网格、横划边界与取消、分页、上屏、删除及常用重启恢复（`20261021-005123`）。夹具只在专用模拟器备份、清空并恢复表情历史，输入数据为合成值。同次运行的 API 35 `EmojiCombinationDeviceSmoke` 通过 Unicode 原样上屏、分页尾项和常用重启恢复；API 28、35 的 `EmojiCombinationLayoutDeviceSmoke` 均通过 165 条组合及 240/320/411dp 布局、可点击和许可证检查。API 28 只计原生布局验收，不计编辑器交互。

设备包使用本分支 Java 原生宿主、开发签名与固定官方 `android-v0.5.0` 库及资源（官方 APK SHA-256：`924d1727b9ecd1099c7a96d1acaadec4a9cfc95c8a54d2f075e21c2a0a3238a1`）；不代表当前 Rust 整包、发布签名或报告者真机验收。远端 `20261021-005503` 的 `pnpm run verify-notes` 与 `bash scripts/verify-local.sh --quick` 通过，包含 `StickerPackModelSmoke` 的有无贴纸、空目录、顺序与横划边界检查，其他平台按改动范围跳过；本地 `git diff --check` 通过。

## Consequences

打开面板即可访问自己的常用表情，不必每次切换；代价是习惯直接打开水杉娘的用户需要多点一次。最近记录的存储键与格式不变，旧记录应继续显示；贴纸异步加载只增加分类，不应抢走当前常用页。
