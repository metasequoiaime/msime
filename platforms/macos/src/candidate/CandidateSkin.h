// Adapted from MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.
#pragma once

#include <filesystem>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace msime::mac
{
struct Rgba
{
    float r = 0.0f;
    float g = 0.0f;
    float b = 0.0f;
    float a = 1.0f;
};

struct SkinTokens
{
    Rgba surface;
    Rgba border;
    Rgba text;
    Rgba number;
    Rgba selected;
    Rgba selectedHover;
    Rgba selectedText;
    Rgba selectedNumber;
    Rgba hover;
    Rgba accent;
    // The macOS candidate geometry of design-tokens-desktop §1.2: a 10pt card, 6pt rows, a 1pt hairline and no selection bar. Themes only recolour; a package or a toolbar stylesheet may still change these.
    float radius = 10.0f;
    float candidateRadius = 6.0f;
    float selectedRadius = 6.0f;
    float borderWidth = 1.0f;
    float pad = 6.0f;
    bool showSelectedBar = false;
};

// Corner radius of a candidate row background: selectedRadius for the highlighted row, candidateRadius for hover and every other row. The native tokens keep the two equal, so selected, pressed and hover fills share one shape.
inline float CandidateRowRadius(const SkinTokens &tokens, bool highlighted, bool first = false, bool last = false)
{
    const float radius = highlighted ? tokens.selectedRadius : tokens.candidateRadius;
    if (highlighted && (first || last)) return tokens.radius;
    return radius;
}

struct SkinColors
{
    std::string accent;
    std::string selected;
    std::string hover;
    std::string surface;
    std::string border;
    std::string text;
    std::string number;
    std::optional<bool> showSelectedBar;
};

struct SkinPackage
{
    std::string id;
    std::string name;
    std::string version;
    std::string author;
    std::string description;
    // The global theme the package is drawn over: `system` or a built-in theme id, as the shared loader validated it.
    std::string base = "system";
    std::string toolbarStylesheet;
    std::string preview;
    std::vector<std::string> layouts;
    std::vector<std::string> themes;
    double minWidthDip = 0.0;
    double decorationTopDip = 0.0;
    double decorationWidthDip = 0.0;
    SkinColors dark;
    SkinColors light;
};

struct SkinIssue
{
    std::string folder;
    std::string reason;
};

struct SkinCatalog
{
    std::vector<SkinPackage> packages;
    std::vector<SkinIssue> issues;
};

struct SkinListEntry
{
    std::string id;
    std::string name;
    bool builtin = false;
};

// What `custom` is made of (crates/client-core CustomTheme, minus the keyboard design, which macOS does not draw).
struct CustomTheme
{
    // `system` or a built-in theme id, never `custom`.
    std::string base = "system";
    // The external package id the custom theme draws, empty for none.
    std::string candidateSkin;
    // The seven candidate pickers, each `#RRGGBB` or empty for unset. showSelectedBar is unused.
    SkinColors candidateColors;
};

// One entry of msime_client_theme_catalog: the theme picker's order, ids, titles and swatches. `appearance` is "light", "dark" or empty for system and custom, which also have no preview.
struct ThemeCatalogEntry
{
    std::string id;
    std::string title;
    std::string appearance;
    bool hasPreview = false;
    Rgba previewBackground;
    Rgba previewPanel;
    Rgba previewAccent;
    Rgba previewText;
};

struct ResolvedSkin
{
    // The global theme drawn.
    std::string id;
    // The theme's title, or the package's name while a package is drawn.
    std::string name;
    // The package whose colours were drawn, empty when none was: the one signal for its decoration and minimum width.
    std::string candidateSkin;
    // Set when the theme fixes the mode (a built-in theme, or a custom theme over one); the surfaces are then drawn in it whatever the system mode is.
    std::optional<bool> fixedDark;
    // The mode the tokens were resolved for.
    bool dark = false;
    SkinTokens tokens;
    std::string decorationPath;
    double decorationTopDip = 0.0;
    double decorationWidthDip = 0.0;
    double minWidthDip = 0.0;
};

bool IsSafeSkinId(std::string_view id);
// The native macOS candidate tokens (design-tokens-desktop §1.2) that every null slot of a resolved theme falls back to.
SkinTokens NativeCandidateTokens(bool dark);
// The selected row's foregrounds for a palette that left them to the platform, derived from the fill actually drawn (THEME_CONTRACT §5 step 6): white or near-black on an opaque fill, the number at 0.82 like the native one, and the row's own text and number colours on a translucent fill, where they are what stays readable. On the native solid accent this gives the native white.
void DeriveSelectedForegrounds(SkinTokens &tokens, bool text, bool number);
// The seven global themes in picker order, read once from msime_client_theme_catalog.
const std::vector<ThemeCatalogEntry> &ThemeCatalog();
bool IsGlobalThemeId(std::string_view id);
// `system` or a built-in theme: what a custom theme may be drawn over.
bool IsThemeBaseId(std::string_view id);
std::string ThemeTitle(std::string_view id);
// Resolve the candidate palette of a global theme through msime_client_resolve_theme, over NativeCandidateTokens for every null slot. This reads the package from disk, so callers resolve when the theme, the mode, the layout or the skin root changes, never while drawing. `layout` is "horizontal" or "vertical".
ResolvedSkin ResolveSkin(std::string_view globalTheme, const CustomTheme &custom, bool dark, std::string_view layout,
                         const std::filesystem::path &skinsRoot);
/// The floating toolbar's palette: the candidate palette it derives from (surface, text, hover, selected with selected text, border), then the safe color/geometry subset of the drawn package's toolbar stylesheet when there is one. Unsupported CSS (layout, scripts, images, effects) is intentionally ignored because the macOS toolbar is an AppKit view rather than a WebView.
SkinTokens ToolbarSkinTokens(const ResolvedSkin &skin, const std::filesystem::path &skinsRoot);
std::optional<Rgba> ParseCssColor(std::string_view text);
std::optional<SkinPackage> LoadSkinPackage(const std::filesystem::path &skinsRoot, const std::string &id,
                                           std::string *error = nullptr);
bool SupportsSkin(const SkinPackage &package, std::string_view layout, std::string_view theme);
SkinCatalog ScanSkinCatalog(const std::filesystem::path &skinsRoot);
std::vector<SkinListEntry> ListSkins(const std::filesystem::path &skinsRoot);
std::filesystem::path DefaultSkinsRoot();
void SetDefaultSkinsRoot(std::filesystem::path root);
} // namespace msime::mac

namespace metasequoia
{
namespace mac = ::msime::mac;
}
