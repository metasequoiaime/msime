# Agent Note: macOS 首次安装不需要注销重登

Status: implemented

## Problem

用户装完 pkg 之后，在系统设置的「添加」对话框里找不到水杉输入法，只有注销重登之后才出现。安装包的结束页也直接这样要求：「第一次在这台 Mac 上安装时，请在苹果菜单中选『退出登录』，重新登录后再到系统设置添加」。这一步对每个新用户都是一道门槛，很多人会以为没装上。

这句话的依据是 [首次安装要重新登录](2026-10-09-macos-first-install-login-required.md) 和 platforms/macos/README.md「安装与输入源注册」里记的一条「macOS 限制」：一个在本次登录开始时不在输入源列表里的 bundle identifier，`TISRegisterInputSource` 返回 noErr，但要到下次登录才进注册表和「添加」对话框。

2026-11-28 在 macOS 27.2.1（26B131）上复测，这条限制不成立。每一步都用全新的、这次登录里从没出现过的 identifier：

- 自己编的最小 IMK bundle，放进 `~/Library/Input Methods`，`TISRegisterInputSource` 返回 0，紧接着 `TISCreateInputSourceList(includeAllInstalled)` 就列出了父源和模式。
- 已装的水杉输入法整体复制一份，Info.plist 里只把 `app.msime.inputmethod.MetasequoiaIME` 换成新 identifier，ad-hoc 重签，用 bundle 自己的 `--register-input-source` 登记：退出码 0，12 条源全部在注册表里，`.Hans` 是 enabled。
- 同一份副本用 `pkgbuild` 打成组件包，像设置应用的系统安装那样以 root 跑 `installer -pkg` 装进 `/Library/Input Methods`（install.log 里是 `Registered bundle … for uid 0`），再以登录用户身份 `lsregister -f` 和 `--register-input-source`：退出码 0，12 条源全在注册表里，用户缓存目录下的 `com.apple.IntlDataCache.le.kbdx` 立即带上了它。
- 同一台机器上的 AIME（同样装在 `/Library/Input Methods`）第一次安装时，postinstall 以用户身份登记、启用并选中了自己的模式，install.log 里是 `enable …: 0`、`select …: 0`。

真正看不到新输入法的只有「添加」对话框。它所在的键盘设置扩展（`KeyboardSettings.appex`）在 `com.apple.Keyboard-Settings.extension/` 下有一份自己的 `com.apple.IntlDataCache.le.kbdx`，只在进程启动时读一次。上面那次 root 安装之后，这份缓存里查不到新 identifier。结束扩展进程、删掉这份缓存，再打开键盘设置，重建的缓存里就有它了。设置应用装好输入法后跑的 `refresh_system_input_source_lists` 做的正是这几步，这次也按它的顺序实测过。

所以首次安装同一次登录里就能添加，「先注销重登」是基于错误前提的提示。

## Decision

- 安装包结束页（`platforms/macos/resources/installer/{zh-Hans,en}.lproj/conclusion.html.in`）不再要求首次安装注销重登。第二步改成：设置应用的窗口显示安装完成后，直接去系统设置添加，并写明不需要退出登录。
- `scripts/test-macos-package-resources.py` 原来要求结束页里有「退出登录」/「Log Out」，现在反过来，结束页里不能出现「重新登录」/「log back in」。
- platforms/macos/README.md 删掉那条「新 identifier 要到下次登录」的限制，改记这次的复测结果。保留的是「添加」对话框读扩展自己的缓存、要刷新才看得到这一条，它对新 identifier 和已有 identifier 新增的模式同样适用。
- 设置应用里登记失败时的 `login_required` 流程不改。它只在 `--register-input-source` 确实失败时出现，这时候重新登录让系统按目录重扫，仍然是一个有效的兜底。

## Alternatives considered

- **把输入法直接放进 pkg 的载荷、在 postinstall 里以用户身份登记（AIME 的做法）。** 最强的理由是装完 pkg 就装好了输入法，不需要再在设置应用里点「立即安装」、再输一次管理员密码。这次没有做，因为它要改掉 pkg 只装设置应用的设计（[只发 pkg](../process/2026-11-02-macos-pkg-only.md)），还牵涉系统更新助手的安装和 Homebrew、MDM 的命令行安装路径。这是一个独立的取舍，不该混在纠正错误提示的这次改动里。注销重登这件事不依赖它，现在的两步流程已经能在同一次登录里完成。
- **在安装包上加 `onConclusion="RequireLogout"`。** 让系统强制注销，最强的理由是行为确定。没有采用，因为复测已经证明不需要注销，强制注销只会把错误的提示变成错误的流程。

## Consequences

- 新用户装完 pkg、在设置应用里点「立即安装」之后，直接去系统设置就能添加水杉输入法，不再被要求注销。
- 这次复测在 macOS 27.2.1 上做的，用的是从没登录过这个 identifier 的会话里新造的 identifier，没有在一台全新的 Mac 或全新的用户账户上从头装一遍正式签名的 pkg。如果有用户在新机器上仍然在「添加」里找不到它，先查扩展的缓存有没有被刷新（`com.apple.Keyboard-Settings.extension/com.apple.IntlDataCache.le.kbdx` 里有没有 `app.msime.inputmethod.MetasequoiaIME`），不要再归因到「macOS 要到下次登录」。
- `login_required` 的界面文案（`packages/ui/src/account/macos-install-page.tsx`、`packages/ui/src/settings/input-source-startup-notice.tsx`）仍然用「macOS 只在登录时读取新装的输入法」解释原因，这个解释和本次结论不一致，但只在登记真的失败时出现，留待单独处理。
