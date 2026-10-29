# Agent Note: HarmonyOS「防止截屏」开关与面板隐私模式

Status: implemented

## Problem

#10098 的「防止截屏」在 #10159 里只做了 Android（见 [Android 防止截屏](2026-10-29-android-screen-capture-protection.md)），#10194 跟进 HarmonyOS。鸿蒙原来既没有开关、存储和认证，也没有在任何面板或窗口上设隐私模式，`module.json5` 也没有声明 `ohos.permission.PRIVACY_WINDOW`。设备离手时，别人拿起已解锁的手机可以截屏或录下设置应用的页面，或者在任意应用里弹出键盘、打开剪贴板面板后截屏。

## Decision

切换规则与 Android 逐条相同，写在纯 TS 的 `keyboard/settings/ScreenCaptureProtectionPolicy.ts`（`tests/screen-capture-protection.test.ts`）：值不变什么也不做；设备没有可用的系统认证（`userAuth.getAvailableStatus(PIN, ATL1)` 报错，即没设锁屏密码或被锁定）时一律拒绝，第一次打开也拒绝；从未打开过时的第一次打开直接保存；其余先认证。文件里开着却没有「打开过」标记时按打开过处理。

存储与 Android 的思路相同、形式不同：

- 开关写在设置应用的 `files/state/screen-capture-protection.json`（`{"enabled":…,"enabled_before":…}`），不在共享偏好里，不进 Rust 账号同步的导出映射。鸿蒙没有本地备份，`module.json5` 也没有开系统备份扩展，所以「不进备份」不需要额外处理。
- 键盘扩展从 API 12 起是独立沙箱，两个进程拿到的 `filesDir`、`preferencesDir` 路径字符串相同，背后是两个目录（见 `platforms/harmony/README.md`「输入法扩展的独立沙箱与共享沙箱镜像」），所以不能直接共用 `preferences`。开关作为 `SETTINGS_ENTRIES` 里的 `screen-capture-protection` 条目经共享沙箱单向镜像给键盘：基础访问模式下也导入，不在 `KEYBOARD_ENTRIES` 里，键盘改不回去。设置应用里清除数据后文件消失，镜像随之删掉键盘那一份，回到关闭。

认证在 `ScreenCaptureAuthentication.ets`：与打字统计相同的锁屏密码加可用的指纹、人脸，ATL1，挑战值用系统随机数；不带冷却，每次切换都弹；超时 115 秒按失败处理。与 `StatisticsAuthentication` 不同，没有锁屏密码时不放行。认证类单独写一份而没有和统计页的抽公共部分：统计页那份带冷却、没有锁屏密码时放行，`tests/run.sh` 还逐行钉着它的写法，合并会让两种相反的规则挤在一个类里。

设置页走 `startRequest` 的 `screen_capture_protection_settings`（`load` / `save`），原生 `SettingsBridge.screenCaptureProtectionSettings` 读当前值、按规则决定、需要时认证、写文件，写成后当场改设置窗口的隐私模式并立即 `publishShared` 给键盘。发布失败（镜像读写出错）只记日志、仍回答已保存：文件和窗口都已经改了，若照 `failed` 回答，页面会提示「没有保存」、把开关退回旧值，另一处开关也收不到变化；窗口退到后台时的 `publishShared` 会再发布一次。拒绝时回答错误码 `screen_capture_authentication_unavailable`、`screen_capture_authentication_failed` 或 `screen_capture_busy`，共享页据此提示并让开关保持原值。这个请求不联网，没有隐私政策授权时也照常处理。

窗口效果：

- 设置应用只有一个主窗口（没有子窗口；`CustomDialog` 画在同一个窗口里）。`EntryAbility.onWindowStageCreate` 在载入页面之前，开关开着时用 `getMainWindowSync().setWindowPrivacyMode(true)` 设好，第一帧就截不到；之后由设置页写入时经 `window.getLastWindow` 当场更新。
- 键盘三块面板：软键盘（2in1 上是候选窗）、浮动工具条、输入模式提示，各在 `createPanel` 之后、显示之前调 `Panel.setPrivacyMode`。每次 `inputStart`（含面板建好前就来的那一个），`KeyboardSession.attach` 导入镜像之后，`applyScreenCaptureProtection` 重读开关、给现有面板重设，所以在设置应用里切换后，下一次弹出键盘就生效。`setPrivacyMode` 是一次同步的窗口管理 IPC，正好在 `panel.show()` 之前，2in1 上每次焦点变化最多三次；所以 `ScreenCaptureProtection.protectPanel` 用以面板对象为键的 `WeakMap` 记下每块面板上次设成功的值，没变就不调用。面板重建后是新对象，照常设一次；设失败不记，下一次再试。读开关的文件只有几十个字节，每次照读。没有放进 `preferencesReloaded`：开关不是偏好，偏好修订号不随它变，那个钩子不会因为它触发。
- 设隐私模式失败（权限不在、窗口已销毁）只记日志，不中断面板创建或页面载入。
- `ohos.permission.PRIVACY_WINDOW` 在 SDK 里是 normal 级、system_grant，在 `module.json5` 里声明即授予，不弹授权框。

