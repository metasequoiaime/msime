# Agent Note: 鸿蒙键盘打开设置的链接带子目标，长按九键左侧符号栏直达编辑页

Status: implemented

## Problem

[九键左侧符号栏](2026-10-29-nine-key-sidebar-shared.md)的两张表在「屏幕键盘」页「按键布局」子页里改，各有一个编辑页。Android 长按符号栏打开「按键布局」并直接进入那张表的编辑页（`NineKeySidebarPolicy.EDIT_ARGUMENT` 即 `edit_sidebar_key`，值是共享偏好名）。鸿蒙键盘打开设置只能指定页面：Want 里一个 `page`，取值是 `SettingsPageLink.PAGES` 里的设置页 id。「按键布局」不是设置页，是「屏幕键盘」页里的一层子视图，编辑页又在它上面一层，所以长按只落到「屏幕键盘」页，用户还要再点两下，而且自己得知道入口在哪。

## Decision

### 链接格式

键盘打开设置的 Want 参数：

- `page`：不变，`SettingsPageLink.PAGES` 之一。
- `target`：新增，可选，那一页里要直接打开的一处。只有 `screen-keyboard` 有子目标（`SettingsPageLink.SCREEN_KEYBOARD_TARGETS`），取值是九键左侧符号栏两张表的共享偏好名：`touch_nine_key_sidebar_symbols`（字母表）、`touch_nine_key_digit_sidebar_symbols`（数字表），与 Android `edit_sidebar_key` 的值相同。

校验在请求方和应答方各做一次，规则相同（`SettingsPageLink.fromParameters`）：`page` 不可链接时整条链接作废；`target` 缺失、不是字符串、认不得或不属于这一页时丢掉 `target`，照旧打开页面。只带 `page` 的旧链接意思不变。子目标只用于导航：编辑页打开时什么也不写，用户点「保存」才写那张表。

### 从键盘到网页

1. `KeyboardView.openSidebarSettings(digits)`：数字层的两处（出厂、自定义）传数字表，字母层和笔画的四处传字母表，调 `KeyboardSession.openSettings('screen-keyboard', target)`。
2. `KeyboardSession.openSettings(page, target)` 校验后交给 `onOpenSettings(page, requestMicrophone, target)`；`KeyboardExtensionAbility.openSettings` 把 `target` 和 `page` 放在同一个 Want 参数表里。
3. `EntryAbility.linkedPage` 读 `page` 与 `target`，得到 `SettingsLinkRequest { page, target }`。冷启动存进 `pendingPage`，已运行时交给设置页的 `onOpenPage`。
4. `Settings.ets` 桥接：`pendingPage()` 回复 `{ ok, value: page, target }`，`value` 还是页面 id，与旧回复同一个形状；窗口已显示时调 `msimeHarmonyOpenPage(page, target)`，没有子目标时只传页面。
5. 网页 `apps/harmony/src/settings-link.ts` 再按 `LINKED_PAGES` 与 `LINKED_SCREEN_KEYBOARD_TARGETS` 校验一次，`HarmonySettings` 把它变成 `SettingsPage` 的 `initialPage` / `initialTarget`（冷启动、欢迎流程结束后）或 `route { page, target, nonce }`（窗口已打开）。

### 共享设置页

- `useSettingsNavigation` 持有一个 `SettingsLinkTarget { page, target, serial }`。路由先选页，再交出子目标；首次挂载的子目标在历史记录初始化之后交出。理由是手机上每层子视图各压一条历史记录，页面的记录必须在下面，系统返回手势才能一层层退回去。页面换成别的时丢掉子目标，免得用户以后自己点进那一页时又被带进编辑页。
- `ScreenKeyboardSettingsPage` 在自己是当前页时取走子目标（`clearLinkTarget`），只处理一次。`nineKeySidebarKindForPreference` 把偏好名换成哪张表；这一页有「按键布局」时打开它，再把 `{ kind, serial }` 交给 `TouchKeyLayoutSettings` 的 `editRequest`。
- `TouchKeyLayoutSettings` 在子页打开时按 `serial` 只处理一次请求：字母表先切到 9 键分段（版本没有 9 键时停在「按键布局」），再打开编辑页；那张表的编辑页已经开着时不再压一条记录，草稿保留。返回一步关编辑页，再一步关「按键布局」。
- 表单要等偏好读完才挂载，所以编辑页拿到的总是盘上的表，不会是还没读完时的默认值。

