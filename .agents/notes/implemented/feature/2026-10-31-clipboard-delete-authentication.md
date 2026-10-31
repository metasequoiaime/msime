# Agent Note: 剪贴板删除数据验证（Android、iOS、HarmonyOS）

Status: implemented

## Problem

#10408：拿到已解锁手机的人可以在键盘剪贴板面板里直接删掉本机剪贴板历史。[剪贴板操作认证](2026-10-25-android-clipboard-operations-auth.md)只给 Android 的搜索、导出、清空加了系统认证，单条删除（长按操作行的「删除」、左滑垃圾桶）和多选删除都不认证，删除后只有 5 到 9 秒的撤销（[单条删除撤销](2026-10-29-android-clipboard-single-delete-undo.md)）。iOS 和 HarmonyOS 的键盘里删除和清空都没有任何认证。

报告者在 issue 评论里回答维护者的提问后，规则定为：「键盘 → 工具栏 → 剪贴板」里加开关「剪贴板删除数据验证」，默认关闭；打开后删除本机记录先过系统认证，多选整批认证一次，撤销不认证；打开和关闭开关都要认证，第一次打开也一样；设备没有锁屏凭据时不许打开，提示先去系统设置开启；认证取消或失败时不写设置；进程重启只读存储的值。Android 的访客剪贴板不看开关，删除一律认证。维护者决定三端在同一个 PR 里做。

## Decision

三端用同一套切换规则，各自一个纯策略：Android `ClipboardDeleteAuthenticationPolicy`（Java）、iOS `ClipboardDeleteAuthenticationPolicy`（`SharedUI/clipboard/ClipboardDeleteAuthentication.swift`）、HarmonyOS `ClipboardDeleteAuthenticationPolicy.ts`。`decide(当前值, 目标值, 认证可用)`：值不变什么也不做；认证不可用时两个方向都拒绝（`UNAVAILABLE`）；其余一律先认证（`AUTHENTICATE`），没有「直接保存」的分支。这是与 [Android 防止截屏](2026-10-29-android-screen-capture-protection.md) 唯一的区别：那边第一次打开直接保存。打开以后用户又移除了锁屏，删除和关闭都做不了，只有重新设锁屏才能解开，与防止截屏的先例相同。拒绝时的提示三端同义：「请先在系统设置中开启锁屏验证（指纹、图案、密码等）后再使用」（iOS 措辞换成锁屏密码与面容 ID、触控 ID）。

开关只属于这台设备，三端都不随账号同步：

- Android：`AndroidLocalSettings` 的 `platform.android.clipboard_delete_authentication`，不在同步字段表里；`LocalBackupPolicy.backsUpLocalSetting` 对它返回 false，不导出也不从备份恢复，理由与防止截屏相同：一份关着的旧备份能不经认证关掉它。键盘进程在 `MSIMEInputService` 重读本地设置时读入。
- iOS：App Group 的 `UserDefaults`（`keyboard.clipboard.deleteAuthentication`），与隐私模式一样不在共享偏好文档里；设置 App 写，键盘每次删除时现读。
- HarmonyOS：设置应用的 `files/state/clipboard-delete-authentication.json`，经共享沙箱镜像（`SharedStateMirrorPolicy.SETTINGS_ENTRIES`）单向传给键盘，键盘不发布它，改不回去。设置界面是共享 React 页：宿主提供 `SettingsClient.clipboardDeleteAuthentication` 时「剪贴板」组末尾多这一个开关，与 `screenCaptureProtection` 一样按客户端是否存在决定显示，不新增 `HostCapabilities` 字段，因为开关的值和规则都在宿主原生侧，不是偏好文档里的字段。

删除入口：

