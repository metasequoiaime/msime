#!/usr/bin/env python3
"""Windows 原生设置窗口的插画与侧栏图标没有和共享设置 UI 走散。

- 页首插画：`packages/ui/src/assets/mascot/*.webp` 的每个场景在 `SettingsMascotIntro.h` 的 `MascotScene` 里都有，`platforms/windows/settings/assets/mascot/` 里有同名、同尺寸的 PNG，`Settings.rc` 按枚举顺序把它们打成 RCDATA 110 起的资源。
- 页首文案：`SettingsMascotIntro.h` 的每一句说明连同场景都逐字出自 `settings-mascot-intro.tsx`；没有标签的页面用的是经路由别名对应到它的共享页面那一句。
- AI 服务商图标：`AiSettings.h` 的 `provider_icons` 与 `ai-provider-icon.tsx` 是同一张表，`platforms/windows/settings/assets/providers/` 里有每张图源转成的 80 × 80 PNG 和同一份许可，`Settings.rc` 按表的顺序打成 RCDATA 150 起的资源。
- 侧栏图标：`SettingsNavIcons.h` 每一页记的图源文件还在，并且是共享设置 UI 给对应页面的那张（`settings-page-registry.ts` 的 `icon` 经 `app-resources.ts` 的 `settingsIcons` 落到的文件）。

PNG 是否由 WebP 逐像素转来这里不查（需要 WebP 解码器）；图源换了以后按 `platforms/windows/settings/README.md` 重新转换。
"""

from __future__ import annotations

import pathlib
import re
import struct

ROOT = pathlib.Path(__file__).resolve().parent.parent
UI = ROOT / "packages/ui/src"
SETTINGS = ROOT / "platforms/windows/settings"


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def png_size(path: pathlib.Path) -> tuple[int, int]:
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n" and data[12:16] == b"IHDR", f"{path} 不是 PNG"
    return struct.unpack(">II", data[16:24])