### 一致性检查

`scripts/test-harmony-settings-links.py` 原来核对两份页面名单，现在还核对键盘的 `SCREEN_KEYBOARD_TARGETS`、网页的 `LINKED_SCREEN_KEYBOARD_TARGETS`，以及 `packages/ui` 的 `nineKeySidebarPreference` 三者相同。少了任何一边，子目标都会被悄悄丢掉。`platforms/harmony/tests/run.sh` 钉住六处长按各传哪张表。

## Alternatives considered

- **新增一个设置页 id，例如 `key-layout-letter`**：不用改 Want 的形状，名单照旧是一张表。不采用：「按键布局」和编辑页都不是设置注册表里的页面，硬加 id 要么在注册表里造出一个没有内容的页，要么在网页里写一张 id 到子视图的特殊映射，而且子视图的历史记录也得从页面路由里特殊处理。
- **在 `page` 里拼路径，例如 `screen-keyboard/touch_nine_key_sidebar_symbols`**：只有一个参数。不采用：旧的校验会把整个字符串当成不认得的页面，打开的是「输入」页而不是「屏幕键盘」；两段还得各自校验，拆开放两个参数更直接。
- **参数名照抄 Android 的 `edit_sidebar_key`**：两端字面一致。不采用：Android 那个是一个页面专用的参数；鸿蒙所有页面共用一条 Want 路由，用通用的 `target` 以后别的页面也能用，取值与 Android 相同就够了。
- **`pendingPage()` 的 `value` 改成 `{ page, target }` 对象**：一个字段装完。不采用：网页旧的读法只认字符串，`target` 放在旁边，`value` 保持原形状，读不懂 `target` 的一方照旧拿到页面。

## Consequences

- 收益：鸿蒙长按符号栏与 Android 一样直达那张表的编辑页；链接格式可给别的页面加子目标，只需在 `SettingsPageLink.targets`、`settings-link.ts` 和设置页里各加一处，脚本会核对前两处。
- 代价：同一条规则写在 ArkTS 与网页两处（与页面名单相同的做法），靠脚本保证一致。手机上从键盘进入时历史栈会多两层，返回要退三次才离开「屏幕键盘」页，与用户自己点进去时相同。
- 平板没有「按键布局」入口（只在鸿蒙手机设置里），链接到平板设置时停在「屏幕键盘」页。

## Verification

- `bash platforms/harmony/tests/run.sh` 通过，其中 `kb-session.test.ts` 覆盖 `fromParameters`：页面与子目标一起通过、只有页面的旧链接、不认得或属于别的页面的子目标被丢掉、子目标不能带出不可链接的页面。
- `apps/desktop` 下 `tests/settings/harmony-settings-link.test.ts` 覆盖网页的校验和 `pendingPage()` 回复（含没有 `target` 的旧回复）；`tests/settings/nine-key-sidebar-settings.test.tsx` 覆盖：冷启动链接打开字母表编辑页并切到 9 键分段，已打开时的路由打开数字表编辑页、历史记录一层层退回、不写偏好，链接只生效一次，不认得的子目标只打开「屏幕键盘」，没有 9 键时字母表请求不打开编辑页。
- `python3 scripts/test-harmony-settings-links.py` 通过；把网页一侧的数字表偏好名改错后如期报出两处不同。
- Studio 上经 `rbuild` 跑 `ohpm install && hvigorw assembleHap --no-daemon`，CompileArkTS 完成、BUILD SUCCESSFUL，ArkTS 警告都不在本次改动的文件里。

没有验证：没有在真机或模拟器上长按过符号栏，冷启动和设置窗口已打开两条路径、手机返回手势一层层退回都只有 jsdom 下的测试。
