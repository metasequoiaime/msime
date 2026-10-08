# Agent Note: Fcitx5 主题文件不写顶层键，SupportedScale 写成分组

Status: implemented

## Problem

Linux 宿主为 Fcitx5 经典界面生成 `fcitx5/themes/<Theme>/theme.conf`。同一份文件还有第二个读者：GNOME Wayland 这类不让输入法自己弹窗的桌面上，Wayland 原生的 GTK 程序由程序里的 fcitx5-gtk 模块画客户端输入面板，它在 `gtk3/fcitxtheme.cpp` 的 `Theme::load` 里用 GLib 的 `GKeyFile` 读这份主题，读不进就退回 `default`。生成的文件第一行是顶层的 `SupportedScale=2`，GKeyFile 报 `Key file does not start with a group` 整份拒绝，这些程序里的候选窗于是成了白底黑字，皮肤颜色和装饰图全都没有。

Fcitx5 自己的 INI 读取器（`readAsIni`）允许顶层键，所以经典界面一直正常，问题只在客户端面板上出现。

## Decision

`CandidateFcitxTheme.h` 生成的 theme.conf 以 `[Metadata]` 开头，不写任何顶层键。经典界面加载 `@2x` 图要的倍率写成分组：

```
[SupportedScale]
Value=2
```

这是上游 fcitx/fcitx5#1695 把 `SupportedScale` 改成 `std::optional<int>` 之后的写法：`OptionalFallbackMarshaller` 先读子键 `Value`，没有子键才读顶层值，序列化时也只写 `Value`。GKeyFile 把它当成普通分组，fcitx5-gtk 不读这个分组，不受影响。

各版本的结果：

- 5.1.22 及更早：没有这个选项，分组被忽略，和以前一样只用 1x 图。
- 5.1.23：选项是 `Option<int>`，只认顶层值；分组下的节点自身没有值，解析失败，`Configuration::load` 把它复位成默认的 1，不加载 `@2x` 图，其余主题照常。
- 5.1.23 之后的版本（含 #1695）：读到 2，加载 `@2x` 图。

`linux-candidate-fcitx-theme-gkeyfile` 用 GKeyFile 读带装饰图和不带的两种主题，检查能读、`SupportedScale/Value`、`InputPanel/Background` 的颜色和 `Overlay` 都读得到；`linux-candidate-fcitx-theme` 钉住文件以 `[Metadata]` 开头、没有 `SupportedScale=` 这一行。

## Alternatives considered

- 保留顶层 `SupportedScale=2`：5.1.23 是目前唯一需要它才加载 `@2x` 图的版本，保留它能让这个版本在高分屏上更清楚。但代价是所有 GNOME Wayland 上的 GTK 程序完全没有皮肤，影响面大得多，而 5.1.23 少了它只是高分屏稍糊。
- 直接删掉 `SupportedScale`、什么都不写：同样修好了 fcitx5-gtk，但以后所有支持倍率的 Fcitx5 都不加载我们生成的 `@2x` 图，`FcitxThemeImages.h` 按 2 倍画的那一套就白画了。分组写法在 5.1.23 上与它等价，在之后的版本上能用上 `@2x` 图，所以选分组。
- 写一个空分组名 `[]` 再跟 `SupportedScale=2`：GKeyFile 的 `g_key_file_is_group_name` 拒绝空组名，同样整份失败。

## Consequences

GNOME Wayland 上由 fcitx5-gtk 画的候选窗能读到水杉皮肤。Fcitx5 5.1.23 在高分屏上不加载 `@2x` 图，比带顶层键时稍糊；更早和更新的版本行为不变或更好。以后往 theme.conf 加任何经典界面的顶层选项都必须同样写成分组，`linux-candidate-fcitx-theme-gkeyfile` 会在顶层键出现时失败。尚未在 GNOME Wayland 会话里亲眼确认 fcitx5-gtk 画出的候选窗，也没有在 5.1.23 之后的 Fcitx5 上实测 `@2x` 图被加载，后者的结论来自上游源码。