def webp_size(path: pathlib.Path) -> tuple[int, int]:
    data = path.read_bytes()
    assert data[:4] == b"RIFF" and data[8:12] == b"WEBP", f"{path} 不是 WebP"
    chunk = data[12:16]
    if chunk == b"VP8X":
        width = int.from_bytes(data[24:27], "little") + 1
        height = int.from_bytes(data[27:30], "little") + 1
        return width, height
    if chunk == b"VP8L":
        bits = int.from_bytes(data[21:25], "little")
        return (bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1
    if chunk == b"VP8 ":
        width, height = struct.unpack("<HH", data[26:30])
        return width & 0x3FFF, height & 0x3FFF
    raise AssertionError(f"{path} 的 WebP 块 {chunk!r} 不认识")


intro_header = read(SETTINGS / "SettingsMascotIntro.h")
enum = re.search(r"enum class MascotScene \{(.*?)\};", intro_header, re.S)
assert enum, "读不到 MascotScene"
scenes = re.findall(r"\b([a-z]+),", enum[1])
shared_scenes = sorted(path.stem for path in (UI / "assets/mascot").glob("*.webp"))
assert sorted(scenes) == shared_scenes, f"MascotScene 与共享插画不一致：{scenes} != {shared_scenes}"
mascot_union = re.search(r"export type MascotScene =(.*?);", read(UI / "core/mascot.tsx"), re.S)
assert mascot_union, "读不到共享 UI 的 MascotScene"
assert sorted(re.findall(r'"([a-z]+)"', mascot_union[1])) == shared_scenes, "共享 UI 的场景与插画文件不一致"

base = re.search(r"mascot_resource_base = (\d+);", intro_header)
assert base, "读不到 mascot_resource_base"
resources = re.findall(r'^(\d+) RCDATA "assets\\\\mascot\\\\([a-z]+)\.png"$', read(SETTINGS / "Settings.rc"), re.M)
expected = [(str(int(base[1]) + index), scene) for index, scene in enumerate(scenes)]
assert resources == expected, f"Settings.rc 的插画资源与 MascotScene 顺序不一致：{resources} != {expected}"
packaged = sorted(path.stem for path in (SETTINGS / "assets/mascot").glob("*.png"))
assert packaged == shared_scenes, f"打包的 PNG 与共享插画不一致：{packaged} != {shared_scenes}"
for scene in scenes:
    png = png_size(SETTINGS / "assets/mascot" / f"{scene}.png")
    webp = webp_size(UI / "assets/mascot" / f"{scene}.webp")
    assert png == webp, f"{scene}.png 的尺寸 {png} 与 WebP 的 {webp} 不同，图源换过以后没有重新转换"

# AI 服务商下拉的品牌图标：AiSettings.h 的 provider_icons 与共享 UI 的 ai-provider-icon.tsx 是同一张表（服务商、图源文件、深色底板），PNG 都在，Settings.rc 按表的顺序从 provider_icon_resource_base 起打成 RCDATA，许可随图标一起带上。
ai_header = read(SETTINGS / "AiSettings.h")
icon_table = re.search(r"provider_icons\{\{(.*?)\}\};", ai_header, re.S)
assert icon_table, "读不到 provider_icons"
win_icons = re.findall(r'\{L"([a-z]+)", "([a-z]+)", (true|false)\}', icon_table[1])
icon_count = re.search(r"std::array<ProviderIcon, (\d+)> provider_icons", ai_header)
assert icon_count and int(icon_count[1]) == len(win_icons), "provider_icons 的条目数读不全"
icon_tsx = read(UI / "settings/ai-provider-icon.tsx")
imports = dict(re.findall(r'^import ([a-z]+) from "\.\./assets/providers/([a-z]+)\.svg";$', icon_tsx, re.M))
icon_record = re.search(r"const icons: Record<string, string> = \{(.*?)\};", icon_tsx, re.S)
assert icon_record, "读不到 ai-provider-icon.tsx 的 icons"
shared_icons = {}
for entry in re.findall(r"([a-z]+)(?:: ([a-z]+))?,", icon_record[1]):
    provider, variable = entry[0], entry[1] or entry[0]
    shared_icons[provider] = imports[variable]
dark_tiles = re.search(r'\[([^\]]*)\]\.includes\(provider\) \? "dark"', icon_tsx)
assert dark_tiles, "读不到深色底板的服务商"
shared_dark = set(re.findall(r'"([a-z]+)"', dark_tiles[1]))
assert {provider: file for provider, file, _ in win_icons} == shared_icons, f"provider_icons 与 ai-provider-icon.tsx 不一致：{win_icons} != {shared_icons}"
assert {provider for provider, _, dark in win_icons if dark == "true"} == shared_dark, "深色底板的服务商与 ai-provider-icon.tsx 不一致"
icon_base = re.search(r"provider_icon_resource_base = (\d+);", ai_header)
assert icon_base, "读不到 provider_icon_resource_base"
icon_resources = re.findall(r'^(\d+) RCDATA "assets\\\\providers\\\\([a-z]+)\.png"$', read(SETTINGS / "Settings.rc"), re.M)
expected_icons = [(str(int(icon_base[1]) + index), file) for index, (_, file, _) in enumerate(win_icons)]
assert icon_resources == expected_icons, f"Settings.rc 的服务商图标与 provider_icons 顺序不一致：{icon_resources} != {expected_icons}"
mascot_ids = {int(number) for number, _ in resources}
assert not mascot_ids & {int(number) for number, _ in icon_resources}, "服务商图标和插画的资源号重叠"
for _, file, _ in win_icons:
    assert (UI / "assets/providers" / f"{file}.svg").is_file(), f"图源 {file}.svg 不存在"
    assert png_size(SETTINGS / "assets/providers" / f"{file}.png") == (80, 80), f"{file}.png 不是 80 × 80"
assert (SETTINGS / "assets/providers/LICENSE").read_bytes() == (UI / "assets/providers/LICENSE").read_bytes(), "服务商图标的许可没有随图标带上"

# 路由别名：共享设置页 id 到本窗口的页面。
navigation = read(SETTINGS / "SettingsNavigation.h")
aliases = re.search(r"route_aliases\{\{(.*?)\}\};", navigation, re.S)
assert aliases, "读不到 route_aliases"
shared_ids_for: dict[str, set[str]] = {}
for route, page in re.findall(r'\{"([a-z-]+)", "([a-z]+)"\}', aliases[1]):
    shared_ids_for.setdefault(page, set()).add(route)

tsx = read(UI / "settings/settings-mascot-intro.tsx")
intro_pattern = r'"?([a-z-]+)"?:\s*\{\s*scene:\s*"([a-z]+)",\s*description:\s*"([^"]+)",?\s*\}'
shared_intros = re.findall(intro_pattern, tsx)
assert shared_intros, "读不到 settings-mascot-intro.tsx 的引导文案"
shared_pairs = {(scene, text) for _, scene, text in shared_intros}
# 只取按页面记的 `introductions`：`tabIntroductions` 的键是标签 id，会和页面 id 重名（语音的 `input` 标签）。
page_block = re.search(r"const introductions[^=]*= \{(.*?)\n\};", tsx, re.S)
assert page_block, "读不到 introductions"
untabbed = {key: (scene, text) for key, scene, text in re.findall(intro_pattern, page_block[1])}
entries = re.findall(r'\{"([a-z]+)", "([a-z]*)", \{MascotScene::([a-z]+), L"([^"]+)"\}\}', intro_header)
count = re.search(r"std::array<PageMascotIntro, (\d+)> mascot_intros", intro_header)
assert count and int(count[1]) == len(entries), "mascot_intros 的条目数读不全"
for page, tab, scene, text in entries:
    assert (scene, text) in shared_pairs, f"{page}/{tab} 的引导「{text}」({scene}) 不在 settings-mascot-intro.tsx 里"
    if not tab:
        candidates = {untabbed[route] for route in shared_ids_for.get(page, set()) if route in untabbed}
        assert (scene, text) in candidates, f"{page} 的引导不是共享页面 {sorted(shared_ids_for.get(page, set()))} 的那句"
assert not any(page == "about" for page, *_ in entries), "关于页不放页首引导"

# 侧栏图标的图源。
registry = read(UI / "settings/settings-page-registry.ts")
icon_key_for = dict(re.findall(r'id: "([a-z-]+)",\s*title: "[^"]+",\s*icon: settingsIcons\.([A-Za-z]+)', registry))
resources_ts = read(UI / "settings/app-resources.ts")
icon_block = re.search(r"export const settingsIcons = \{(.*?)\} as const;", resources_ts, re.S)
assert icon_block, "读不到 settingsIcons"
icon_file_for = dict(re.findall(r'([A-Za-z]+): new URL\("\.\./assets/([a-z-]+\.svg)"', icon_block[1]))
icons_header = read(SETTINGS / "SettingsNavIcons.h")
icons = re.findall(r'\{"([a-z]+)", "([a-z-]+\.svg)",', icons_header)
pages = re.findall(r'\{"([a-z]+)",\s*\d+,\s*PageHost::', navigation)
assert [page for page, _ in icons] == pages, f"侧栏图标与页面顺序不一致：{[p for p, _ in icons]} != {pages}"
for page, source in icons:
    assert (UI / "assets" / source).is_file(), f"{page} 的图源 {source} 不存在"
    routes = shared_ids_for.get(page, set()) | {page}
    shared = {icon_file_for[icon_key_for[route]] for route in routes if route in icon_key_for}
    assert source in shared, f"{page} 的图标 {source} 不是共享 UI 给 {sorted(routes)} 的 {sorted(shared)}"

print("windows settings art: mascots, intro copy, provider icons and sidebar icons follow the shared settings UI")
