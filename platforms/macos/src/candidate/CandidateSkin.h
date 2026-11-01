// Adapted from MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.
#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <filesystem>
#include <functional>
#include <optional>
#include <string>
#include <string_view>
#include <vector>
#include "../../../../shared/contracts/candidate_decoration_overlay.h"

#include "../../../../shared/candidate/CandidateDecoration.h"

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
    std::optional<Rgba> preeditText;
    std::optional<Rgba> preeditCaret;
    std::optional<Rgba> selectedBar;
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
    // macOS 候选窗的几何，来自 design-tokens-desktop §1.2：10pt 卡片圆角、6pt 行圆角、1pt 细边框、不画选中竖条。内边距原为 6pt，#7974 收紧到 4pt：10 − 4 = 6，选中高亮与卡片同心，窗口也更紧凑。主题只改颜色；皮肤包或工具栏样式表仍可以改这些值。
    float radius = 10.0f;
    float candidateRadius = 6.0f;
    float selectedRadius = 6.0f;
    float borderWidth = 1.0f;
    float pad = 4.0f;
    std::optional<double> paddingHorizontal;
    std::optional<double> paddingVertical;
    std::optional<double> itemPaddingHorizontal;
    std::optional<double> itemPaddingVertical;
    std::optional<double> itemSpacing;
    bool explicitItemRadius = false;
    bool showSelectedBar = false;
    // The translation line when a package gives one (the resolved `secondary` differs from `number`); none draws it like the numbers.
    std::optional<Rgba> translation;
    // 选中行的释义行（连同它下面的读音行），只在有 `translation` 时才有：选中底色不透明时按底色挑白或近黑、0.82 不透明度，和选中序号一样；底色半透明时就是 `translation` 本身。没有它时选中行仍画 `translation`，皮肤的释义色和不透明的选中底色常常是同一色系（`selected = "#4D6BFE"`、`translation = "#2F5BD3"`），蓝字压在蓝底上读不出来。
    std::optional<Rgba> selectedTranslation;
    // The floating toolbar's separator, set only by a package's `[toolbar]` colours; none draws it in the border colour. The candidate window does not read it.
    std::optional<Rgba> divider;
    // 列表内三角翻页标记前的分隔线。
    std::optional<Rgba> pageArrowDivider;
};

// 选中与悬停使用各自的行 token，首尾位置不借用整个卡片的圆角。
inline float CandidateRowRadius(const SkinTokens &tokens, bool highlighted)
{
    return highlighted ? tokens.selectedRadius : tokens.candidateRadius;
}

// 卡片边缘到候选行的实际距离；零表示贴边，不再以边框宽度作为下限。
inline float CandidateInset(const SkinTokens &tokens)
{
    return std::max(0.0f, static_cast<float>(tokens.paddingHorizontal.value_or(tokens.pad)));
}

inline float CandidateVerticalInset(const SkinTokens &tokens)
{
    return std::max(0.0f, static_cast<float>(tokens.paddingVertical.value_or(tokens.pad)));
}

// 候选行的圆角至少要多大，高亮的角才不伸出卡片的圆角：两个圆角在对角线方向相切时 r = R - d(2 + √2)，卡片圆角 R 和内边距 d 都从边框内沿量起（窗口按矩形裁掉描边外侧的一半）。内边距够大时结果是 0，不改变行圆角。
inline float CandidateRowRadiusFloor(const SkinTokens &tokens)
{
    const float halfBorder = std::max(0.0f, tokens.borderWidth) / 2.0f;
    const float inset = std::min(CandidateInset(tokens), CandidateVerticalInset(tokens)) - halfBorder;
    return std::max(0.0f, tokens.radius - halfBorder - inset * (2.0f + std::sqrt(2.0f)));
}