共享 React 设置页：宿主提供 `SettingsClient.screenCaptureProtection` 时，「我的 › 隐私」在匿名使用统计下面多一组「截屏与录屏 / 防止截屏」，鸿蒙「关于 › 许可与隐私」也画同一组（2in1 没有「我的 › 隐私」页，浮动工具条和输入模式提示又只在 2in1 上有，所以这里必须有一份）。只有 `apps/harmony` 提供这个客户端；Android 发布 APK 不含共享设置页，macOS、Linux、Windows 不提供，不会出现。手机上两处同时挂着时经窗口事件同步显示，宿主按文件里的真实状态回答，某一处停在旧值也不会切错。设置搜索只按行标题匹配，外层再挂一个 `data-settings-search-label="截屏与录屏"`，搜「防止截屏」「录屏」都能找到，「我的 › 隐私」只在打开时渲染，常驻的「关于」那一份让搜索在任何页面都找得到。

`tests/run.sh` 钉住接线：权限声明、三块面板各设一次、获得焦点后重设（两处 `applyScreenCaptureProtection`）、主窗口在载入前设好、设置页写入前过规则、写成后改窗口并发布、认证不可用时不放行。

## 与 Android 的不同

- 存储：Android 是 `AndroidLocalSettings` 的两项、`LocalBackupPolicy` 显式排除；鸿蒙是 `files/state` 里的一个 JSON 文件经共享沙箱镜像，没有备份可排除。
- 键盘生效依赖共享沙箱：签名 Profile 不带 data-group-id、或在模拟器上时，键盘读不到设置应用的文件，面板不设隐私模式（设置应用自己的窗口照样受保护）。正式签名的包带着这个 ID。
- 「重置所有设置」：Android 认证过的「重置所有设置」把它恢复为关闭；鸿蒙共享页只有「恢复默认设置」，它只恢复共享偏好文档、不经认证，不碰这个文件，所以不需要也不应该连带关闭。
- 设置界面：Android 是原生隐私页「本机」分类下的卡片；鸿蒙是共享 React 页的一个分组，同样的说明压成行描述。
- 弹窗：Android 要给每个对话框、底部面板、`PopupWindow` 单独设标志；鸿蒙设置应用没有子窗口，键盘里的悬停提示（`bindPopup`）不开子窗口，画在面板里，随面板一起受保护。

## Alternatives considered

- 按分诊建议用 `@ohos.data.preferences` 存（`screen-capture-protection` 文件，`enabled`、`enabled_before` 两个键）：最省事，与统计页认证开关同一种写法；但键盘扩展是独立沙箱，`preferencesDir` 路径相同、目录不同，键盘永远读到默认的关闭，而共享沙箱镜像只复制 `files/state`、`files` 下的文件，所以改为 `files/state` 里的 JSON 文件并登记镜像条目。
- 键盘直接读共享目录里设置应用发布的那份，不复制到自己的 `files/state`：少一次复制；但其余设置都是导入后从本地读，统一走镜像导入，删掉文件的语义（清除数据后回到关闭）也由镜像现成的 ABSENT 处理。
- 只在「我的 › 隐私」放开关：与分诊和 Android 的位置一致；但 2in1 没有这一页，而浮动工具条、输入模式提示只出现在 2in1 上，用户在那里就打不开这项保护，所以「关于 › 许可与隐私」也放一份，代价是手机上两处并存，用事件同步。
- 在 `preferencesReloaded` 里重设面板：分诊提到的位置之一；但它只在偏好修订号变化时触发，这个开关不在偏好里，切换后不会触发，所以放在每次 `inputStart` 导入镜像之后。

## Consequences

收益：打开后鸿蒙设置应用的页面和键盘的三块面板在系统截屏、录屏里应为黑色；切换受系统认证保护，云同步、键盘进程都改不了它。

代价：打开后用户自己也无法截屏反馈问题，需要先关掉（要认证）。键盘那一侧要等下一次编辑框获得焦点才跟上设置应用里的切换。没有共享沙箱的包（侧载调试签名、模拟器）里键盘面板不受保护。

验证：`platforms/harmony/tests/run.sh`（含新的 `screen-capture-protection.test.ts` 和接线检查）通过，删掉一处面板接线后接线检查失败；`hvigorw assembleHap` 的 ArkTS 编译通过（原生库和资源借用较早暂存的一份，新文件没有 ArkTS 警告）；`apps/desktop` 的 `privacy-page.test.tsx` 新增 6 项（手机上经宿主切换、被拒时保持原值并提示、2in1 在「关于」里能搜到「录屏」、没有这项能力的宿主不出现、手机上「隐私」和「关于」两处开关在任一处切换后一致、`screen_capture_busy` 时保持原值并提示重试）通过；去掉切换成功后的窗口事件，两处一致的那一项失败。没有在真机上验证截屏和录屏是否变黑、认证弹窗与取消后的原值、没有锁屏密码时的拒绝，也没有验证 `Panel.setPrivacyMode` 在浮动工具条、输入模式提示这类 STATUS_BAR 面板上的效果。
