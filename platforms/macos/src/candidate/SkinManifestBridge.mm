// External skin manifests are validated by the shared client-core loader over the host C ABI, the one the settings page and the other hosts use. Windows parses skin.toml with toml++ (server/src/skin/candidate_skin_catalog.cpp), so the loader has to be a full TOML 1.0 parser too; a hand-written subset here would draw Fluent for packages the settings page lists as valid.
#include "CandidateSkin.h"

#import <Foundation/Foundation.h>

#include "msime_client.h"

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
    char *raw = call(static_cast<const uint8_t *>(request.bytes), request.length);
    if (raw == nullptr)
    {
        reply.error = "no response";
        return reply;
    }
    NSData *data = [NSData dataWithBytes:raw length:std::char_traits<char>::length(raw)];
    msime_client_string_free(raw);
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

std::vector<std::string> StringArrayField(NSDictionary *object, NSString *key)
{
    std::vector<std::string> values;
    NSArray *items = object[key];
    if (![items isKindOfClass:NSArray.class])
    {
        return values;
    }
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
    NSNumber *bar = palette[@"showSelectedBar"];
    if ([bar isKindOfClass:NSNumber.class])
    {
        colors.showSelectedBar = bar.boolValue;
    }
    return colors;
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
        return "decoration 尺寸无效";
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
            char *raw = msime_client_theme_catalog();
            if (raw == nullptr)
            {
                return result;
            }
            NSData *data = [NSData dataWithBytes:raw length:std::char_traits<char>::length(raw)];
            msime_client_string_free(raw);
            NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
            NSDictionary *value = [envelope isKindOfClass:NSDictionary.class] && [envelope[@"ok"] isEqual:@YES]
                                      ? envelope[@"value"]
                                      : nil;
            NSArray *themes = [value isKindOfClass:NSDictionary.class] ? value[@"themes"] : nil;
            if (![themes isKindOfClass:NSArray.class])
            {
                return result;
            }
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
                         const std::filesystem::path &skinsRoot)
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
        if (!custom.candidateSkin.empty() && IsSafeSkinId(custom.candidateSkin) && !IsGlobalThemeId(custom.candidateSkin))
        {
            customTheme[@"candidate_skin"] = @(custom.candidateSkin.c_str());
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
            ApplyThemeColor(palette, @"accent", tokens.accent);
            // The native selection is a solid fill of the accent, so a null `selected` follows whatever accent was drawn (a picked one included) rather than the native green.
            tokens.selected = tokens.accent;
            ApplyThemeColor(palette, @"selected", tokens.selected);
            const bool selectedText = ApplyThemeColor(palette, @"selected_text", tokens.selectedText);
            const bool selectedNumber = ApplyThemeColor(palette, @"selected_number", tokens.selectedNumber);
            // Over a system base the shared resolver leaves these two to the platform, and the native white is only readable on the native solid accent: a package's translucent `selected` or a light picked colour needs them from the fill that is actually drawn.
            DeriveSelectedForegrounds(tokens, !selectedText, !selectedNumber);
            ApplyThemeColor(palette, @"hover", tokens.hover);
            tokens.selectedHover = tokens.selected;
            NSNumber *bar = palette[@"show_selected_bar"];
            if ([bar isKindOfClass:NSNumber.class])
            {
                tokens.showSelectedBar = bar.boolValue;
            }
        }
        NSString *drawn = value[@"candidate_skin"];
        if ([drawn isKindOfClass:NSString.class] && drawn.length > 0)
        {
            resolved.candidateSkin = drawn.UTF8String ?: "";
            // The decoration and the minimum width still come from the package itself; the theme only says whether it is drawn.
            if (const auto package = LoadSkinPackage(skinsRoot, resolved.candidateSkin))
            {
                resolved.name = package->name;
                resolved.decorationTopDip = package->decorationTopDip;
                resolved.decorationWidthDip = package->decorationWidthDip;
                resolved.minWidthDip = package->minWidthDip;
                if (!package->preview.empty())
                {
                    resolved.decorationPath = (skinsRoot / package->id / package->preview).string();
                }
            }
        }
        return resolved;
    }
}
} // namespace msime::mac
