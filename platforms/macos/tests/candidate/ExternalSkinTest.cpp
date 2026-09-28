#include "../../src/candidate/CandidateSkin.h"

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
    Require(msime::mac::IsBuiltInSkinId("fluent") && msime::mac::IsBuiltInSkinId("wechat") &&
                msime::mac::IsBuiltInSkinId("graphite") && msime::mac::IsBuiltInSkinId("willow_green") &&
                !msime::mac::IsBuiltInSkinId("niya-demo"),
            "Built-in skin ids did not match the Windows catalog.");
    Require(msime::mac::IsSafeSkinId("niya-demo") && !msime::mac::IsSafeSkinId("Fluent") &&
                !msime::mac::IsSafeSkinId("../x") && msime::mac::NormalizeSkinId("") == "willow_green" &&
                    msime::mac::NormalizeSkinId("nope!") == "fluent",
            "Skin id validation did not match the Windows catalog.");
    Require(msime::mac::BuiltInSkinEntries().size() == 4, "The built-in skin list was incomplete.");

    const auto fluentDark = msime::mac::BuiltInSkinTokens("fluent", true);
    const auto fluentLight = msime::mac::BuiltInSkinTokens("fluent", false);
    Require(fluentDark.showSelectedBar && fluentLight.showSelectedBar && fluentDark.accent.b > fluentDark.accent.r,
            "Fluent tokens lost the accent bar.");
    const auto wechatDark = msime::mac::BuiltInSkinTokens("wechat", true);
    Require(!wechatDark.showSelectedBar && wechatDark.selected.g > 0.6f && wechatDark.selectedText.r > 0.9f,
            "WeChat tokens did not use a filled green selection.");
    const auto graphiteDark = msime::mac::BuiltInSkinTokens("graphite", true);
    Require(!graphiteDark.showSelectedBar && graphiteDark.selected.a < 0.01f,
            "Graphite tokens did not keep a transparent selection.");
    const auto willowDark = msime::mac::BuiltInSkinTokens("willow_green", true);
    Require(willowDark.borderWidth == 0.0f && willowDark.radius >= 8.0f,
            "Willow green tokens did not keep the full-bleed card.");

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
base = "fluent"
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
    Require(!msime::mac::LoadSkinPackage(root, "fluent", &error),
            "A built-in id was treated as an external package.");

    const auto catalog = msime::mac::ScanSkinCatalog(root);
    Require(catalog.packages.size() == 1 && catalog.issues.size() == 1 && catalog.packages[0].id == "niya-demo",
            "Catalog scanning did not separate valid packages from issues.");

    const auto resolved = msime::mac::ResolveSkin("niya-demo", true, root);
    Require(resolved.id == "niya-demo" && resolved.tokens.accent.r > 0.8f && resolved.decorationTopDip == 88.0 &&
                resolved.decorationPath.find("character.png") != std::string::npos,
            "External skin colors were not applied on top of Fluent.");
    const auto missing = msime::mac::ResolveSkin("missing-skin", true, root);
    Require(missing.id == "fluent", "An unknown skin id did not fall back to Fluent.");

    const auto listed = msime::mac::ListSkins(root);
    Require(listed.size() == 5 && listed[0].id == "fluent" && listed.back().id == "niya-demo",
            "The settings list did not keep built-in skins ahead of external packages.");

    WriteFile(root / "wechat-based" / "skin.toml", R"toml(
schema_version = 1
id = "wechat-based"
name = "WeChat Based"
version = "1.0"
base = "wechat"
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
    Require(wechatBased.has_value() && wechatBased->base == "wechat",
            "A wechat-based skin with a valid toolbar stylesheet was rejected.");
    const auto resolvedWechat = msime::mac::ResolveSkin("wechat-based", true, root);
    Require(resolvedWechat.id == "wechat-based" && !resolvedWechat.tokens.showSelectedBar &&
                resolvedWechat.tokens.selected.g > 0.6f && resolvedWechat.tokens.accent.r > 0.9f,
            "External skin tokens did not inherit the declared base skin.");
    const auto toolbarDark = msime::mac::ToolbarSkinTokens("wechat-based", true, root);
    Require(toolbarDark.surface.r < 0.1f && toolbarDark.surface.g < 0.1f && toolbarDark.surface.b < 0.1f &&
                toolbarDark.border.a > 0.2f && toolbarDark.text.r > 0.9f && toolbarDark.accent.r > 0.9f &&
                toolbarDark.radius == 12.0f && toolbarDark.borderWidth == 2.0f && toolbarDark.pad == 7.0f &&
                toolbarDark.hover.r > 0.1f,
            "The native toolbar did not apply the supported external CSS palette.");
    const auto toolbarLight = msime::mac::ToolbarSkinTokens("wechat-based", false, root);
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
    Require(msime::mac::ResolveSkin("wechat-based", false, root, "vertical", "dark").id == "fluent" &&
                msime::mac::ResolveSkin("wechat-based", false, root, "horizontal", "light").id == "wechat-based",
            "Incompatible external skins did not fall back to Fluent.");

    // Windows reads skin.toml with toml++, and the settings page with the shared loader: literal strings, a multi-line array, an inline table, a digit separator, a unicode escape and a `#` inside a literal string are all ordinary TOML the candidate window must draw rather than fall back to Fluent.
    WriteFile(root / "full-toml" / "skin.toml", R"toml(schema_version = 1
id = 'full-toml'
name = "\u6768\u67f3 Full"
version = '1.0'
base = 'fluent'
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
    const auto resolvedFull = msime::mac::ResolveSkin("full-toml", false, root, "vertical", "light");
    Require(resolvedFull.id == "full-toml" && resolvedFull.minWidthDip == 10.0 && resolvedFull.tokens.accent.r > 0.99f &&
                resolvedFull.tokens.accent.g < 0.01f && resolvedFull.tokens.surface.r > 0.99f &&
                !resolvedFull.tokens.showSelectedBar,
            "The candidate window drew Fluent for a full TOML manifest.");
    const auto toolbarFull = msime::mac::ToolbarSkinTokens("full-toml", false, root);
    Require(toolbarFull.radius == 11.0f && toolbarFull.surface.b > 0.3f && toolbarFull.surface.r < 0.1f,
            "The toolbar did not pick up a full TOML manifest's stylesheet.");
    WriteFile(root / "full-toml" / "toolbar.css", std::string(65537, 'x'));
    const auto oversizedToolbar = msime::mac::ToolbarSkinTokens("full-toml", false, root);
    const auto fluentToolbar = msime::mac::ToolbarSkinTokens("fluent", false, root);
    Require(oversizedToolbar.radius == fluentToolbar.radius,
            "An oversized toolbar stylesheet was not ignored.");
    Require(msime::mac::ListSkins(root).size() == 7, "The settings list lost the full TOML manifest.");
    std::filesystem::remove_all(root / "full-toml");

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
    Require(msime::mac::ListSkins({}).size() == 4, "Empty root searched the working directory.");
    WriteFile(root / "oversized" / "skin.toml", std::string(65537, 'x'));
    Require(!msime::mac::LoadSkinPackage(root, "oversized"), "Oversized manifest accepted.");
    std::filesystem::remove_all(outside);
    std::filesystem::remove_all(root);
    return 0;
}
