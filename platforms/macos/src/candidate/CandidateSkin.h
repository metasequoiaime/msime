// Adapted from MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.
#pragma once

#include <filesystem>
#include <functional>
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
    // The translation line when a package gives one (the resolved `secondary` differs from `number`); none draws it like the numbers.
    std::optional<Rgba> translation;
    // The floating toolbar's separator, set only by a package's `[toolbar]` colours; none draws it in the border colour. The candidate window does not read it.
    std::optional<Rgba> divider;
};

// Corner radius of a candidate row background: selectedRadius for the highlighted row, candidateRadius for hover and every other row. The native tokens keep the two equal, so selected, pressed and hover fills share one shape.
inline float CandidateRowRadius(const SkinTokens &tokens, bool highlighted, bool first = false, bool last = false)
{
    const float radius = highlighted ? tokens.selectedRadius : tokens.candidateRadius;
    if (highlighted && (first || last)) return tokens.radius;
    return radius;
}

// The user's own 候选窗 style from the shared preferences: candidate_scale_percent and candidate_opacity_percent as factors, and candidate_corner_radius in points, none to keep the theme's or the package's card radius. The defaults leave a skin exactly as it was resolved.
struct CandidateWindowStyle
{
    double scale = 1.0;
    double opacity = 1.0;
    std::optional<double> cornerRadius;
};

struct SkinColors
{
    std::string accent;
    std::string selected;
    std::string hover;
    std::string surface;
    std::string border;
    std::string text;
    std::string number;
    // Secondary candidate text (the translation); the shared resolver passes it on as `secondary`.
    std::string translation;
    std::optional<bool> showSelectedBar;
};

// Where the decoration sits along the card's top edge; right is where every host drew it before a manifest could say otherwise.
enum class DecorationAlign
{
    left,
    center,
    right,
};

enum class BackgroundFit
{
    // Fill the card, cropping the image's overflow.
    cover,
    // Show the whole image inside the card.
    contain,
    // Scale each axis to the card independently.
    stretch,
};

// An image drawn over the card surface and under the candidates, clipped to the card outline. `image` is package-relative.
struct SkinBackground
{
    std::string image;
    BackgroundFit fit = BackgroundFit::cover;
    double opacity = 1.0;
};

// One mode of a package's floating toolbar colours, each `#RRGGBB`/`#RRGGBBAA` as the shared loader normalized it, or empty for what the theme derives.
struct SkinToolbarColors
{
    std::string background;
    std::string border;
    // The drag handle. The macOS toolbar has no separate handle (the logo is the handle and keeps the brand colours), so nothing draws this.
    std::string handle;
    std::string divider;
    std::string icon;
    std::string hover;
};

struct SkinToolbar
{
    // 0-32; none keeps the host's own radius.
    std::optional<double> cornerRadiusDip;
    SkinToolbarColors dark;
    SkinToolbarColors light;
};

// Licence metadata the manifest declares. Informational only: nothing is enforced or drawn from it.
struct SkinLicense
{
    std::string code;
    std::string assets;
    std::string source;
};

// The x of a decoration `width` wide over a card `cardWidth` wide: `pad` in from the edge it is aligned to, or centred, never left of the card. The same rule as the Windows host's candidate_decoration_left.
inline double DecorationLeft(DecorationAlign align, double cardWidth, double pad, double width)
{
    double left = cardWidth - pad - width;
    switch (align)
    {
    case DecorationAlign::left:
        left = pad;
        break;
    case DecorationAlign::center:
        left = (cardWidth - width) / 2.0;
        break;
    case DecorationAlign::right:
        break;
    }
    return left > 0.0 ? left : 0.0;
}

// Where the decoration is drawn, in top-down coordinates: x from the card's left edge, top from the window's top edge.
struct DecorationRect
{
    double x = 0.0;
    double top = 0.0;
    double width = 0.0;
    double height = 0.0;
};