// 共享偏好的用户覆盖：长度为缩放前的点，缺省保留皮肤与宿主。窗口逐轴内边距先回退到旧的统一 padding；候选项圆角显式设置后不随窗口圆角联动。
struct CandidateWindowStyle
{
    double scale = 1.0;
    double opacity = 1.0;
    std::optional<double> cornerRadius;
    std::optional<double> padding;
    std::optional<double> paddingHorizontal;
    std::optional<double> paddingVertical;
    std::optional<double> itemPaddingHorizontal;
    std::optional<double> itemPaddingVertical;
    std::optional<double> itemCornerRadius;
    std::optional<double> itemSpacing;
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
// 装饰图对齐、摆放和动效余量与 Linux 的 X11 原生画布共用 shared/candidate/CandidateDecoration.h。
using msime::candidate::DecorationAlign;

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

// Canvas 的长度是 DIP，只有 slice 保留源图片像素；路径在包里相对、解析后的皮肤里绝对。
struct SkinInsets
{
    double top = 0, right = 0, bottom = 0, left = 0;
};

// 共享 catalog 的候选几何长度。`em` 长度在宿主拿到候选字号后换算；固定 DIP 长度直接使用。
struct SkinLength
{
    double value = 0.0;
    bool em = false;
};

struct SkinLayoutGeometry
{
    std::optional<SkinLength> padding;
    std::optional<SkinLength> itemGap;
    std::optional<SkinLength> itemExtraHeight;
    std::optional<SkinLength> numberInset;
    std::optional<SkinLength> itemPaddingRight;
    std::optional<SkinLength> itemMinWidth;
    std::optional<SkinLength> numberGap;
    std::optional<double> numberScale;
    std::optional<SkinLength> selectedBarWidth;
    std::optional<double> selectedBarHeightEm;
    std::optional<bool> selectedBarInside;
    std::optional<bool> itemCornersFollowCard;
    std::optional<bool> justifyRows;
    std::optional<bool> trimHiddenPreeditGap;
    std::optional<std::string> pageArrowStyle;
    std::optional<SkinInsets> preeditMarginDip;
};

struct ResolvedSkinLayout
{
    double cornerRadiusDip = 0.0;
    double borderWidthDip = 0.0;
    double itemCornerRadiusDip = 0.0;
    std::optional<std::string> shadow;
    bool pageArrows = false;
    bool fitContent = false;
    bool expandPanel = false;
    unsigned expandRows = 4;
    SkinLayoutGeometry geometry;
    SkinLayoutGeometry horizontal;
    SkinLayoutGeometry vertical;
};
struct SkinBorderImage
{
    std::string source;
    std::string lightSource;
    std::array<double, 4> slice{};
    std::array<double, 4> width{};
    bool fill = true;
};
struct SkinFreeDecoration
{
    std::string image;
    double width = 0, height = 0;
    std::string anchor = "right";
    double offsetX = 0, offsetY = 0;
    int zIndex = 2;
};
struct SkinCanvas
{
    // 宿主的不透明度只淡化面板贴图，不影响文字或自由挂件。
    double panelOpacity = 1.0;
    std::optional<SkinBorderImage> border;
    std::optional<SkinInsets> content;
    SkinInsets outer;
    std::optional<double> lineGap;
    std::vector<SkinFreeDecoration> decorations;
};

inline SkinInsets CanvasContentInsets(const SkinCanvas &canvas, double inset, std::optional<double> verticalInset = std::nullopt)
{
    const auto width = canvas.border ? canvas.border->width : std::array<double, 4>{};
    if (canvas.content) {
        const auto &content = *canvas.content;
        return {std::max(content.top, width[0]), std::max(content.right, width[1]),
                std::max(content.bottom, width[2]), std::max(content.left, width[3])};
    }
    return {verticalInset.value_or(inset) + width[0], inset + width[1],
            verticalInset.value_or(inset) + width[2], inset + width[3]};
}
inline double CanvasExtraWidth(const SkinCanvas &canvas, double inset)
{
    const auto content = CanvasContentInsets(canvas, inset);
    return content.left + content.right - 2 * inset + canvas.outer.left + canvas.outer.right;
}

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

struct SkinToolbarImage
{
    // 共享加载器规范化的缓存 JSON；状态变化通过无 I/O 布局桥处理。
    std::string json;
    std::filesystem::path directory;
};

// 装饰图的待机动效（清单 `decoration.motion`）：候选窗在屏幕上时整张装饰图的小幅循环动作，与显隐动效无关。`float` 是关键字，所以浮动叫 floating。
enum class DecorationMotion
{
    none,
    floating,
    sway,
    breathe,
    bounce,
};

using msime::candidate::kDecorationMotionMargin;

enum class SkinAnimationPreset
{
    none,
    fade,
    fade_scale,
    slide_up,
    spring,
    glow,
};

// 皮肤包清单显式写出的动效；没写的预设是 none，内置主题和不画包时全为 none（即时显隐）。
struct SkinAnimation
{
    SkinAnimationPreset enter = SkinAnimationPreset::none;
    SkinAnimationPreset exit = SkinAnimationPreset::none;
    SkinAnimationPreset selection = SkinAnimationPreset::none;
    double durationMs = 0.0;
    double staggerMs = 0.0;
};

// Licence metadata the manifest declares. Informational only: nothing is enforced or drawn from it.
struct SkinLicense
{
    std::string code;
    std::string assets;
    std::string source;
};

using msime::candidate::DecorationLeft;
using msime::candidate::DecorationPlacement;
using msime::candidate::DecorationRect;

struct SkinRect
{
    double x = 0.0;
    double y = 0.0;
    double width = 0.0;
    double height = 0.0;
};

// 四角顺序为左上、右上、右下、左下；短行末端不借用卡片的右角。
inline std::array<double, 4> CandidateRowCornerRadii(SkinRect row, SkinRect area, bool topEdge,
                                                   double inner, double outer, double tolerance = 0.5)
{
    const bool top = topEdge && row.y <= area.y + tolerance;
    const bool bottom = row.y + row.height >= area.y + area.height - tolerance;
    const bool left = row.x <= area.x + tolerance;
    const bool right = row.x + row.width >= area.x + area.width - tolerance;
    return {top && left ? outer : inner, top && right ? outer : inner,
            bottom && right ? outer : inner, bottom && left ? outer : inner};
}

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
    // 候选行的选中与悬停圆角，与卡片圆角独立。
    std::optional<double> itemCornerRadiusDip;
    double decorationTopDip = 0.0;
    double decorationWidthDip = 0.0;
    // The image drawn in the decoration band, package-relative: the manifest's `decoration.image`, or else an image preview. Empty unless decorated.
    std::string decorationImage;
    DecorationAlign decorationAlign = DecorationAlign::right;
    DecorationMotion decorationMotion = DecorationMotion::none;
    // 1200–8000，共享加载器已经校验过。
    double decorationMotionPeriodMs = 3000.0;
    // 装饰图的帧数：没有装饰图为 0，静态图为 1，APNG 为它的动画帧数。
    unsigned decorationFrames = 0;
    std::optional<msime::candidate::DecorationCandidate> decorationCandidate;
    std::optional<SkinBackground> background;
    std::optional<SkinCanvas> canvas;
    SkinToolbar toolbar;
    std::optional<std::string> shadow;
    SkinToolbarImage toolbarImage;
    std::optional<ResolvedSkinLayout> resolvedLayout;
    SkinAnimation animation;
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
    DecorationMotion decorationMotion = DecorationMotion::none;
    double decorationMotionPeriodMs = 3000.0;
    std::optional<msime::candidate::DecorationCandidate> decorationCandidate;
    // Absolute path of the drawn package's background image, empty for none.
    std::string backgroundPath;
    BackgroundFit backgroundFit = BackgroundFit::cover;
    double backgroundOpacity = 1.0;
    double minWidthDip = 0.0;
    SkinAnimation animation;
    std::optional<SkinCanvas> canvas;
    // 当前横/竖排的共享出厂继承结果；没有 Windows 基底的包保持空值，继续使用 macOS 原生默认。
    std::optional<SkinLayoutGeometry> layout;
    // 只在共享版式存在时生效；收紧取消默认卡宽、固定列间余量和同次组字只增不减。
    bool fitContent = false;
    // 共享皮肤声明的首行之后的卷轴行数；原生主题保留宿主默认。
    std::optional<unsigned> expandRows;
    // 空值保留宿主阴影；共享版式未声明档位时使用来源的普通双层阴影。
    std::optional<std::string> shadow;
    // 空值保留原生首行；共享版式显式控制列表内翻页与横排卷轴。
    std::optional<bool> pageArrows, expandPanel;
    bool canvasPanelDrawn = false;
    SkinToolbarImage toolbarImage;
};