- Android：`removeClipboardItem`（长按「删除」、左滑垃圾桶）和 `removeClipboardItems`（多选）都只经 `deleteClipboardAfterAuthentication` 交出真正的删除。它按 `requiresAuthentication(开关, 访客模式)` 决定直接删还是走现有的认证桥 `authenticateClipboard`。多选在发起时复制一份所选文字，整批认证一次。发起时记下当时的 `ClipboardHistoryStore`，回调时存储换了（认证期间切换了访客 / 本机剪贴板）就作废并提示，不删到另一个存储里同样文字的记录；不靠自增认证代次作废，否则迟到的结果在核对代次时直接返回，`clipboardAuthenticationBusy` 永远清不掉。撤销不经这里。清空本来就要认证，不变。
- 认证桥 `ClipboardAuthenticationActivity` 在设备没有可用认证时回专门的结果码 `RESULT_UNAVAILABLE`（2002），键盘据此提示先开锁屏，不再说「未通过系统认证」；这也改了现有搜索、导出、清空在无锁屏设备上的提示。键盘在拉起认证桥之前先用框架的 `KeyguardManager.isDeviceSecure()` 预查，没有锁屏凭据就当场提示、不拉起：认证桥一打开就抢走编辑器焦点，键盘收起、面板关掉，访客剪贴板的删除总要认证，不预查的话没设锁屏的设备上每点一次删除键盘都会收起；不用 AndroidX 的 `BiometricManager`，因为输入法服务要在 check-host 不带 AndroidX 的编译里编过，认证桥里那次 `canAuthenticate` 留作兜底。认证桥本身没能打开（`startActivity` 抛异常）时提示「无法打开系统认证」，不复用锁屏引导。认证窗口没走到回调就被结束时，`onDestroy` 在 `isFinishing()` 下补发取消：删除也走这座桥以后，这种情况一旦发生，busy 卡住会让之后所有剪贴板认证被静默忽略。
- iOS：键盘剪贴板面板「管理记录」菜单的「删除」和确认后的「清空」，以及设置 App「剪贴板历史」页的左滑删除、长按菜单删除和清空，都经 `deleteAfterAuthentication`。iOS 设置 App 的历史页进入时不认证（与 Android 的搜索页不同），不一起管就能从那里绕过。认证用 `LAContext` 的 `.deviceOwnerAuthentication`（锁屏密码，可用时由面容 ID 或触控 ID 代替），`ClipboardDeleteAuthenticator` 进行中不叠第二个认证框，面板收起或页面离开时 `invalidate` 并丢掉迟到的结果。键盘扩展 Info.plist 补 `NSFaceIDUsageDescription`。
- HarmonyOS：键盘里管理行的「删除」、2in1 行尾的「删除」和确认后的「清空」都经 `KeyboardView.deleteClipboardAfterAuthentication`，每次现读镜像过来的开关。认证复用 `ScreenCaptureAuthentication`（锁屏密码加可用的指纹、人脸，ATL1，不带冷却），只换认证框标题。

iOS 与 HarmonyOS 的键盘没有访客剪贴板、没有删除撤销、也没有多选删除，所以只看开关；它们把「清空」一起放进门里，因为那两端的清空原本完全不认证，开关打开后若清空还能直接删光，开关形同虚设。Android 的清空一直要认证，不受开关影响。

### iOS 键盘扩展里能不能用 LocalAuthentication

先查 SDK：iPhoneOS 27.1 SDK 的 `LocalAuthentication.framework/Headers/LAContext.h` 里，`evaluatePolicy:localizedReason:reply:` 和 `canEvaluatePolicy:error:` 只标了 `API_UNAVAILABLE(tvos)`，没有 `NS_EXTENSION_UNAVAILABLE` 或 `API_UNAVAILABLE(ios_app_extension)`，键盘扩展里能编译、能调用。头文件说明不了系统会不会在键盘扩展里拉起认证界面，所以又在 iOS 27.1 模拟器上做了实测：一个只有宿主 App 和键盘扩展的最小工程，键盘出现时调用 `canEvaluatePolicy(.deviceOwnerAuthentication)` 和 `evaluatePolicy`。结果：`canEvaluatePolicy` 为真；登记面容 ID 并发送匹配通知后，键盘扩展收到 `success=true`，coreauthd 记下 `caller: <键盘扩展 bundle id> … biometry … success: 1`；只有锁屏密码时系统同样显示了认证界面（`Did show UI with error: (null)`），进行中关掉宿主 App 时回答 `LAErrorSystemCancel`（-4）。模拟器冷启动后的第一次调用曾报 `-1000 UI activation timed out after 5 seconds`，之后复现不出；`ClipboardDeleteAuthenticator` 把任何失败都当未通过，不删。没有完全访问权限时同样可用，但剪贴板历史本来就需要完全访问权限。真机没有验证。