// The decoration as every host draws it: the window is `band` taller than the card and that band is transparent; the image is `width` wide at its own aspect ratio, its bottom `pad` below the card's top edge so it sits over the edge, and it is drawn after the card. An image too tall for the band and the overlap is scaled down whole rather than squashed. None for an empty band, width or image.
inline std::optional<DecorationRect> DecorationPlacement(DecorationAlign align, double cardWidth, double pad, double band,
                                                         double width, double imageWidth, double imageHeight)
{
    if (!(band > 0.0) || !(width > 0.0) || !(imageWidth > 0.0) || !(imageHeight > 0.0))
        return std::nullopt;
    const double room = band + pad;
    double height = width * imageHeight / imageWidth;
    if (height > room)
    {
        width *= room / height;
        height = room;
    }
    return DecorationRect{DecorationLeft(align, cardWidth, pad, width), room - height, width, height};
}

struct SkinRect
{
    double x = 0.0;
    double y = 0.0;
    double width = 0.0;
    double height = 0.0;
};

// Where a background image of natural size `imageWidth` x `imageHeight` is drawn in `card`, and which part of the image: cover crops the centred overflow to fill the card, contain letterboxes the whole image centred inside it, stretch scales each axis to the card. Both rectangles are symmetric about the centre, so they hold for flipped and unflipped coordinates alike. None for an empty card or image.
struct SkinBackgroundRects
{
    SkinRect destination;
    SkinRect source;
};
inline std::optional<SkinBackgroundRects> BackgroundRects(BackgroundFit fit, SkinRect card, double imageWidth,
                                                          double imageHeight)
{
    if (!(card.width > 0.0) || !(card.height > 0.0) || !(imageWidth > 0.0) || !(imageHeight > 0.0))
        return std::nullopt;
    SkinBackgroundRects rects{card, {0.0, 0.0, imageWidth, imageHeight}};
    const double scaleX = card.width / imageWidth;
    const double scaleY = card.height / imageHeight;
    switch (fit)
    {
    case BackgroundFit::stretch:
        break;
    case BackgroundFit::cover:
    {
        const double scale = scaleX > scaleY ? scaleX : scaleY;
        rects.source.width = card.width / scale;
        rects.source.height = card.height / scale;
        rects.source.x = (imageWidth - rects.source.width) / 2.0;
        rects.source.y = (imageHeight - rects.source.height) / 2.0;
        break;
    }
    case BackgroundFit::contain:
    {
        const double scale = scaleX < scaleY ? scaleX : scaleY;
        rects.destination.width = imageWidth * scale;
        rects.destination.height = imageHeight * scale;
        rects.destination.x = card.x + (card.width - rects.destination.width) / 2.0;
        rects.destination.y = card.y + (card.height - rects.destination.height) / 2.0;
        break;
    }
    }
    return rects;
}

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
    // `candidate_window.corner_radius_dip` (0-32); none keeps the theme's card radius.
    std::optional<double> cornerRadiusDip;
    double decorationTopDip = 0.0;
    double decorationWidthDip = 0.0;
    // The image drawn in the decoration band, package-relative: the manifest's `decoration.image`, or else an image preview. Empty unless decorated.
    std::string decorationImage;
    DecorationAlign decorationAlign = DecorationAlign::right;
    std::optional<SkinBackground> background;
    SkinToolbar toolbar;
    std::optional<SkinLicense> license;
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
    // 浅色槽位 `custom_theme.candidate_skin`：浅色模式画的外部皮肤包，深色槽位空着时深色模式也取它；空串表示没有。
    std::string candidateSkin;
    // 深色槽位 `custom_theme.candidate_skin_dark`：深色模式画的外部皮肤包，空串表示没有。
    std::string candidateSkinDark;
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
    // The package's corner radius, when it sets one, is already in tokens.radius; ToolbarSkinTokens keeps it out of the toolbar.
    SkinTokens tokens;
    std::string decorationPath;
    double decorationTopDip = 0.0;
    double decorationWidthDip = 0.0;
    DecorationAlign decorationAlign = DecorationAlign::right;
    // Absolute path of the drawn package's background image, empty for none.
    std::string backgroundPath;
    BackgroundFit backgroundFit = BackgroundFit::cover;
    double backgroundOpacity = 1.0;
    double minWidthDip = 0.0;
};

