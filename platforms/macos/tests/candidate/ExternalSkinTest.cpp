#include "../../src/candidate/CandidateSkin.h"

#include <cmath>
#include <filesystem>
#include <fstream>
#include <stdexcept>
#include <string>
#include <unistd.h>
#include <sys/stat.h>

namespace
{
void Require(bool condition, const char *message)
{
    if (!condition)
    {
        throw std::runtime_error(message);
    }
}

std::filesystem::path MakeTempRoot()
{
    char templatePath[] = "/tmp/metasequoia-skins-XXXXXX";
    Require(mkdtemp(templatePath) != nullptr, "Failed to create a temporary skins directory.");
    return templatePath;
}

void WriteFile(const std::filesystem::path &path, const std::string &text)
{
    std::filesystem::create_directories(path.parent_path());
    std::ofstream stream(path);
    Require(static_cast<bool>(stream), "Failed to write a skin fixture.");
    stream << text;
}
} // namespace

int main()
{
    // The built-in looks are global themes now: their ids are never package ids, and the palettes are tested in CandidateSkinTest against the shared catalog.
    Require(msime::mac::IsSafeSkinId("niya-demo") && !msime::mac::IsSafeSkinId("Fluent") &&
                !msime::mac::IsSafeSkinId("../x"),
            "Skin id validation did not match the shared catalog.");
    Require(!msime::mac::IsGlobalThemeId("niya-demo") && msime::mac::IsGlobalThemeId("shuishan") &&
                !msime::mac::IsGlobalThemeId("fluent"),
            "Package ids and theme ids overlapped.");

    const auto hex = msime::mac::ParseCssColor("#07c160");
    const auto rgba = msime::mac::ParseCssColor("rgba(224, 138, 168, 0.28)");
    Require(hex.has_value() && hex->g > 0.7f && rgba.has_value() && rgba->a > 0.2f && rgba->a < 0.3f,
            "CSS color parsing rejected valid skin colors.");
    Require(!msime::mac::ParseCssColor("url(javascript:alert(1))").has_value(),
            "CSS color parsing accepted a non-color.");
    for (const char *invalid : {"#ffzzzz", "rgba(0,0,0,2)", "rgb(256,0,0)", "rgb(-1,0,0)",
                                "rgb(0,0,0)junk", "rgb(0,0,0 junk)", "rgba(0,0,0,nan)"}) {
        Require(!msime::mac::ParseCssColor(invalid), "Malformed color was accepted.");
    }

    const std::filesystem::path root = MakeTempRoot();
    WriteFile(root / "niya-demo" / "skin.toml", R"toml(
schema_version = 1
id = "niya-demo"
name = "Niya Demo"
version = "0.1.1"
author = "Metasequoia IME contributors"
description = "demo"
base = "system"
preview = "assets/character.png"

[supports]
layouts = ["horizontal", "vertical"]
themes = ["dark", "light"]

[candidate_window]
min_width_dip = 176

[candidate_window.decoration]
top_inset_dip = 88
width_dip = 136

[candidate.dark]
accent = "#e08aa8"
selected = "rgba(224, 138, 168, 0.28)"
hover = "rgba(224, 138, 168, 0.16)"

[candidate.light]
accent = "#c45c7a"
selected = "rgba(196, 92, 122, 0.18)"
hover = "rgba(196, 92, 122, 0.10)"
surface = "#fff7fa"
border = "rgba(176, 80, 110, 0.22)"
)toml");
    WriteFile(root / "niya-demo" / "assets" / "character.png", "png");
    WriteFile(root / "broken" / "skin.toml", "schema_version = 2\nid = \"broken\"\n");

    std::string error;
    auto package = msime::mac::LoadSkinPackage(root, "niya-demo", &error);
    Require(package.has_value() && package->name == "Niya Demo" && package->decorationTopDip == 88.0 &&
                package->dark.accent == "#e08aa8",
            "A valid external skin.toml was rejected.");
    Require(!msime::mac::LoadSkinPackage(root, "broken", &error) &&
                error.find("schema_version") != std::string::npos,
            "An invalid external skin was accepted.");
    WriteFile(root / "shuishan" / "skin.toml", "schema_version = 1\nid = \"shuishan\"\n");
    Require(!msime::mac::LoadSkinPackage(root, "shuishan", &error),
            "A global theme id was treated as an external package.");
    std::filesystem::remove_all(root / "shuishan");

    const auto catalog = msime::mac::ScanSkinCatalog(root);
    Require(catalog.packages.size() == 1 && catalog.issues.size() == 1 && catalog.packages[0].id == "niya-demo",
            "Catalog scanning did not separate valid packages from issues.");

    // A package is drawn as the custom theme's candidate skin, never as a global theme of its own.
    const auto withSkin = [](std::string id, std::string base = "system") {
        msime::mac::CustomTheme custom;
        custom.base = std::move(base);
        custom.candidateSkin = std::move(id);
        return custom;
    };
    const auto resolved = msime::mac::ResolveSkin("custom", withSkin("niya-demo"), true, "horizontal", root);
    Require(resolved.id == "custom" && resolved.candidateSkin == "niya-demo" && resolved.name == "Niya Demo" &&
                !resolved.fixedDark && resolved.tokens.accent.r > 0.8f && resolved.decorationTopDip == 88.0 &&
                resolved.decorationPath.find("character.png") != std::string::npos,
            "External skin colors were not applied on top of the system tokens.");
    // The package's translucent selection over a system base cannot carry the native white: the selected word and number keep the row's own colours.
    const auto resolvedLight = msime::mac::ResolveSkin("custom", withSkin("niya-demo"), false, "horizontal", root);
    Require(resolvedLight.tokens.selected.a < 0.2f &&
                resolvedLight.tokens.selectedText.r == resolvedLight.tokens.text.r &&
                resolvedLight.tokens.selectedText.g == resolvedLight.tokens.text.g &&
                resolvedLight.tokens.selectedText.a == resolvedLight.tokens.text.a &&
                resolvedLight.tokens.selectedNumber.g == resolvedLight.tokens.number.g &&
                resolvedLight.tokens.selectedNumber.a == resolvedLight.tokens.number.a,
            "A translucent package selection kept the native white foregrounds.");
    const auto missing = msime::mac::ResolveSkin("custom", withSkin("missing-skin"), true, "horizontal", root);
    Require(missing.id == "custom" && missing.candidateSkin.empty() && missing.decorationPath.empty(),
            "A missing package was drawn.");
    Require(msime::mac::ResolveSkin("system", withSkin("niya-demo"), true, "horizontal", root).candidateSkin.empty(),
            "A package was drawn while another global theme was selected.");

    const auto listed = msime::mac::ListSkins(root);
    Require(listed.size() == 1 && listed[0].id == "niya-demo" && !listed[0].builtin,
            "The settings list did not list only the external packages.");

    WriteFile(root / "night-based" / "skin.toml", R"toml(
schema_version = 1
id = "night-based"
name = "Night Based"
version = "1.0"
base = "night"

[supports]
layouts = ["horizontal", "vertical"]
themes = ["dark", "light"]

[candidate_window]
min_width_dip = 0

[candidate_window.decoration]
top_inset_dip = 0
width_dip = 0

[candidate.dark]
accent = "#ff0000"
)toml");
    auto nightBased = msime::mac::LoadSkinPackage(root, "night-based", &error);
    Require(nightBased.has_value() && nightBased->base == "night", "A night-based skin was rejected.");
    // 清单的 base 胜过存下的底色，并决定包画在哪种明暗下：深色底的包只在深色模式画，按那个底的深色配色。
    const auto resolvedNight = msime::mac::ResolveSkin("custom", withSkin("night-based"), true, "horizontal", root);
    Require(resolvedNight.candidateSkin == "night-based" && resolvedNight.fixedDark == true && resolvedNight.dark &&
                !resolvedNight.tokens.showSelectedBar && resolvedNight.tokens.accent.r > 0.9f &&
                resolvedNight.tokens.selected.r > 0.9f && resolvedNight.tokens.selected.a < 0.2f &&
                resolvedNight.tokens.surface.b > resolvedNight.tokens.surface.r,
            "External skin tokens did not inherit the declared base theme.");
    // 浅色模式不画深色底的包：皮肤留下的 night 底也不属于浅色，按 system 画，主题因此不再固定明暗。
    const auto nightInLight = msime::mac::ResolveSkin("custom", withSkin("night-based", "night"), false, "horizontal", root);
    Require(nightInLight.candidateSkin.empty() && !nightInLight.fixedDark && !nightInLight.dark,
            "A dark-based package was drawn in light mode.");

    // 浅色、深色两个槽位：深色模式取 candidate_skin_dark，浅色模式取 candidate_skin；解析请求必须把深色槽位带给共享解析器。
    WriteFile(root / "paper-based" / "skin.toml", R"toml(
schema_version = 1
id = "paper-based"
name = "Paper Based"
version = "1.0"
base = "paper"

[supports]
layouts = ["horizontal", "vertical"]
themes = ["light"]

[candidate_window]
min_width_dip = 0

[candidate.light]
accent = "#00ff00"
)toml");
    Require(msime::mac::LoadSkinPackage(root, "paper-based", &error).has_value(), "A paper-based skin was rejected.");
    msime::mac::CustomTheme slots = withSkin("paper-based", "night");
    slots.candidateSkinDark = "night-based";
    const auto slotLight = msime::mac::ResolveSkin("custom", slots, false, "horizontal", root);
    const auto slotDark = msime::mac::ResolveSkin("custom", slots, true, "horizontal", root);
    Require(slotLight.candidateSkin == "paper-based" && slotLight.fixedDark == false && slotLight.tokens.accent.g > 0.9f,
            "The light slot was not drawn in light mode.");
    Require(slotDark.candidateSkin == "night-based" && slotDark.fixedDark == true && slotDark.tokens.accent.r > 0.9f,
            "The dark slot was not sent with the resolve request or not drawn in dark mode.");
    // 两个槽位各画各自明暗的包时，主题不固定明暗，跟随宿主；内置主题和没设皮肤的自定义主题仍固定。
    Require(!msime::mac::FixedThemeMode(slotLight, slotDark), "Two slots of different modes pinned one mode.");
    Require(msime::mac::FixedThemeMode(msime::mac::ResolveSkin("night", {}, false, "horizontal", root),
                                       msime::mac::ResolveSkin("night", {}, true, "horizontal", root)) == true &&
                msime::mac::FixedThemeMode(msime::mac::ResolveSkin("custom", withSkin("", "paper"), false, "horizontal", root),
                                           msime::mac::ResolveSkin("custom", withSkin("", "paper"), true, "horizontal", root)) == false &&
                !msime::mac::FixedThemeMode(msime::mac::ResolveSkin("system", {}, false, "horizontal", root),
                                            msime::mac::ResolveSkin("system", {}, true, "horizontal", root)),
            "A theme with one mode of its own was not reported as fixed.");
    // 只有一个槽位的旧文档：深色槽位空着，深色模式回落到 candidate_skin。
    Require(msime::mac::ResolveSkin("custom", withSkin("night-based", "night"), true, "horizontal", root).candidateSkin == "night-based",
            "Dark mode did not fall back to the light slot.");
    // 不安全的深色槽位 id 不进请求，请求也不会因此整个失败。
    msime::mac::CustomTheme unsafeDark = withSkin("paper-based", "paper");
    unsafeDark.candidateSkinDark = "../escape";
    Require(msime::mac::ResolveSkin("custom", unsafeDark, false, "horizontal", root).candidateSkin == "paper-based",
            "An unsafe dark slot broke the resolve request.");

    // 槽位归属与应用、取消的规则，用例与 apps/desktop/tests/candidate/candidate-skin-slots.test.ts 相同。
    Require(msime::mac::SkinSlotOfBase("paper") == msime::mac::SkinSlot::light &&
                msime::mac::SkinSlotOfBase("light") == msime::mac::SkinSlot::light &&
                msime::mac::SkinSlotOfBase("night") == msime::mac::SkinSlot::dark &&
                msime::mac::SkinSlotOfBase("ink") == msime::mac::SkinSlot::dark &&
                msime::mac::SkinSlotOfBase("shuishan") == msime::mac::SkinSlot::dark &&
                msime::mac::SkinSlotOfBase("system") == msime::mac::SkinSlot::both,
            "A skin was not put in the slot of its base.");
    const auto slotOf = [](const std::string &id) {
        if (id == "sakura" || id == "paper-notes") return msime::mac::SkinSlot::light;
        if (id == "dusk" || id == "starry") return msime::mac::SkinSlot::dark;
        if (id == "mist") return msime::mac::SkinSlot::both;
        return msime::mac::SkinSlot::unknown;
    };
    msime::mac::CustomTheme start;
    start.candidateSkinDark = "dusk";
    start.candidateColors.text = "#123456";
    const auto light = msime::mac::ApplyCandidateSkin(start, "sakura", "paper", slotOf);
    Require(light.base == "paper" && light.candidateSkin == "sakura" && light.candidateSkinDark == "dusk" &&
                light.candidateColors.text == "#123456",
            "A light skin did not fill only the light slot.");
    const auto dark = msime::mac::ApplyCandidateSkin(light, "starry", "night", slotOf);
    Require(dark.base == "night" && dark.candidateSkin == "sakura" && dark.candidateSkinDark == "starry",
            "A dark skin did not fill only the dark slot.");
    const auto both = msime::mac::ApplyCandidateSkin(dark, "mist", "system", slotOf);
    Require(both.base == "system" && both.candidateSkin == "mist" && both.candidateSkinDark == "mist",
            "A system skin did not fill both slots.");
    // 旧文档只有一款深色皮肤，存在 candidate_skin 里：应用浅色皮肤时把它挪进深色槽位；原来是浅色皮肤时不挪。
    const auto legacy = msime::mac::ApplyCandidateSkin(withSkin("starry", "night"), "sakura", "paper", slotOf);
    Require(legacy.candidateSkin == "sakura" && legacy.candidateSkinDark == "starry",
            "A legacy dark skin was lost when a light skin was applied.");
    const auto replaced = msime::mac::ApplyCandidateSkin(withSkin("paper-notes"), "sakura", "paper", slotOf);
    Require(replaced.candidateSkin == "sakura" && replaced.candidateSkinDark.empty(),
            "A replaced light skin was moved into the dark slot.");
    // 不知道属于哪个槽位的旧皮肤（包已不在目录里）直接覆盖，不挪进深色槽位；system 底的旧皮肤能画在深色模式，照样挪过去。
    const auto unknown = msime::mac::ApplyCandidateSkin(withSkin("gone"), "sakura", "paper", slotOf);
    Require(unknown.candidateSkin == "sakura" && unknown.candidateSkinDark.empty(),
            "An unknown legacy skin was moved into the dark slot.");
    Require(msime::mac::ApplyCandidateSkin(withSkin("mist"), "sakura", "paper", slotOf).candidateSkinDark == "mist",
            "A legacy system-based skin was not kept for dark mode.");
    // 新的深色皮肤取代旧文档放在 candidate_skin 里的深色皮肤。
    const auto newDark = msime::mac::ApplyCandidateSkin(withSkin("starry", "night"), "dusk", "night", slotOf);
    Require(newDark.candidateSkin.empty() && newDark.candidateSkinDark == "dusk",
            "A new dark skin did not replace the legacy dark skin.");
    msime::mac::CustomTheme twoSkins = withSkin("sakura");
    twoSkins.candidateSkinDark = "dusk";
    const auto removedDark = msime::mac::RemoveCandidateSkin(twoSkins, "dusk");
    Require(removedDark.candidateSkin == "sakura" && removedDark.candidateSkinDark.empty(),
            "Removing a skin cleared the other slot.");
    Require(removedDark.base == twoSkins.base, "Removing one of two skins changed the base.");
    msime::mac::CustomTheme mist = withSkin("mist");
    mist.candidateSkinDark = "mist";
    const auto removedMist = msime::mac::RemoveCandidateSkin(mist, "mist");
    Require(removedMist.candidateSkin.empty() && removedMist.candidateSkinDark.empty(),
            "Removing a skin in both slots left one of them.");
    // 先用浅色皮肤再用深色皮肤，底是 night；两款都取下后底回到 system，浅色模式不会被 night 钉成深色。
    const auto lightThenDark =
        msime::mac::ApplyCandidateSkin(msime::mac::ApplyCandidateSkin({}, "sakura", "paper", slotOf), "dusk", "night", slotOf);
    Require(lightThenDark.base == "night", "Applying a dark skin did not take its base.");
    const auto darkLeft = msime::mac::RemoveCandidateSkin(lightThenDark, "sakura");
    Require(darkLeft.base == "night" && darkLeft.candidateSkinDark == "dusk", "Removing the light skin changed the base.");
    const auto noneLeft = msime::mac::RemoveCandidateSkin(darkLeft, "dusk");
    Require(noneLeft.base == "system" && noneLeft.candidateSkin.empty() && noneLeft.candidateSkinDark.empty(),
            "Removing the last skin kept a base that fixes both modes.");
    msime::mac::CustomTheme picked;
    picked.base = "ink";
    Require(msime::mac::RemoveCandidateSkin(picked, "dusk").base == "ink",
            "Removing a skin that is not in use changed the base.");
    std::filesystem::remove_all(root / "paper-based");

    WriteFile(root / "wechat-based" / "skin.toml", R"toml(
schema_version = 1
id = "wechat-based"
name = "WeChat Based"
version = "1.0"
base = "system"
toolbar_stylesheet = "toolbar.css"

[supports]
layouts = ["horizontal"]
themes = ["dark", "light"]

[candidate_window]
min_width_dip = 0

[candidate_window.decoration]
top_inset_dip = 0
width_dip = 0

[candidate.dark]
accent = "#ff0000"
)toml");
    WriteFile(root / "wechat-based" / "toolbar.css", R"css(
:root {
  --toolbar-bg: #121212;
  --toolbar-border: rgba(255, 255, 255, .25);
  --toolbar-text: #f8f8f8;
  --toolbar-accent: #ff8800;
  --toolbar-radius: 12px;
  --toolbar-border-width: 2px;
  --toolbar-padding: 7px;
}
.toolbar:hover { background-color: #221100; }
html[data-theme="light"] {
  --toolbar-bg: #fffaf0;
  --toolbar-text: #302000;
}
)css");
    auto wechatBased = msime::mac::LoadSkinPackage(root, "wechat-based", &error);
    Require(wechatBased.has_value() && wechatBased->base == "system",
            "A skin with a valid toolbar stylesheet was rejected.");
    const auto resolvedWechat = msime::mac::ResolveSkin("custom", withSkin("wechat-based"), true, "horizontal", root);
    // Over `system` a null `selected` is the native solid fill of the drawn accent.
    Require(resolvedWechat.candidateSkin == "wechat-based" && !resolvedWechat.fixedDark &&
                !resolvedWechat.tokens.showSelectedBar && resolvedWechat.tokens.accent.r > 0.9f &&
                resolvedWechat.tokens.selected.r > 0.9f && resolvedWechat.tokens.selected.a > 0.99f,
            "External skin tokens did not sit on the native tokens.");
    const auto toolbarDark = msime::mac::ToolbarSkinTokens(resolvedWechat, root);
    Require(toolbarDark.surface.r < 0.1f && toolbarDark.surface.g < 0.1f && toolbarDark.surface.b < 0.1f &&
                toolbarDark.border.a > 0.2f && toolbarDark.text.r > 0.9f && toolbarDark.accent.r > 0.9f &&
                toolbarDark.radius == 12.0f && toolbarDark.borderWidth == 2.0f && toolbarDark.pad == 7.0f &&
                toolbarDark.hover.r > 0.1f,
            "The native toolbar did not apply the supported external CSS palette.");
    const auto toolbarLight = msime::mac::ToolbarSkinTokens(
        msime::mac::ResolveSkin("custom", withSkin("wechat-based"), false, "horizontal", root), root);
    Require(toolbarLight.surface.r > 0.9f && toolbarLight.surface.g > 0.9f && toolbarLight.text.r < 0.3f,
            "Theme-scoped toolbar CSS did not apply to the light palette.");
    Require(msime::mac::SupportsSkin(*wechatBased, "horizontal", "dark") &&
                !msime::mac::SupportsSkin(*wechatBased, "vertical", "dark"),
            "External skin layout and theme support was not enforced.");
    std::filesystem::remove(root / "wechat-based" / "toolbar.css");
    Require(!msime::mac::LoadSkinPackage(root, "wechat-based", &error) &&
                error.find("toolbar_stylesheet") != std::string::npos,
            "A skin with a missing toolbar stylesheet was accepted.");
    WriteFile(root / "wechat-based" / "toolbar.css", ".toolbar { color: red; }\n");
    Require(msime::mac::ResolveSkin("custom", withSkin("wechat-based"), false, "vertical", root).candidateSkin.empty() &&
                msime::mac::ResolveSkin("custom", withSkin("wechat-based"), false, "horizontal", root).candidateSkin ==
                    "wechat-based",
            "An external skin was drawn in a layout it does not support.");

    // Windows reads skin.toml with toml++, and the settings page with the shared loader: literal strings, a multi-line array, an inline table, a digit separator, a unicode escape and a `#` inside a literal string are all ordinary TOML the candidate window must draw rather than drop.
    WriteFile(root / "full-toml" / "skin.toml", R"toml(schema_version = 1
id = 'full-toml'
name = "\u6768\u67f3 Full"
version = '1.0'
base = 'system'
description = 'hash # inside a literal'
toolbar_stylesheet = 'toolbar.css'

[supports]
layouts = [
  'horizontal', # trailing comment
  'vertical',
]
themes = ['dark', 'light']

[candidate_window]
min_width_dip = 1_0
decoration = { top_inset_dip = 0, width_dip = 0 }

[candidate.light]
accent = '#ff0000'
surface = '#fff7fa'
show_selected_bar = false
)toml");
    WriteFile(root / "full-toml" / "toolbar.css", ":root { --toolbar-bg: #123456; --toolbar-radius: 11px; }\n");
    auto full = msime::mac::LoadSkinPackage(root, "full-toml", &error);
    Require(full.has_value() && full->name == "\u6768\u67f3 Full" && full->description == "hash # inside a literal" &&
                full->layouts.size() == 2 && full->minWidthDip == 10.0 && full->decorationTopDip == 0.0 &&
                full->light.accent == "#ff0000" && full->light.showSelectedBar == false &&
                !full->dark.showSelectedBar.has_value() && full->toolbarStylesheet == "toolbar.css",
            "A full TOML 1.0 manifest was rejected or misread.");
    const auto resolvedFull = msime::mac::ResolveSkin("custom", withSkin("full-toml"), false, "vertical", root);
    Require(resolvedFull.candidateSkin == "full-toml" && resolvedFull.minWidthDip == 10.0 && resolvedFull.tokens.accent.r > 0.99f &&
                resolvedFull.tokens.accent.g < 0.01f && resolvedFull.tokens.surface.r > 0.99f &&
                !resolvedFull.tokens.showSelectedBar,
            "The candidate window dropped a full TOML manifest.");
    const auto toolbarFull = msime::mac::ToolbarSkinTokens(resolvedFull, root);
    Require(toolbarFull.radius == 11.0f && toolbarFull.surface.b > 0.3f && toolbarFull.surface.r < 0.1f,
            "The toolbar did not pick up a full TOML manifest's stylesheet.");
    WriteFile(root / "full-toml" / "toolbar.css", std::string(65537, 'x'));
    const auto oversizedToolbar = msime::mac::ToolbarSkinTokens(resolvedFull, root);
    Require(oversizedToolbar.radius == resolvedFull.tokens.radius,
            "An oversized toolbar stylesheet was not ignored.");
    Require(msime::mac::ListSkins(root).size() == 4, "The settings list lost the full TOML manifest.");
    std::filesystem::remove_all(root / "full-toml");

    // The msime-skins keys: decoration.image over the preview, its alignment, the card radius, a background, a translation colour, and the toolbar's own radius and colours under its stylesheet.
    WriteFile(root / "styled" / "skin.toml", R"toml(
schema_version = 1
id = "styled"
name = "Styled"
version = "1.0"
base = "system"
preview = "assets/preview.png"
toolbar_stylesheet = "toolbar.css"

[supports]
layouts = ["horizontal", "vertical"]
themes = ["dark", "light"]

[candidate_window]
min_width_dip = 176
corner_radius_dip = 20

[candidate_window.decoration]
image = "assets/character.png"
top_inset_dip = 104
width_dip = 96
align = "left"

[candidate_window.background]
image = "assets/background.png"
fit = "contain"
opacity = 0.35

[candidate.dark]
number = "#E0C07A"
translation = "#9FB4E0"

[toolbar.dark]
background = "#141B33"
border = "#5B9BFF"
handle = "#5B9BFF"
divider = "#224466"
icon = "#E3EAFF"
hover = "#303030"

[toolbar.light]
background = "#F4F8FF"

[license]
code = "MIT"
assets = "CC-BY-4.0"
source = "synthetic"
)toml");
    WriteFile(root / "styled" / "assets" / "preview.png", "png");
    WriteFile(root / "styled" / "assets" / "character.png", "png");
    WriteFile(root / "styled" / "assets" / "background.png", "png");
    WriteFile(root / "styled" / "toolbar.css", "html[data-theme=\"light\"] { --toolbar-bg: #102030; }\n");
    auto styled = msime::mac::LoadSkinPackage(root, "styled", &error);
    Require(styled.has_value() && styled->cornerRadiusDip == 20.0 && styled->decorationImage == "assets/character.png" &&
                styled->decorationAlign == msime::mac::DecorationAlign::left && styled->background &&
                styled->background->image == "assets/background.png" &&
                styled->background->fit == msime::mac::BackgroundFit::contain && styled->background->opacity == 0.35 &&
                !styled->toolbar.cornerRadiusDip && styled->toolbar.dark.background == "#141B33" &&
                styled->toolbar.dark.handle == "#5B9BFF" && styled->dark.translation == "#9FB4E0" && styled->license &&
                styled->license->code == "MIT" && styled->license->source == "synthetic",
            "The msime-skins manifest keys were not read.");
    const auto styledDark = msime::mac::ResolveSkin("custom", withSkin("styled"), true, "horizontal", root);
    Require(styledDark.candidateSkin == "styled" && styledDark.tokens.radius == 20.0f &&
                styledDark.decorationPath.find("character.png") != std::string::npos &&
                styledDark.decorationAlign == msime::mac::DecorationAlign::left &&
                styledDark.backgroundPath.find("background.png") != std::string::npos &&
                styledDark.backgroundFit == msime::mac::BackgroundFit::contain && styledDark.backgroundOpacity == 0.35,
            "The decoration image, alignment, card radius or background was not resolved.");
    Require(styledDark.tokens.translation && std::abs(styledDark.tokens.translation->r - 0x9F / 255.0f) < 0.002f &&
                std::abs(styledDark.tokens.translation->b - 0xE0 / 255.0f) < 0.002f,
            "The package translation colour was not resolved.");
    const auto styledLight = msime::mac::ResolveSkin("custom", withSkin("styled"), false, "horizontal", root);
    Require(!styledLight.tokens.translation, "A translation colour appeared where secondary equals number.");
    // The card radius stays out of the toolbar; the manifest's toolbar colours apply in their own mode.
    const auto styledToolbarDark = msime::mac::ToolbarSkinTokens(styledDark, root);
    Require(styledToolbarDark.radius == msime::mac::NativeCandidateTokens(true).radius &&
                std::abs(styledToolbarDark.surface.r - 0x14 / 255.0f) < 0.002f &&
                std::abs(styledToolbarDark.border.b - 1.0f) < 0.002f && std::abs(styledToolbarDark.text.r - 0xE3 / 255.0f) < 0.002f &&
                std::abs(styledToolbarDark.hover.r - 0x30 / 255.0f) < 0.002f && styledToolbarDark.divider &&
                std::abs(styledToolbarDark.divider->b - 0x66 / 255.0f) < 0.002f && !styledToolbarDark.translation,
            "The manifest toolbar palette was not applied, or the card radius leaked into the toolbar.");
    // The stylesheet still wins over the manifest's toolbar colours.
    const auto styledToolbarLight = msime::mac::ToolbarSkinTokens(styledLight, root);
    Require(std::abs(styledToolbarLight.surface.r - 0x10 / 255.0f) < 0.002f && std::abs(styledToolbarLight.surface.b - 0x30 / 255.0f) < 0.002f &&
                !styledToolbarLight.divider,
            "The toolbar stylesheet did not win over the manifest toolbar colours.");
    // A toolbar radius comes from [toolbar], and the stylesheet's radius still wins over it; center alignment and a preview fallback decoration.
    WriteFile(root / "styled" / "skin.toml", R"toml(
schema_version = 1
id = "styled"
name = "Styled"
version = "1.0"
base = "system"
preview = "assets/preview.png"
toolbar_stylesheet = "toolbar.css"

[supports]
layouts = ["horizontal", "vertical"]
themes = ["dark", "light"]

[candidate_window.decoration]
top_inset_dip = 40
width_dip = 60
align = "center"

[toolbar]
corner_radius_dip = 4
)toml");
    WriteFile(root / "styled" / "toolbar.css", "html[data-theme=\"light\"] { --toolbar-radius: 14px; }\n");
    const auto centered = msime::mac::ResolveSkin("custom", withSkin("styled"), true, "horizontal", root);
    Require(centered.decorationAlign == msime::mac::DecorationAlign::center &&
                centered.decorationPath.find("preview.png") != std::string::npos && centered.backgroundPath.empty() &&
                centered.tokens.radius == msime::mac::NativeCandidateTokens(true).radius,
            "A preview decoration, center alignment or the host radius was not kept.");
    Require(msime::mac::ToolbarSkinTokens(centered, root).radius == 4.0f &&
                msime::mac::ToolbarSkinTokens(msime::mac::ResolveSkin("custom", withSkin("styled"), false, "horizontal", root), root).radius == 14.0f,
            "The toolbar radius did not layer manifest under stylesheet.");
    Require(msime::mac::DecorationLeft(msime::mac::DecorationAlign::left, 200, 8, 60) == 8.0 &&
                msime::mac::DecorationLeft(msime::mac::DecorationAlign::center, 200, 8, 60) == 70.0 &&
                msime::mac::DecorationLeft(msime::mac::DecorationAlign::right, 200, 8, 60) == 132.0 &&
                msime::mac::DecorationLeft(msime::mac::DecorationAlign::right, 40, 8, 60) == 0.0,
            "Decoration alignment placed the image wrongly.");
    // A 60 x 90 image in a 100 band with an 8 overlap: its own aspect, bottom 8 below the band. A taller one shrinks whole to the 108 of room instead of being squashed.
    const auto fits = msime::mac::DecorationPlacement(msime::mac::DecorationAlign::right, 200, 8, 100, 60, 120, 180);
    const auto tall = msime::mac::DecorationPlacement(msime::mac::DecorationAlign::left, 200, 8, 100, 60, 60, 216);
    Require(fits && fits->x == 132.0 && fits->width == 60.0 && fits->height == 90.0 && fits->top == 18.0 && tall &&
                tall->height == 108.0 && tall->width == 30.0 && tall->top == 0.0 && tall->x == 8.0 &&
                !msime::mac::DecorationPlacement(msime::mac::DecorationAlign::right, 200, 8, 0, 60, 120, 180),
            "The decoration was not placed on the card's top edge at its own aspect ratio.");
    const auto cover = msime::mac::BackgroundRects(msime::mac::BackgroundFit::cover, {0, 0, 200, 100}, 100, 100);
    const auto contain = msime::mac::BackgroundRects(msime::mac::BackgroundFit::contain, {10, 0, 200, 100}, 100, 100);
    const auto stretch = msime::mac::BackgroundRects(msime::mac::BackgroundFit::stretch, {0, 0, 200, 100}, 100, 100);
    Require(cover && cover->destination.width == 200 && cover->source.x == 0 && cover->source.y == 25 &&
                cover->source.width == 100 && cover->source.height == 50 && contain &&
                contain->destination.x == 60 && contain->destination.width == 100 && contain->destination.height == 100 &&
                stretch && stretch->destination.width == 200 && stretch->source.width == 100 &&
                !msime::mac::BackgroundRects(msime::mac::BackgroundFit::cover, {0, 0, 0, 100}, 100, 100),
            "Background fit placed the image wrongly.");
    WriteFile(root / "styled" / "skin.toml", R"toml(
schema_version = 1
id = "styled"
name = "Styled"
version = "1.0"
base = "system"

[supports]
layouts = ["horizontal"]
themes = ["dark"]

[candidate_window]
corner_radius_dip = 40
)toml");
    Require(!msime::mac::LoadSkinPackage(root, "styled", &error) && error.find("corner_radius_dip") != std::string::npos,
            "An out-of-range corner radius was accepted or its reason lost.");
    std::filesystem::remove_all(root / "styled");

    // Manifest/resource paths cannot escape through a package or resource symlink.
    const auto outside = MakeTempRoot();
    WriteFile(outside / "skin.toml", "schema_version = 1");
    std::filesystem::create_directory_symlink(outside, root / "escape");
    Require(!msime::mac::LoadSkinPackage(root, "escape"), "Package symlink escaped root.");
    std::filesystem::create_directory(root / "linked-manifest");
    std::filesystem::create_symlink(outside / "skin.toml", root / "linked-manifest" / "skin.toml");
    Require(!msime::mac::LoadSkinPackage(root, "linked-manifest"), "Manifest symlink escaped package.");
    std::filesystem::remove(root / "niya-demo" / "assets" / "character.png");
    std::filesystem::create_symlink(outside / "skin.toml", root / "niya-demo" / "assets" / "character.png");
    Require(!msime::mac::LoadSkinPackage(root, "niya-demo"), "Image symlink escaped package.");
    std::filesystem::create_directory(root / "pipe");
    Require(mkfifo((root / "pipe" / "skin.toml").c_str(), 0600) == 0, "FIFO fixture failed.");
    Require(!msime::mac::LoadSkinPackage(root, "pipe"), "Nonregular manifest accepted.");
    Require(msime::mac::ListSkins({}).empty(), "Empty root searched the working directory.");
    WriteFile(root / "oversized" / "skin.toml", std::string(65537, 'x'));
    Require(!msime::mac::LoadSkinPackage(root, "oversized"), "Oversized manifest accepted.");
    std::filesystem::remove_all(outside);
    std::filesystem::remove_all(root);
    return 0;
}
