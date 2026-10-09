# Agent Note: IBus 引擎不再把键盘布局换成 us

Status: implemented

## Problem

#6365 反映 Linux 版只能用 QWERTY，Dvorak 等布局用不了。两个宿主的按键处理本身是跟布局走的：IBus 的 `process_key` 拿 keyval、Fcitx5 的 `FcitxState::key` 拿 `key.sym()`，都是按当前布局翻译后的符号，字母快捷键（Ctrl+Shift+F、Ctrl+Shift+E 等）也按符号匹配，只有候选序号键有意按物理数字行取键。问题出在 IBus 组件文件 `platforms/linux/data/msime-linux.xml.in`：引擎写着 `<layout>us</layout>`。IBus 自带面板在 X11 上切换引擎时调用 `XKBLayout.set_layout(engine)`，只有布局为 `default` 时才不动当前键盘映射；GNOME Shell 把 IBus 输入源的 `xkbId` 取成引擎的 `layout`，`us` 是有效的 XKB 布局，于是切到水杉就套用美式布局。用户的 Dvorak 在切到本输入法的那一刻被换掉，产品里也没有入口改回来。

Fcitx5 没有这个问题：`msime-inputmethod.conf` 不写 `LayoutHint`，`msime-linux-setup` 把本输入法加入当前组时布局字段留空，按键用的是输入法组的默认布局或用户给这个条目单独选的布局。

## Decision

引擎的 `<layout>` 改为 `default`。IBus 面板和 GNOME Shell 切到本输入法时都保持当前布局，keyval 由用户自己的布局翻译。`ibus-rime`、`ibus-libpinyin` 的组件文件也写 `default`。

`tests/core/ibus_component_contract.py`（ctest `linux-ibus-component-contract`）断言每个引擎的 `<layout>` 是 `default` 且不写 `layout_variant` / `layout_option`；`tests/core/fcitx5_contract.py` 断言输入法条目不带 `LayoutHint`。Linux README 的「输入细节与平台差异」写明两个宿主的布局来源和 Fcitx5 下的设置方法。

## Alternatives considered

- **保留 `us`，让用户在 IBus 首选项里开「使用系统键盘布局」**：这一项不用改产品，对一直用美式布局的用户零风险。但它只对 IBus 自带面板生效，GNOME Shell 不读这个设置，GNOME 用户（IBus 的主要用户）没有任何办法；而且要每个 Dvorak 用户自己去找一个与输入法无关的开关。
- **在宿主里把 keycode 按 QWERTY 映射成字母或反向映射**：等于自己再实现一遍 XKB，而 keyval 本来就是按布局翻译好的，问题不在按键处理这一层。

## Consequences

Dvorak、Colemak、AZERTY 等拉丁布局的用户切到水杉后保持自己的布局，拼音字母和字母快捷键都按键帽上的字母走。代价是：切到水杉之前用的是非拉丁布局（例如俄文）的 IBus 用户，此前会被强制切成 us，现在会保留那个布局，打出的 keyval 不是拉丁字母，拼音组不起来；这种组合需要用户把一个拉丁布局放在本输入法之前。这与 `ibus-rime`、`ibus-libpinyin` 的行为一致。

## Verification

- `python3 platforms/linux/tests/core/ibus_component_contract.py`：改前失败（`<layout>` 为 `'us'`），改后通过。
- `python3 platforms/linux/tests/core/fcitx5_contract.py`、`python3 scripts/test-linux-editions.py`、`python3 scripts/test-editions.py`：通过。
- `rbuild bash platforms/linux/build-container.sh`：在容器里构建 IBus、Fcitx5 两个宿主并跑 ctest，74 项全部通过。
- 没有在真实 GNOME 或 IBus 面板会话里切换布局实测。