## Alternatives considered

- 没有锁屏凭据的设备上，访客剪贴板的删除照旧放行（#10429 的做法）：最强理由是访客通常不知道机主的锁屏凭据，没设锁屏的手机上访客模式里永远删不掉记录，而同一台手机在本机模式、开关关闭时可以随意删。维护者接受了报告者的字面规则（访客模式删除一律认证，没有锁屏时只提示先设），所以不放行。
- 认证期间切换访客 / 本机剪贴板时，在 `toggleClipboardMode` 里自增 `clipboardAuthenticationGeneration` 作废回调：写法最短，但迟到的回调在核对代次处直接返回，执行不到清 busy 的那一行，之后的剪贴板认证全被吞掉，直到进程重启。改为记下发起时的存储引用。
- iOS 键盘里删除时跳回设置 App 认证再回来：不依赖扩展里的 LocalAuthentication，但键盘扩展打开宿主 App 只能绕私有接口，回来后编辑框焦点和面板状态都会丢；实测扩展里可以直接认证，不需要这条路。
- HarmonyOS 只在设置页加开关、键盘里不认证：只会是一个假的门。键盘里直接调 `userAuth` 失败时一律不删，绝不放行。
- HarmonyOS 用 `HostCapabilities` 新字段控制开关显示：其他按宿主能力显示的项都是偏好文档里的字段；这项的值在宿主原生侧，读写都要经宿主桥，沿用 `screenCaptureProtection` 的「提供客户端才显示」更直接。
- Android 多选删除在认证通过后才退出多选：取消认证时能保留选择，体验更好；但键盘顶行多选「导出」已经是「先退出多选再认证」，这次保持一致，取消后需要重新选择。

## Consequences

收益：开关打开后三端键盘里删除本机剪贴板记录都要先过系统认证；切换受认证保护，同步、备份和键盘进程都改不了它。Android 的认证桥在无锁屏设备上给出正确的提示，认证窗口被直接结束时不再让后续认证失效。

代价与已知风险：

- Android：认证桥是 `FLAG_ACTIVITY_NEW_TASK` 的 Activity，拉起时编辑器窗口失焦，键盘可能收起并关掉剪贴板面板（`finishInputViewPresentation` → `closeToolbarPanels`）。撤销倒数从认证成功、真正删除时才开始，只有 5 到 9 秒，用户回到键盘时可能要重开面板才看得到撤销，甚至已经过期。没有在设备上验证。
- 开关打开后用户移除锁屏，删除和关闭都被拒绝，只有重新设锁屏才能解开；访客模式在没有锁屏的设备上删不掉记录。
- iOS 只在模拟器上证实键盘扩展里能弹认证界面并收到结果，没有在真机上验证；面容 ID 第一次在键盘扩展里使用时系统会按扩展的 `NSFaceIDUsageDescription` 询问授权。
- HarmonyOS 的输入法扩展里能否直接拉起 `userAuth` 认证框没有在设备上验证；拉不起时删除一律不执行，开关打开的用户在键盘里删不掉记录，只能先在设置里关掉。设置应用切换后，键盘在下一次编辑框获得焦点导入镜像时才跟上。

验证：`platforms/android/check-host.sh`（含新增 `ClipboardDeleteAuthenticationPolicySmoke` 和删除接线、设置页接线、备份排除、认证桥结果码与 `onDestroy` 补发的结构检查）；`platforms/harmony/tests/run.sh`（含 `clipboard-delete-authentication.test.ts` 和 ArkTS 接线检查）；`hvigorw assembleHap` 编出 HAP；`apps/desktop` 的 vitest（`clipboard-delete-authentication.test.tsx`）与两端 `tsc --noEmit`；`scripts/test-ios-clipboard-delete-authentication.py`（`ci-ios.yml` 里也跑；切换规则、删除判断、本机存储、认证器的一次性、失败、取消与迟到结果）；iOS 键盘扩展和设置 App 在模拟器 SDK 下编译通过（链接缺 Rust 主机库，未链接）。设备验收（三端的认证、取消、无锁屏提示，Android 撤销是否在认证返回后仍可用，API 28 的凭据确认 Activity 分支）都没有做。