bool IsSafeSkinId(std::string_view id);
// The native macOS candidate tokens (design-tokens-desktop §1.2) that every null slot of a resolved theme falls back to.
SkinTokens NativeCandidateTokens(bool dark);
// The selected row's foregrounds for a palette that left them to the platform, derived from the fill actually drawn (THEME_CONTRACT §5 step 6): white or near-black on an opaque fill, the number at 0.82 like the native one, and the row's own text and number colours on a translucent fill, where they are what stays readable. On the native solid accent this gives the native white.
void DeriveSelectedForegrounds(SkinTokens &tokens, bool text, bool number);
// A resolved candidate skin with the user's style laid over it, for the candidate window and its preview only: the floating toolbar and the colour wells keep the skin as resolved.
//
// The radius goes user value, then the package's corner_radius_dip (already in tokens.radius), then the native card radius, and a row is never rounder than the card it sits in. The opacity reaches the card surface, its border and the package background image and nothing else, so text, numbers and the selected fill stay as legible as the theme made them. The scale multiplies every length the skin carries, the radii, the inset and the package's decoration and minimum width, so the card keeps its proportions at any size; the hairline border keeps its width.
ResolvedSkin StyledCandidateSkin(ResolvedSkin skin, const CandidateWindowStyle &style);
// The seven global themes in picker order, read once from msime_client_theme_catalog.
const std::vector<ThemeCatalogEntry> &ThemeCatalog();
bool IsGlobalThemeId(std::string_view id);
// `system` or a built-in theme: what a custom theme may be drawn over.
bool IsThemeBaseId(std::string_view id);
std::string ThemeTitle(std::string_view id);
// 皮肤包放在哪个槽位：浅色、深色、两个都放（base 为 system），以及不知道（包不在目录里）。
enum class SkinSlot
{
    light,
    dark,
    both,
    unknown,
};
// 以 `base` 为底的皮肤包所属的槽位，即 base 那个内置主题的明暗（client-core 的 `theme::skin_appearance`）；system 和目录里没有的 id 都是 both。
SkinSlot SkinSlotOfBase(std::string_view base);
// 把皮肤包 `id`（清单 base 为 `base`）放进它所属的槽位，规则与 packages/ui/src/theme/global-theme.ts 的 `applyCandidateSkin` 相同：深色皮肤写深色槽位，并清掉旧文档放在浅色槽位里的深色皮肤；浅色皮肤写浅色槽位，深色槽位空着而原来浅色槽位里的是深色或 system 底的皮肤（`slotOf` 给出 dark 或 both）时先把它挪进深色槽位，unknown 的直接覆盖；system 底的写两个槽位。`base` 照旧写成包的 base，取色器不动。
CustomTheme ApplyCandidateSkin(CustomTheme custom, const std::string &id, const std::string &base,
                               const std::function<SkinSlot(const std::string &)> &slotOf);
// 取消使用皮肤包 `id`：只清放着它的槽位，另一个槽位不动（`removeCandidateSkin`）。取下的是最后一款皮肤时底改回 system，免得包留下的固定明暗的底在两种明暗下都生效；`id` 不在任何槽位里时原样返回。
CustomTheme RemoveCandidateSkin(CustomTheme custom, const std::string &id);
// 主题是否固定了明暗：浅色、深色两次解析给出同一个固定明暗时才算（内置主题、没设皮肤的自定义主题叠在固定明暗的底上）。两个槽位各自画自己明暗的皮肤包时两次解析的明暗不同，候选窗、悬浮工具栏和菜单要跟随宿主的明暗，不能被浅色那次解析钉住。
std::optional<bool> FixedThemeMode(const ResolvedSkin &light, const ResolvedSkin &dark);
// Resolve the candidate palette of a global theme through msime_client_resolve_theme, over NativeCandidateTokens for every null slot. This reads the package from disk, so callers resolve when the theme, the mode, the layout or the skin root changes, never while drawing. `layout` is "horizontal" or "vertical".
ResolvedSkin ResolveSkin(std::string_view globalTheme, const CustomTheme &custom, bool dark, std::string_view layout,
                         const std::filesystem::path &skinsRoot);
/// The floating toolbar's palette: the candidate palette it derives from (surface, text, hover, selected with selected text, border) at the native toolbar radius, never the package's card radius; then the drawn package's `[toolbar]` radius and colours for the resolved mode (background, border, divider, icon, hover; handle has no macOS target); then the safe color/geometry subset of the package's toolbar stylesheet when there is one, which wins over both. Unsupported CSS (layout, scripts, images, effects) is intentionally ignored because the macOS toolbar is an AppKit view rather than a WebView.
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