bool IsSafeSkinId(std::string_view id);
// The native macOS candidate tokens (design-tokens-desktop §1.2) that every null slot of a resolved theme falls back to.
SkinTokens NativeCandidateTokens(bool dark);
// 玻璃底板会混入背后的内容，次要文字和选中行使用独立的高对比颜色。
void ApplyNativeGlassCandidateContrast(SkinTokens &tokens, bool dark);
// The selected row's foregrounds for a palette that left them to the platform, derived from the fill actually drawn (THEME_CONTRACT §5 step 6): white or near-black on an opaque fill, the number at 0.82 like the native one, and the row's own text and number colours on a translucent fill, where they are what stays readable. On the native solid accent this gives the native white. `translation` 为真且皮肤给了释义色时，选中行的释义色按选中序号同样的规则推导。
void DeriveSelectedForegrounds(SkinTokens &tokens, bool text, bool number, bool translation = true);
// WCAG 2 对比度：`foreground` 按自己的不透明度合成到 `fill` 上，再和 `fill` 比；`fill` 当作不透明。
float ForegroundContrast(Rgba foreground, Rgba fill);
// 选中行的释义和读音要达到的对比度：WCAG AA 正文的 4.5:1。
inline constexpr float kSelectedGlossMinContrast = 4.5f;
// 皮肤没有释义色时选中行的释义色（读音同色），和 Windows 的 `candidate_row_translation_color` 同一规则：选中底色半透明，或选中序号色在它上面已经够 4.5:1 时，就画选中序号色；否则取选中文字的颜色，不透明度从和选中序号一样的 0.82 起往上加，到够 4.5:1 或不透明为止。同一行只有一种前景色，释义不会比候选文字更显眼。默认浅色 #2C7A4B、深色 #17783F 上白色 0.82 都只有约 4.1–4.3:1，所以默认皮肤的释义是白色 0.86–0.89。
Rgba SelectedRowGloss(const SkinTokens &tokens);
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
// 以 `base` 为底的皮肤包所属的槽位，即 base 那个内置主题的明暗（client-services 的 `theme::skin_appearance`）；system 和目录里没有的 id 都是 both。
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
                         const std::filesystem::path &skinsRoot, bool canvasPanelDrawn = false);
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
