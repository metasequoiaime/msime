// 外部皮肤清单经宿主 C ABI 由共享的 client-services 加载器校验，设置页和其他宿主用的也是它。Windows 用 toml++ 解析 skin.toml（server/src/skin/candidate_skin_catalog.cpp），所以这个加载器也必须是完整的 TOML 1.0 解析器；在这里手写一个子集，会让设置页列为有效的包被画成 Fluent。
#include "CandidateSkin.h"

#import <Foundation/Foundation.h>

#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <string>
#include <utility>

#if !__has_feature(objc_arc)
#error "SkinManifestBridge.mm must be compiled with -fobjc-arc"
#endif

namespace msime::mac
{
namespace
{
using HostCall = char *(*)(const uint8_t *, size_t);

// The {ok,value} / {ok:false,error} envelope every msime_client_* JSON call answers with.
struct HostReply
{
    id value = nil;
    std::string error;
};

HostReply CallHost(HostCall call, NSData *request)
{
    HostReply reply;
    auto raw = msime::host_api::own_string(
        call(static_cast<const uint8_t *>(request.bytes), request.length));
    if (!raw)
    {
        reply.error = "no response";
        return reply;
    }
    NSData *data = [NSData dataWithBytes:raw.get() length:std::char_traits<char>::length(raw.get())];
    NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    if (![envelope isKindOfClass:NSDictionary.class])
    {
        reply.error = "invalid response";
        return reply;
    }
    if ([envelope[@"ok"] isEqual:@YES])
    {
        reply.value = envelope[@"value"];
        return reply;
    }
    NSString *error = envelope[@"error"];
    reply.error = [error isKindOfClass:NSString.class] ? std::string(error.UTF8String ?: "") : "error";
    return reply;
}

// The host API only takes absolute roots. An empty root means "no skins directory", never the working directory.
NSString *RootString(const std::filesystem::path &skinsRoot)
{
    if (skinsRoot.empty())
    {
        return nil;
    }
    std::error_code ec;
    const std::filesystem::path absolute = std::filesystem::absolute(skinsRoot, ec);
    if (ec)
    {
        return nil;
    }
    return [NSString stringWithUTF8String:absolute.string().c_str()];
}

std::string StringField(NSDictionary *object, NSString *key)
{
    NSString *value = object[key];
    return [value isKindOfClass:NSString.class] ? std::string(value.UTF8String ?: "") : std::string();
}

double NumberField(NSDictionary *object, NSString *key)
{
    NSNumber *value = object[key];
    return [value isKindOfClass:NSNumber.class] ? value.doubleValue : 0.0;
}

// A number the loader may publish as null, where null means "keep the host's own" and 0 is a real value.
std::optional<double> OptionalNumberField(NSDictionary *object, NSString *key)
{
    NSNumber *value = object[key];
    if (![value isKindOfClass:NSNumber.class]) return std::nullopt;
    return value.doubleValue;
}

SkinAnimationPreset AnimationPreset(NSString *value)
{
    const std::string name = value.UTF8String ?: "";
    if (name == "fade") return SkinAnimationPreset::fade;
    if (name == "fade_scale") return SkinAnimationPreset::fade_scale;
    if (name == "slide_up") return SkinAnimationPreset::slide_up;
    if (name == "spring") return SkinAnimationPreset::spring;
    if (name == "glow") return SkinAnimationPreset::glow;
    return SkinAnimationPreset::none;
}

// 清单写了这个预设才播放：省略 [animation] 或其中某个预设时，共享解析器补上的活泼默认组合在 macOS 上一律按 none 处理，见 .agents/notes/implemented/feature/2026-10-16-macos-candidate-declared-motion.md。没有 declared 的旧库同样不播放。
SkinAnimationPreset DeclaredAnimationPreset(NSDictionary *object, NSDictionary *declared, NSString *key)
{
    NSNumber *written = [declared isKindOfClass:NSDictionary.class] ? declared[key] : nil;
    if (![written isKindOfClass:NSNumber.class] || !written.boolValue) return SkinAnimationPreset::none;
    return AnimationPreset(object[key]);
}

SkinAnimation AnimationFromJSON(NSDictionary *object)
{
    SkinAnimation animation;
    if (![object isKindOfClass:NSDictionary.class]) return animation;
    NSDictionary *declared = object[@"declared"];
    animation.enter = DeclaredAnimationPreset(object, declared, @"enter");
    animation.exit = DeclaredAnimationPreset(object, declared, @"exit");
    animation.selection = DeclaredAnimationPreset(object, declared, @"selection");
    const double duration = NumberField(object, @"durationMs");
    const double stagger = NumberField(object, @"staggerMs");
    if (duration >= 40.0 && duration <= 600.0) animation.durationMs = duration;
    if (stagger >= 0.0 && stagger <= 80.0) animation.staggerMs = stagger;
    return animation;
}

std::vector<std::string> StringArrayField(NSDictionary *object, NSString *key)
{
    std::vector<std::string> values;
    NSArray *items = object[key];
    if (![items isKindOfClass:NSArray.class])
    {
        return values;
    }
    values.reserve(items.count);
    for (NSString *item in items)
    {
        if ([item isKindOfClass:NSString.class])
        {
            values.emplace_back(item.UTF8String ?: "");
        }
    }
    return values;
}

// Colors stay strings: ParseCssColor still decides, at resolve time, which of them the native presenter can draw.
SkinColors ColorsFromJSON(NSDictionary *palette)
{
    SkinColors colors;
    if (![palette isKindOfClass:NSDictionary.class])
    {
        return colors;
    }
    colors.accent = StringField(palette, @"accent");
    colors.selected = StringField(palette, @"selected");
    colors.hover = StringField(palette, @"hover");
    colors.surface = StringField(palette, @"surface");
    colors.border = StringField(palette, @"border");
    colors.text = StringField(palette, @"text");
    colors.number = StringField(palette, @"number");
    colors.translation = StringField(palette, @"translation");
    NSNumber *bar = palette[@"showSelectedBar"];
    if ([bar isKindOfClass:NSNumber.class])
    {
        colors.showSelectedBar = bar.boolValue;
    }
    return colors;
}

SkinToolbarColors ToolbarColorsFromJSON(NSDictionary *palette)
{
    SkinToolbarColors colors;
    if (![palette isKindOfClass:NSDictionary.class])
    {
        return colors;
    }
    colors.background = StringField(palette, @"background");
    colors.border = StringField(palette, @"border");
    colors.handle = StringField(palette, @"handle");
    colors.divider = StringField(palette, @"divider");
    colors.icon = StringField(palette, @"icon");
    colors.hover = StringField(palette, @"hover");
    return colors;
}

std::array<double, 4> BoxField(id raw)
{
    NSArray *values = raw;
    if (![values isKindOfClass:NSArray.class] || (values.count != 1 && values.count != 2 && values.count != 4)) return {};
    auto number = [&](NSUInteger index) { return [values[index] isKindOfClass:NSNumber.class] ? [values[index] doubleValue] : 0.0; };
    return {number(0), number(values.count == 1 ? 0 : 1), number(values.count == 4 ? 2 : 0), number(values.count == 4 ? 3 : values.count == 1 ? 0 : 1)};
}
SkinInsets InsetsField(NSDictionary *object)
{
    return {NumberField(object, @"top"), NumberField(object, @"right"), NumberField(object, @"bottom"), NumberField(object, @"left")};
}

std::optional<SkinLength> LengthField(id raw)
{
    if (![raw isKindOfClass:NSDictionary.class]) return std::nullopt;
    NSDictionary *object = raw;
    NSNumber *value = object[@"value"];
    NSNumber *em = object[@"em"];
    if (![value isKindOfClass:NSNumber.class] || ![em isKindOfClass:NSNumber.class]) return std::nullopt;
    return SkinLength{value.doubleValue, static_cast<bool>(em.boolValue)};
}

SkinLayoutGeometry LayoutGeometryFromJSON(id raw)
{
    SkinLayoutGeometry geometry;
    if (![raw isKindOfClass:NSDictionary.class]) return geometry;
    NSDictionary *object = raw;
    geometry.padding = LengthField(object[@"padding"]);
    geometry.itemGap = LengthField(object[@"itemGap"]);
    geometry.itemExtraHeight = LengthField(object[@"itemExtraHeight"]);
    geometry.numberInset = LengthField(object[@"numberInset"]);
    geometry.itemPaddingRight = LengthField(object[@"itemPaddingRight"]);
    geometry.itemMinWidth = LengthField(object[@"itemMinWidth"]);
    geometry.numberGap = LengthField(object[@"numberGap"]);
    geometry.numberScale = OptionalNumberField(object, @"numberScale");
    geometry.selectedBarWidth = LengthField(object[@"selectedBarWidth"]);
    geometry.selectedBarHeightEm = OptionalNumberField(object, @"selectedBarHeightEm");
    NSNumber *selectedBarInside = object[@"selectedBarInside"];
    if ([selectedBarInside isKindOfClass:NSNumber.class]) geometry.selectedBarInside = selectedBarInside.boolValue;
    NSNumber *itemCornersFollowCard = object[@"itemCornersFollowCard"];
    if ([itemCornersFollowCard isKindOfClass:NSNumber.class]) geometry.itemCornersFollowCard = itemCornersFollowCard.boolValue;
    NSNumber *justifyRows = object[@"justifyRows"];
    if ([justifyRows isKindOfClass:NSNumber.class]) geometry.justifyRows = justifyRows.boolValue;
    NSNumber *trimHiddenPreeditGap = object[@"trimHiddenPreeditGap"];
    if ([trimHiddenPreeditGap isKindOfClass:NSNumber.class]) geometry.trimHiddenPreeditGap = trimHiddenPreeditGap.boolValue;
    NSString *pageArrowStyle = object[@"pageArrowStyle"];
    if ([pageArrowStyle isKindOfClass:NSString.class]) geometry.pageArrowStyle = std::string(pageArrowStyle.UTF8String ?: "");
    if ([object[@"preeditMarginDip"] isKindOfClass:NSDictionary.class]) geometry.preeditMarginDip = InsetsField(object[@"preeditMarginDip"]);
    return geometry;
}

std::optional<ResolvedSkinLayout> ResolvedLayoutFromJSON(id raw)
{
    if (![raw isKindOfClass:NSDictionary.class]) return std::nullopt;
    NSDictionary *object = raw;
    ResolvedSkinLayout layout;
    layout.cornerRadiusDip = NumberField(object, @"cornerRadiusDip");
    layout.borderWidthDip = NumberField(object, @"borderWidthDip");
    layout.itemCornerRadiusDip = NumberField(object, @"itemCornerRadiusDip");
    if ([object[@"shadow"] isKindOfClass:NSString.class]) layout.shadow = StringField(object, @"shadow");
    layout.pageArrows = [object[@"pageArrows"] isKindOfClass:NSNumber.class] && [object[@"pageArrows"] boolValue];
    layout.fitContent = [object[@"fitContent"] isKindOfClass:NSNumber.class] && [object[@"fitContent"] boolValue];
    layout.expandPanel = [object[@"expandPanel"] isKindOfClass:NSNumber.class] && [object[@"expandPanel"] boolValue];
    NSNumber *expandRows = object[@"expandRows"];
    if ([expandRows isKindOfClass:NSNumber.class]) layout.expandRows = expandRows.unsignedIntValue;
    layout.geometry = LayoutGeometryFromJSON(object[@"geometry"]);
    layout.horizontal = LayoutGeometryFromJSON(object[@"horizontal"]);
    layout.vertical = LayoutGeometryFromJSON(object[@"vertical"]);
    return layout;
}

std::optional<SkinCanvas> CanvasFromJSON(NSDictionary *object)
{
    if (![object[@"canvas"] isEqual:@YES] || NumberField(object, @"schemaVersion") != 2) return std::nullopt;
    SkinCanvas canvas;
    NSDictionary *border = object[@"borderImage"];
    if ([border isKindOfClass:NSDictionary.class]) {
        SkinBorderImage image;
        image.source = StringField(border, @"source");
        image.lightSource = StringField(border, @"lightSource");
        image.slice = BoxField([border[@"stretchDip"] isKindOfClass:NSArray.class] ? border[@"stretchDip"] : border[@"sliceDip"]);
        image.width = [border[@"widthDip"] isKindOfClass:NSArray.class] ? BoxField(border[@"widthDip"]) : image.slice;
        image.fill = ![border[@"fill"] isEqual:@NO];
        canvas.border = std::move(image);
    }
    if ([object[@"contentInsetsDip"] isKindOfClass:NSDictionary.class]) canvas.content = InsetsField(object[@"contentInsetsDip"]);
    if ([object[@"outerInsetsDip"] isKindOfClass:NSDictionary.class]) canvas.outer = InsetsField(object[@"outerInsetsDip"]);
    canvas.lineGap = OptionalNumberField(object, @"lineGapDip");
    NSArray *decorations = object[@"decorations"];
    if ([decorations isKindOfClass:NSArray.class]) for (NSDictionary *item in decorations) {
        if (![item isKindOfClass:NSDictionary.class]) continue;
        SkinFreeDecoration decoration;
        decoration.image = StringField(item, @"image");
        decoration.width = NumberField(item, @"widthDip");
        decoration.height = NumberField(item, @"heightDip");
        decoration.anchor = StringField(item, @"anchor");
        decoration.offsetX = NumberField(item, @"offsetXDip");
        decoration.offsetY = NumberField(item, @"offsetYDip");
        decoration.zIndex = static_cast<int>(NumberField(item, @"zIndex"));
        canvas.decorations.push_back(std::move(decoration));
    }
    std::stable_sort(canvas.decorations.begin(), canvas.decorations.end(), [](const auto &a, const auto &b) { return a.zIndex < b.zIndex; });
    return canvas;
}

std::optional<SkinPackage> PackageFromJSON(id value)
{
    NSDictionary *object = value;
    if (![object isKindOfClass:NSDictionary.class])
    {
        return std::nullopt;
    }
    SkinPackage package;
    package.id = StringField(object, @"id");
    package.name = StringField(object, @"name");
    package.version = StringField(object, @"version");
    package.author = StringField(object, @"author");
    package.description = StringField(object, @"description");
    package.base = StringField(object, @"base");
    package.toolbarStylesheet = StringField(object, @"toolbarStylesheet");
    package.preview = StringField(object, @"preview");
    package.layouts = StringArrayField(object, @"layouts");
    package.themes = StringArrayField(object, @"themes");
    package.minWidthDip = NumberField(object, @"minWidthDip");
    package.decorationTopDip = NumberField(object, @"decorationTopDip");
    package.decorationWidthDip = NumberField(object, @"decorationWidthDip");
    package.cornerRadiusDip = OptionalNumberField(object, @"cornerRadiusDip");
    package.itemCornerRadiusDip = OptionalNumberField(object, @"itemCornerRadiusDip");
    package.decorationImage = StringField(object, @"decorationImage");
    const std::string align = StringField(object, @"decorationAlign");
    package.decorationAlign = align == "left"     ? DecorationAlign::left
                              : align == "center" ? DecorationAlign::center
                                                  : DecorationAlign::right;
    const std::string motion = StringField(object, @"decorationMotion");
    package.decorationMotion = motion == "float"     ? DecorationMotion::floating
                               : motion == "sway"    ? DecorationMotion::sway
                               : motion == "breathe" ? DecorationMotion::breathe
                               : motion == "bounce"  ? DecorationMotion::bounce
                                                     : DecorationMotion::none;
    const double period = NumberField(object, @"decorationMotionPeriodMs");
    if (period >= 1200.0 && period <= 8000.0) package.decorationMotionPeriodMs = period;
    const double frames = NumberField(object, @"decorationFrames");
    package.decorationFrames = frames >= 0.0 && frames <= 1000.0 ? static_cast<unsigned>(frames) : 0;
    NSDictionary *overlay = object[@"decorationCandidate"];
    if ([overlay isKindOfClass:NSDictionary.class]) {
        msime::candidate::DecorationCandidate parsed;
        const double index = NumberField(overlay, @"index");
        parsed.index = std::isfinite(index) && index >= 0 && index <= 100 && std::floor(index) == index
            ? static_cast<int>(index) : -1;
        parsed.fontSizeDip = NumberField(overlay, @"fontSizeDip");
        parsed.minFontSizeDip = NumberField(overlay, @"minFontSizeDip");
        parsed.maxWidthDip = NumberField(overlay, @"maxWidthDip");
        parsed.color = StringField(overlay, @"resolvedColor");
        parsed.alignH = StringField(overlay, @"alignH");
        parsed.alignV = StringField(overlay, @"alignV");
        parsed.offsetXDip = NumberField(overlay, @"offsetXDip");
        parsed.offsetYDip = NumberField(overlay, @"offsetYDip");
        parsed.stripBadge = ![overlay[@"stripBadge"] isEqual:@NO];
        if (parsed.valid()) package.decorationCandidate = std::move(parsed);
    }
    NSDictionary *background = object[@"background"];
    if ([background isKindOfClass:NSDictionary.class] && !StringField(background, @"image").empty())
    {
        SkinBackground parsed;
        parsed.image = StringField(background, @"image");
        const std::string fit = StringField(background, @"fit");
        parsed.fit = fit == "contain" ? BackgroundFit::contain : fit == "stretch" ? BackgroundFit::stretch : BackgroundFit::cover;
        parsed.opacity = OptionalNumberField(background, @"opacity").value_or(1.0);
        package.background = std::move(parsed);
    }
    NSDictionary *toolbar = object[@"toolbar"];
    if ([toolbar isKindOfClass:NSDictionary.class])
    {
        package.toolbar.cornerRadiusDip = OptionalNumberField(toolbar, @"cornerRadiusDip");
        package.toolbar.dark = ToolbarColorsFromJSON(toolbar[@"dark"]);
        package.toolbar.light = ToolbarColorsFromJSON(toolbar[@"light"]);
    }
    package.resolvedLayout = ResolvedLayoutFromJSON(object[@"resolvedLayout"]);
    if ([object[@"shadow"] isKindOfClass:NSString.class]) package.shadow = StringField(object, @"shadow");
    if ([object[@"toolbarImage"] isKindOfClass:NSDictionary.class]) {
        NSData *data = [NSJSONSerialization dataWithJSONObject:object[@"toolbarImage"] options:0 error:nil];
        if (data) package.toolbarImage.json.assign(static_cast<const char *>(data.bytes), data.length);
    }
    package.canvas = CanvasFromJSON(object);
    package.animation = AnimationFromJSON(object[@"animation"]);
    NSDictionary *license = object[@"license"];
    if ([license isKindOfClass:NSDictionary.class])
    {
        package.license = SkinLicense{StringField(license, @"code"), StringField(license, @"assets"), StringField(license, @"source")};
    }
    NSDictionary *candidate = object[@"candidate"];
    if ([candidate isKindOfClass:NSDictionary.class])
    {
        package.dark = ColorsFromJSON(candidate[@"dark"]);
        package.light = ColorsFromJSON(candidate[@"light"]);
    }
    // The loader has already refused a base that is not `system` or a built-in theme; an empty one means the reply is not a package at all.
    if (package.id.empty() || package.name.empty() || package.base.empty())
    {
        return std::nullopt;
    }
    return package;
}

bool StartsWith(const std::string &text, std::string_view prefix)
{
    return text.compare(0, prefix.size(), prefix) == 0;
}

// 设置页的诊断文字是中文；共享加载器的原因是英文短语，这里把已知的原因换成对应的中文说明。
std::string LocalizedReason(const std::string &reason)
{
    if (reason == "invalid skin id")
    {
        return "目录名不是有效的外部皮肤 ID";
    }
    if (reason == "manifest escapes skin directory" || reason == "missing skin directory")
    {
        return "皮肤 manifest 不在有效包目录内";
    }
    if (reason == "missing skin.toml" || reason == "unreadable skin.toml" ||
        reason == "skin.toml is not a regular file" || reason == "skin.toml is not UTF-8")
    {
        return "缺少或无法读取 skin.toml";
    }
    if (reason == "skin.toml is too large")
    {
        return "skin.toml 过大";
    }
    if (reason == "invalid TOML" || reason == "manifest must be a table")
    {
        return "skin.toml 不是有效的 TOML manifest";
    }
    if (reason == "unsupported schema_version")
    {
        return "仅支持 schema_version 1";
    }
    if (StartsWith(reason, "toolbar_stylesheet ") || reason == "invalid toolbar_stylesheet")
    {
        return "toolbar_stylesheet 必须是皮肤目录内存在的 .css 文件";
    }
    if (StartsWith(reason, "preview ") || reason == "invalid preview")
    {
        return "preview 必须是皮肤目录内的相对路径";
    }
    if (reason == "manifest id does not match folder" || reason == "unsupported base skin")
    {
        return "manifest 的基本信息无效";
    }
    if (reason == "base must be system or a built-in theme")
    {
        return "base 必须是 system 或内置主题（shuishan、light、paper、night、ink）";
    }
    for (std::string_view key : {"id ", "name ", "version ", "base ", "author ", "description "})
    {
        if (StartsWith(reason, key))
        {
            return "manifest 的基本信息无效";
        }
    }
    if (reason == "missing supports" || reason == "invalid supports")
    {
        return "supports.layouts 或 supports.themes 无效";
    }
    if (reason == "missing candidate_window")
    {
        return "缺少 candidate_window";
    }
    if (reason == "invalid min_width_dip")
    {
        return "candidate_window.min_width_dip 超出范围";
    }
    if (reason == "missing decoration")
    {
        return "缺少 candidate_window.decoration";
    }
    if (reason == "invalid decoration")
    {
        return "decoration 尺寸、图片或对齐方式无效";
    }
    if (reason == "decoration motion requires schema_version 2")
    {
        return "decoration.motion 需要 schema_version 2";
    }
    if (reason == "invalid decoration motion" || reason == "invalid decoration motion_period_ms")
    {
        return "decoration.motion 须为 none、float、sway、breathe 或 bounce，motion_period_ms 在 1200–8000 之间，且包要有装饰";
    }
    if (reason == "animated image has too many frames")
    {
        return "APNG 动画帧数超过 120";
    }
    if (reason == "image dimensions exceed limits")
    {
        return "图片尺寸超出限制：每边最多 2048 像素，全部图片（APNG 每一帧都算一张）合计最多 800 万像素";
    }
    for (std::string_view key : {"image ", "align "})
    {
        if (StartsWith(reason, key))
        {
            return "decoration 尺寸、图片或对齐方式无效";
        }
    }
    if (reason == "invalid corner_radius_dip")
    {
        return "candidate_window.corner_radius_dip 超出范围（0–32）";
    }
    if (reason == "invalid background")
    {
        return "candidate_window.background 无效：image 须为皮肤目录内的图片，fit 为 cover、contain 或 stretch，opacity 在 0–1 之间";
    }
    if (reason == "invalid toolbar" || reason == "toolbar color exceeds 80 bytes")
    {
        return "toolbar 的圆角或配色无效";
    }
    if (reason == "invalid license")
    {
        return "license 必须是表";
    }
    for (std::string_view key : {"code ", "assets ", "source "})
    {
        if (StartsWith(reason, key))
        {
            return "license 的 code、assets 或 source 无效";
        }
    }
    if (reason == "invalid candidate colors" || reason == "candidate color exceeds 80 bytes")
    {
        return "candidate 配色无效";
    }
    return "无法加载该皮肤包";
}

// Theme colours are `#RRGGBB` or `#RRGGBBAA`; a null or unreadable slot keeps the native token.
bool ApplyThemeColor(NSDictionary *palette, NSString *key, Rgba &slot)
{
    if (const auto color = ParseCssColor(StringField(palette, key)))
    {
        slot = *color;
        return true;
    }
    return false;
}

Rgba ThemeColor(NSDictionary *object, NSString *key)
{
    return ParseCssColor(StringField(object, key)).value_or(Rgba{});
}

void SetError(std::string *error, std::string message)
{
    if (error != nullptr)
    {
        *error = std::move(message);
    }
}
} // namespace

std::optional<SkinPackage> LoadSkinPackage(const std::filesystem::path &skinsRoot, const std::string &id,
                                           std::string *error)
{
    @autoreleasepool
    {
        if (!IsSafeSkinId(id) || IsGlobalThemeId(id))
        {
            SetError(error, LocalizedReason("invalid skin id"));
            return std::nullopt;
        }
        NSString *root = RootString(skinsRoot);
        NSString *identifier = [NSString stringWithUTF8String:id.c_str()];
        if (root == nil || identifier == nil)
        {
            SetError(error, LocalizedReason("missing skin directory"));
            return std::nullopt;
        }
        NSData *request = [NSJSONSerialization dataWithJSONObject:@{@"directory" : root, @"id" : identifier}
                                                          options:0
                                                            error:nil];
        const HostReply reply = CallHost(msime_client_skin_package, request);
        if (reply.value == nil)
        {
            SetError(error, LocalizedReason(reply.error));
            return std::nullopt;
        }
        auto package = PackageFromJSON(reply.value);
        if (!package)
        {
            SetError(error, LocalizedReason(""));
        }
        return package;
    }
}

SkinCatalog ScanSkinCatalog(const std::filesystem::path &skinsRoot)
{
    @autoreleasepool
    {
        SkinCatalog result;
        NSString *root = RootString(skinsRoot);
        if (root == nil)
        {
            return result;
        }
        NSData *request = [root dataUsingEncoding:NSUTF8StringEncoding];
        const HostReply reply = CallHost(msime_client_skin_catalog, request);
        NSDictionary *catalog = reply.value;
        if (![catalog isKindOfClass:NSDictionary.class])
        {
            return result;
        }
        // The loader already sorts packages by name and issues by folder, the order the settings page shows.
        NSArray *packages = catalog[@"packages"];
        if ([packages isKindOfClass:NSArray.class])
        {
            result.packages.reserve(packages.count);
            for (id entry in packages)
            {
                if (auto package = PackageFromJSON(entry))
                {
                    result.packages.push_back(std::move(*package));
                }
            }
        }
        NSArray *issues = catalog[@"issues"];
        if ([issues isKindOfClass:NSArray.class])
        {
            result.issues.reserve(issues.count);
            for (NSDictionary *issue in issues)
            {
                if ([issue isKindOfClass:NSDictionary.class])
                {
                    result.issues.push_back({StringField(issue, @"folder"), LocalizedReason(StringField(issue, @"reason"))});
                }
            }
        }
        return result;
    }
}
const std::vector<ThemeCatalogEntry> &ThemeCatalog()
{
    // The catalog is compiled into the host library, so one read serves the process.
    static const std::vector<ThemeCatalogEntry> entries = [] {
        std::vector<ThemeCatalogEntry> result;
        @autoreleasepool
        {
            auto raw = msime::host_api::own_string(msime_client_theme_catalog());
            if (!raw)
            {
                return result;
            }
            NSData *data = [NSData dataWithBytes:raw.get() length:std::char_traits<char>::length(raw.get())];
            NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
            NSDictionary *value = [envelope isKindOfClass:NSDictionary.class] && [envelope[@"ok"] isEqual:@YES]
                                      ? envelope[@"value"]
                                      : nil;
            NSArray *themes = [value isKindOfClass:NSDictionary.class] ? value[@"themes"] : nil;
            if (![themes isKindOfClass:NSArray.class])
            {
                return result;
            }
            result.reserve(themes.count);
            for (NSDictionary *theme in themes)
            {
                if (![theme isKindOfClass:NSDictionary.class])
                {
                    continue;
                }
                ThemeCatalogEntry entry;
                entry.id = StringField(theme, @"id");
                entry.title = StringField(theme, @"title");
                entry.appearance = StringField(theme, @"appearance");
                NSDictionary *preview = theme[@"preview"];
                if ([preview isKindOfClass:NSDictionary.class])
                {
                    entry.hasPreview = true;
                    entry.previewBackground = ThemeColor(preview, @"background");
                    entry.previewPanel = ThemeColor(preview, @"panel");
                    entry.previewAccent = ThemeColor(preview, @"accent");
                    entry.previewText = ThemeColor(preview, @"text");
                }
                if (!entry.id.empty())
                {
                    result.push_back(std::move(entry));
                }
            }
        }
        return result;
    }();
    return entries;
}

bool IsGlobalThemeId(std::string_view id)
{
    for (const ThemeCatalogEntry &entry : ThemeCatalog())
    {
        if (entry.id == id)
        {
            return true;
        }
    }
    return false;
}

bool IsThemeBaseId(std::string_view id)
{
    return id != "custom" && IsGlobalThemeId(id);
}

std::string ThemeTitle(std::string_view id)
{
    for (const ThemeCatalogEntry &entry : ThemeCatalog())
    {
        if (entry.id == id)
        {
            return entry.title;
        }
    }
    return std::string(id);
}

ResolvedSkin ResolveSkin(std::string_view globalTheme, const CustomTheme &custom, bool dark, std::string_view layout,
                         const std::filesystem::path &skinsRoot, bool canvasPanelDrawn)
{
    @autoreleasepool
    {
        ResolvedSkin resolved;
        // Nothing turns an unknown id into `system` in storage; this only decides what is drawn meanwhile.
        resolved.id = IsGlobalThemeId(globalTheme) ? std::string(globalTheme) : std::string("system");
        resolved.name = ThemeTitle(resolved.id);
        resolved.dark = dark;
        resolved.tokens = NativeCandidateTokens(dark);

        NSMutableDictionary *customTheme = [NSMutableDictionary dictionary];
        if (IsThemeBaseId(custom.base) && custom.base != "system")
        {
            customTheme[@"base"] = @(custom.base.c_str());
        }
        // 两个槽位都原样带上：深色模式取哪一个、包在不在这个明暗下画，都由共享解析器决定。
        if (!custom.candidateSkin.empty() && IsSafeSkinId(custom.candidateSkin) && !IsGlobalThemeId(custom.candidateSkin))
        {
            customTheme[@"candidate_skin"] = @(custom.candidateSkin.c_str());
        }
        if (!custom.candidateSkinDark.empty() && IsSafeSkinId(custom.candidateSkinDark) &&
            !IsGlobalThemeId(custom.candidateSkinDark))
        {
            customTheme[@"candidate_skin_dark"] = @(custom.candidateSkinDark.c_str());
        }
        NSMutableDictionary *colors = [NSMutableDictionary dictionary];
        const std::pair<NSString *, const std::string *> pickers[] = {
            {@"text", &custom.candidateColors.text},       {@"number", &custom.candidateColors.number},
            {@"accent", &custom.candidateColors.accent},   {@"selected", &custom.candidateColors.selected},
            {@"hover", &custom.candidateColors.hover},     {@"surface", &custom.candidateColors.surface},
            {@"border", &custom.candidateColors.border},
        };
        for (const auto &[key, value] : pickers)
        {
            // The shared validator takes exactly `#RRGGBB`; anything else would fail the whole request.
            if (value->size() == 7 && value->front() == '#' && ParseCssColor(*value))
            {
                colors[key] = @(value->c_str());
            }
        }
        if (colors.count > 0)
        {
            customTheme[@"candidate_colors"] = colors;
        }
        NSMutableDictionary *requestObject = [@{
            @"canvas_panel_drawn" : @(canvasPanelDrawn),
            @"global_theme" : @(resolved.id.c_str()),
            @"custom_theme" : customTheme,
            @"dark" : @(dark),
            @"layout" : layout == "vertical" ? @"vertical" : @"horizontal",
        } mutableCopy];
        NSString *root = RootString(skinsRoot);
        if (root != nil)
        {
            requestObject[@"skins_directory"] = root;
        }
        NSData *request = [NSJSONSerialization dataWithJSONObject:requestObject options:0 error:nil];
        const HostReply reply = CallHost(msime_client_resolve_theme, request);
        NSDictionary *value = reply.value;
        if (![value isKindOfClass:NSDictionary.class])
        {
            return resolved;
        }
        NSString *appearance = value[@"appearance"];
        if ([appearance isKindOfClass:NSString.class])
        {
            resolved.fixedDark = [appearance isEqualToString:@"dark"];
            resolved.dark = *resolved.fixedDark;
            resolved.tokens = NativeCandidateTokens(resolved.dark);
        }
        NSDictionary *palette = value[@"candidate"];
        if ([palette isKindOfClass:NSDictionary.class])
        {
            SkinTokens &tokens = resolved.tokens;
            ApplyThemeColor(palette, @"surface", tokens.surface);
            ApplyThemeColor(palette, @"border", tokens.border);
            ApplyThemeColor(palette, @"text", tokens.text);
            ApplyThemeColor(palette, @"number", tokens.number);
            // `secondary` follows `number` unless a package gives a translation colour; only then is the translation drawn in a colour of its own.
            const std::string secondary = StringField(palette, @"secondary");
            if (!secondary.empty() && secondary != StringField(palette, @"number"))
            {
                tokens.translation = ParseCssColor(secondary);
            }
            // 原生选中底是实心的强调色，主题或取色器给了强调色时，`selected` 为 null 就跟着它画；强调色也留给平台时保留原生选中底（深色是比强调色更深的 #17783F，见 `NativeCandidateTokens`）。
            if (ApplyThemeColor(palette, @"accent", tokens.accent)) tokens.selected = tokens.accent;
            ApplyThemeColor(palette, @"selected", tokens.selected);
            const bool selectedText = ApplyThemeColor(palette, @"selected_text", tokens.selectedText);
            const bool selectedNumber = ApplyThemeColor(palette, @"selected_number", tokens.selectedNumber);
            // 皮肤清单可以写 `selected_translation`；共享层哪天把它放进解析结果，就照它画，不再推导。
            Rgba selectedTranslation;
            const bool givenSelectedTranslation = tokens.translation && ApplyThemeColor(palette, @"selected_translation", selectedTranslation);
            if (givenSelectedTranslation) tokens.selectedTranslation = selectedTranslation;
            // Over a system base the shared resolver leaves these two to the platform, and the native white is only readable on the native solid accent: a package's translucent `selected` or a light picked colour needs them from the fill that is actually drawn.
            DeriveSelectedForegrounds(tokens, !selectedText, !selectedNumber, !givenSelectedTranslation);
            ApplyThemeColor(palette, @"hover", tokens.hover);
            tokens.selectedHover = tokens.selected;
            NSNumber *bar = palette[@"show_selected_bar"];
            if ([bar isKindOfClass:NSNumber.class])
            {
                tokens.showSelectedBar = bar.boolValue;
            }
        }
        NSDictionary *details = value[@"candidate_details"];
        if ([details isKindOfClass:NSDictionary.class]) {
            resolved.tokens.preeditText = ParseCssColor(StringField(details, @"preedit_text"));
            resolved.tokens.preeditCaret = ParseCssColor(StringField(details, @"preedit_caret"));
            resolved.tokens.selectedBar = ParseCssColor(StringField(details, @"selected_bar"));
            resolved.tokens.pageArrowDivider = ParseCssColor(StringField(details, @"page_arrow_divider"));
            if (auto selectedTranslation = ParseCssColor(StringField(details, @"selected_translation")))
                resolved.tokens.selectedTranslation = selectedTranslation;
        }
        NSString *drawn = value[@"candidate_skin"];
        if ([drawn isKindOfClass:NSString.class] && drawn.length > 0)
        {
            resolved.candidateSkin = drawn.UTF8String ?: "";
            // The decoration, background, card radius and minimum width still come from the package itself; the theme only says whether it is drawn.
            if (const auto package = LoadSkinPackage(skinsRoot, resolved.candidateSkin))
            {
                resolved.toolbarImage = package->toolbarImage;
                resolved.toolbarImage.directory = skinsRoot / package->id;
                resolved.canvas = package->canvas;
                resolved.shadow = package->shadow;
                if (resolved.canvas) {
                    const auto absolute = [&](const std::string &path) { return path.empty() ? path : (skinsRoot / package->id / path).string(); };
                    if (resolved.canvas->border) {
                        auto &border = *resolved.canvas->border;
                        border.source = absolute(border.source);
                        border.lightSource = absolute(border.lightSource);
                        resolved.canvasPanelDrawn = canvasPanelDrawn;
                    }
                    for (auto &decoration : resolved.canvas->decorations) decoration.image = absolute(decoration.image);
                }
                resolved.name = package->name;
                resolved.animation = package->animation;
                resolved.decorationTopDip = package->decorationTopDip;
                resolved.decorationWidthDip = package->decorationWidthDip;
                resolved.decorationAlign = package->decorationAlign;
                resolved.decorationMotion = package->decorationMotion;
                resolved.decorationMotionPeriodMs = package->decorationMotionPeriodMs;
                resolved.decorationCandidate = package->decorationCandidate;
                resolved.minWidthDip = package->minWidthDip;
                if (package->cornerRadiusDip)
                {
                    resolved.tokens.radius = static_cast<float>(*package->cornerRadiusDip);
                }
                if (package->itemCornerRadiusDip)
                {
                    resolved.tokens.candidateRadius = resolved.tokens.selectedRadius = static_cast<float>(*package->itemCornerRadiusDip);
                }
                if (package->resolvedLayout) {
                    const auto &layoutResult = *package->resolvedLayout;
                    const auto &direction = layout == "vertical" ? layoutResult.vertical : layoutResult.horizontal;
                    resolved.layout = direction;
                    resolved.fitContent = layoutResult.fitContent;
                    resolved.expandRows = std::clamp(layoutResult.expandRows, 2u, 8u);
                    resolved.shadow = layoutResult.shadow.value_or("normal");
                    resolved.pageArrows = layoutResult.pageArrows;
                    resolved.expandPanel = layoutResult.expandPanel;
                    resolved.tokens.radius = static_cast<float>(layoutResult.cornerRadiusDip);
                    resolved.tokens.borderWidth = static_cast<float>(layoutResult.borderWidthDip);
                    resolved.tokens.candidateRadius = resolved.tokens.selectedRadius = static_cast<float>(layoutResult.itemCornerRadiusDip);
                    if (direction.padding) {
                        // em 内边距由候选呈现器拿到实际字号后换算。
                        if (!direction.padding->em) resolved.tokens.pad = static_cast<float>(direction.padding->value);
                    }
                }
                // decorationImage is the manifest's decoration.image, or else an image preview, and is set only for a decorated package.
                if (!package->decorationImage.empty())
                {
                    resolved.decorationPath = (skinsRoot / package->id / package->decorationImage).string();
                }
                if (package->background && package->background->opacity > 0.0)
                {
                    resolved.backgroundPath = (skinsRoot / package->id / package->background->image).string();
                    resolved.backgroundFit = package->background->fit;
                    resolved.backgroundOpacity = package->background->opacity;
                }
            }
        }
        return resolved;
    }
}
} // namespace msime::mac
