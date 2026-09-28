// Adapted from MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.
#include "CandidateSkin.h"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <system_error>
#include <unordered_map>
#include <utility>

namespace msime::mac
{
namespace
{
std::filesystem::path ConfiguredSkinsRoot;

constexpr Rgba Rgb(unsigned rgb, float alpha = 1.0f)
{
    return {((rgb >> 16) & 0xFFu) / 255.0f, ((rgb >> 8) & 0xFFu) / 255.0f, (rgb & 0xFFu) / 255.0f, alpha};
}

float LinearChannel(float component)
{
    return component <= 0.04045f ? component / 12.92f : std::pow((component + 0.055f) / 1.055f, 2.4f);
}

float RelativeLuminance(Rgba color)
{
    return (0.2126f * LinearChannel(color.r)) + (0.7152f * LinearChannel(color.g)) + (0.0722f * LinearChannel(color.b));
}

Rgba ContrastingText(Rgba fill, Rgba fallback)
{
    if (fill.a < 0.85f)
    {
        return fallback;
    }
    return RelativeLuminance(fill) < 0.45f ? Rgba{1.0f, 1.0f, 1.0f, 1.0f} : Rgba{0.1f, 0.1f, 0.1f, 1.0f};
}

SkinTokens FluentTokens(bool dark)
{
    SkinTokens tokens;
    if (dark)
    {
        tokens.surface = Rgb(0x202020);
        tokens.border = Rgb(0x9B9B9B, 0x2E / 255.0f);
        tokens.text = Rgb(0xE9E8E8);
        tokens.number = Rgb(0xE9E8E8, 0.616f);
        tokens.selected = Rgb(0x3E3E3E, 0.725f);
        tokens.hover = Rgb(0x414141);
    }
    else
    {
        tokens.surface = Rgb(0xFFFFFF);
        tokens.border = Rgba{0.0f, 0.0f, 0.0f, 0.12f};
        tokens.text = Rgb(0x1A1A1A);
        tokens.number = Rgb(0x1A1A1A, 0.55f);
        tokens.selected = Rgb(0xE8E8E8);
        tokens.hover = Rgb(0xECECEC);
    }
    tokens.selectedText = tokens.text;
    tokens.selectedHover = tokens.hover;
    tokens.accent = Rgb(0x6B69D6);
    tokens.radius = 6.0f;
    tokens.candidateRadius = 4.0f;
    tokens.selectedRadius = 4.0f;
    tokens.borderWidth = 1.5f;
    tokens.pad = 5.0f;
    tokens.showSelectedBar = true;
    return tokens;
}

SkinTokens WeChatTokens(bool dark)
{
    SkinTokens tokens = FluentTokens(dark);
    tokens.surface = dark ? Rgb(0x151515) : Rgb(0xF7F7F7);
    tokens.border = dark ? Rgb(0x292929) : Rgb(0xDEDEDE);
    tokens.text = dark ? Rgb(0xB7B7B7) : Rgb(0x333333);
    tokens.number = dark ? Rgb(0x858585) : Rgb(0x757575);
    tokens.accent = Rgb(0x07C160);
    tokens.selected = Rgb(0x07C160);
    tokens.selectedText = Rgb(0xFFFFFF);
    tokens.hover = Rgb(0x07C160, dark ? 0.32f : 0.14f);
    tokens.selectedHover = tokens.selected;
    tokens.radius = 5.0f;
    // Windows candidate_presenter.cpp keeps the default 4px itemRadius for wechat and paints selected, pressed and hover rows with that one radius.
    tokens.candidateRadius = 4.0f;
    tokens.selectedRadius = 4.0f;
    tokens.borderWidth = 1.0f;
    tokens.pad = 2.0f;
    tokens.showSelectedBar = false;
    return tokens;
}

SkinTokens GraphiteTokens(bool dark)
{
    SkinTokens tokens = FluentTokens(dark);
    if (dark)
    {
        tokens.surface = Rgb(0x1C1F23);
        tokens.border = Rgb(0x30353B);
        tokens.text = Rgb(0xAEB6C2);
        tokens.number = Rgb(0x707987);
        tokens.selected = Rgba{0.0f, 0.0f, 0.0f, 0.0f};
        tokens.selectedText = Rgb(0xF1F3F5);
        tokens.hover = Rgba{1.0f, 1.0f, 1.0f, 0.055f};
        tokens.selectedHover = tokens.hover;
        tokens.accent = Rgb(0x8993A0);
    }
    else
    {
        tokens.surface = Rgb(0xFBFBFC);
        tokens.border = Rgb(0xE2E5E9);
        tokens.text = Rgb(0x586476);
        tokens.number = Rgb(0x8993A1);
        tokens.selected = Rgba{0.0f, 0.0f, 0.0f, 0.0f};
        tokens.selectedText = Rgb(0x111827);
        tokens.hover = Rgba{31.0f / 255.0f, 41.0f / 255.0f, 55.0f / 255.0f, 0.055f};
        tokens.selectedHover = tokens.hover;
        tokens.accent = Rgb(0x5F6B7A);
    }
    tokens.radius = 3.0f;
    tokens.candidateRadius = 2.0f;
    tokens.selectedRadius = 2.0f;
    tokens.borderWidth = 1.0f;
    tokens.pad = 5.0f;
    tokens.showSelectedBar = false;
    return tokens;
}

SkinTokens WillowGreenTokens(bool dark)
{
    SkinTokens tokens = FluentTokens(dark);
    if (dark)
    {
        tokens.surface = Rgb(0x2D2F2E);
        tokens.text = Rgb(0xD8DBD8);
        tokens.number = Rgb(0xA6ABA7);
        tokens.accent = Rgb(0x65C98D);
        tokens.selected = Rgb(0x65C98D);
        tokens.hover = Rgb(0x65C98D, 0.22f);
    }
    else
    {
        tokens.surface = Rgb(0xF4F5F3);
        tokens.text = Rgb(0x333333);
        tokens.number = Rgb(0x757575);
        tokens.accent = Rgb(0x58B980);
        tokens.selected = Rgb(0x58B980);
        tokens.hover = Rgb(0x58B980, 0.16f);
    }
    tokens.selectedHover = tokens.selected;
    tokens.selectedText = Rgb(0xFFFFFF);
    tokens.border = Rgba{0.0f, 0.0f, 0.0f, 0.0f};
    tokens.radius = 9.0f;
    // The willow_green CSS sets a 0 row radius and relies on the container clip-path; Windows candidate_presenter.cpp deliberately keeps a 4px itemRadius because the card does not clip its rows, and the macOS chrome view does not clip them either.
    tokens.candidateRadius = 4.0f;
    tokens.selectedRadius = 4.0f;
    tokens.borderWidth = 0.0f;
    tokens.pad = 0.0f;
    tokens.showSelectedBar = false;
    return tokens;
}

std::string Trim(std::string text)
{
    while (!text.empty() && std::isspace(static_cast<unsigned char>(text.front())))
    {
        text.erase(text.begin());
    }
    while (!text.empty() && std::isspace(static_cast<unsigned char>(text.back())))
    {
        text.pop_back();
    }
    return text;
}

void ApplyPackageColors(const SkinColors &colors, SkinTokens &tokens)
{
    if (const auto parsed = ParseCssColor(colors.accent))
    {
        tokens.accent = *parsed;
    }
    if (const auto parsed = ParseCssColor(colors.selected))
    {
        tokens.selected = *parsed;
    }
    if (const auto parsed = ParseCssColor(colors.hover))
    {
        tokens.hover = *parsed;
    }
    if (const auto parsed = ParseCssColor(colors.surface))
    {
        tokens.surface = *parsed;
    }
    if (const auto parsed = ParseCssColor(colors.border))
    {
        tokens.border = *parsed;
    }
    if (const auto parsed = ParseCssColor(colors.text))
    {
        tokens.text = *parsed;
    }
    if (const auto parsed = ParseCssColor(colors.number))
    {
        tokens.number = *parsed;
    }
    if (colors.showSelectedBar.has_value())
    {
        tokens.showSelectedBar = *colors.showSelectedBar;
    }
    tokens.selectedText = ContrastingText(tokens.selected, tokens.text);
}

bool IsContained(const std::filesystem::path &root, const std::filesystem::path &resource)
{
    if (root.empty()) return false;
    std::error_code ec;
    const auto canonicalRoot = std::filesystem::weakly_canonical(root, ec);
    if (ec) return false;
    const auto canonicalResource = std::filesystem::weakly_canonical(resource, ec);
    if (ec) return false;
    const auto relative = canonicalResource.lexically_relative(canonicalRoot);
    return !relative.empty() && !relative.is_absolute() &&
           std::none_of(relative.begin(), relative.end(), [](const auto &part) { return part == ".."; });
}

void ApplyToolbarStylesheet(const std::filesystem::path &skinsRoot, const SkinPackage &package, bool dark,
                            SkinTokens &tokens);
} // namespace

bool IsBuiltInSkinId(std::string_view id)
{
    return id == "fluent" || id == "wechat" || id == "graphite" || id == "willow_green";
}

bool IsSafeSkinId(std::string_view id)
{
    if (id.empty() || id.size() > 64 || !std::isalnum(static_cast<unsigned char>(id.front())))
    {
        return false;
    }
    return std::all_of(id.begin(), id.end(), [](unsigned char ch) {
        return std::islower(ch) || std::isdigit(ch) || ch == '.' || ch == '_' || ch == '-';
    });
}

std::string NormalizeSkinId(std::string_view id)
{
    if (id.empty()) return "willow_green";
    return IsSafeSkinId(id) ? std::string(id) : std::string("fluent");
}

const std::vector<SkinListEntry> &BuiltInSkinEntries()
{
    static const std::vector<SkinListEntry> entries = {
        {"fluent", "Fluent", true},
        {"wechat", "微信绿", true},
        {"graphite", "石墨 Graphite", true},
        {"willow_green", "杨柳青", true},
    };
    return entries;
}

SkinTokens BuiltInSkinTokens(std::string_view id, bool dark)
{
    if (id == "wechat")
    {
        return WeChatTokens(dark);
    }
    if (id == "graphite")
    {
        return GraphiteTokens(dark);
    }
    if (id == "willow_green")
    {
        return WillowGreenTokens(dark);
    }
    return FluentTokens(dark);
}

SkinTokens ToolbarSkinTokens(std::string_view id, bool dark)
{
    return ToolbarSkinTokens(id, dark, DefaultSkinsRoot());
}

std::optional<Rgba> ParseCssColor(std::string_view text)
{
    std::string value = Trim(std::string(text));
    if (value.empty() || value == "auto" || value == "none")
    {
        return std::nullopt;
    }
    if (value == "transparent")
    {
        return Rgba{0.0f, 0.0f, 0.0f, 0.0f};
    }
    if (value.rfind("rgba(", 0) == 0 || value.rfind("rgb(", 0) == 0)
    {
        const auto open = value.find('(');
        const auto close = value.rfind(')');
        if (open == std::string::npos || close == std::string::npos || close <= open || close != value.size() - 1)
        {
            return std::nullopt;
        }
        std::string inner = value.substr(open + 1, close - open - 1);
        for (char &ch : inner)
        {
            if (ch == ',' || ch == '/')
            {
                ch = ' ';
            }
        }
        std::istringstream stream(inner);
        float r = 0;
        float g = 0;
        float b = 0;
        float a = 1.0f;
        if (!(stream >> r >> g >> b))
        {
            return std::nullopt;
        }
        stream >> std::ws;
        if (!stream.eof()) {
            if (!(stream >> a)) return std::nullopt;
            stream >> std::ws;
        }
        if (!stream.eof() || !std::isfinite(r) || !std::isfinite(g) || !std::isfinite(b) || !std::isfinite(a) ||
            r < 0 || r > 255 || g < 0 || g > 255 || b < 0 || b > 255 || a < 0 || a > 1)
            return std::nullopt;
        return Rgba{r / 255.0f, g / 255.0f, b / 255.0f, a};
    }
    if (value.front() == '#')
    {
        value.erase(value.begin());
    }
    if (!std::all_of(value.begin(), value.end(), [](unsigned char ch) { return std::isxdigit(ch); }))
        return std::nullopt;
    auto hexByte = [](const std::string &hex) { return static_cast<int>(std::stoul(hex, nullptr, 16)); };
    try
    {
        if (value.size() == 3)
        {
            return Rgba{hexByte(std::string(2, value[0])) / 255.0f, hexByte(std::string(2, value[1])) / 255.0f,
                        hexByte(std::string(2, value[2])) / 255.0f, 1.0f};
        }
        if (value.size() == 6)
        {
            return Rgb(static_cast<unsigned>(std::stoul(value, nullptr, 16)));
        }
        if (value.size() == 8)
        {
            const unsigned long packed = std::stoul(value, nullptr, 16);
            return Rgb(static_cast<unsigned>((packed >> 8) & 0xFFFFFFu), static_cast<float>(packed & 0xFFu) / 255.0f);
        }
    }
    catch (...)
    {
    }
    return std::nullopt;
}

namespace
{
std::string Lowercase(std::string value)
{
    std::transform(value.begin(), value.end(), value.begin(),
                   [](unsigned char ch) { return static_cast<char>(std::tolower(ch)); });
    return value;
}

std::string StripCssComments(std::string text)
{
    std::size_t cursor = 0;
    while ((cursor = text.find("/*", cursor)) != std::string::npos)
    {
        const std::size_t end = text.find("*/", cursor + 2);
        if (end == std::string::npos)
        {
            text.erase(cursor);
            break;
        }
        text.erase(cursor, end + 2 - cursor);
    }
    return text;
}

std::optional<Rgba> FindCssColor(std::string value)
{
    value = Trim(value);
    if (const auto direct = ParseCssColor(value))
    {
        return direct;
    }
    for (const char *prefix : {"rgba(", "rgb("})
    {
        const std::size_t begin = value.find(prefix);
        if (begin != std::string::npos)
        {
            const std::size_t end = value.find(')', begin + std::char_traits<char>::length(prefix));
            if (end != std::string::npos)
            {
                if (const auto parsed = ParseCssColor(value.substr(begin, end - begin + 1)))
                {
                    return parsed;
                }
            }
        }
    }
    const std::size_t hash = value.find('#');
    if (hash != std::string::npos)
    {
        std::size_t end = hash + 1;
        while (end < value.size() && std::isxdigit(static_cast<unsigned char>(value[end]))) ++end;
        if (const auto parsed = ParseCssColor(value.substr(hash, end - hash)))
        {
            return parsed;
        }
    }
    return std::nullopt;
}

std::optional<float> ParseCssLength(std::string value)
{
    value = Trim(value);
    char *end = nullptr;
    const float number = std::strtof(value.c_str(), &end);
    if (end == value.c_str() || !std::isfinite(number) || number < 0.0f || number > 64.0f)
    {
        return std::nullopt;
    }
    const std::string unit = Lowercase(Trim(end));
    if (!unit.empty() && unit != "px")
    {
        return std::nullopt;
    }
    return number;
}

std::string ResolveCssVariable(std::string value, const std::unordered_map<std::string, std::string> &variables)
{
    for (int pass = 0; pass < 4; ++pass)
    {
        const std::size_t begin = value.find("var(");
        if (begin == std::string::npos) break;
        const std::size_t end = value.find(')', begin + 4);
        if (end == std::string::npos) break;
        std::string key = Trim(value.substr(begin + 4, end - begin - 4));
        const std::size_t fallback = key.find(',');
        std::string replacement;
        if (fallback != std::string::npos)
        {
            replacement = Trim(key.substr(fallback + 1));
            key = Trim(key.substr(0, fallback));
        }
        key = Lowercase(key);
        const auto found = variables.find(key);
        if (found != variables.end()) replacement = found->second;
        if (replacement.empty()) break;
        value.replace(begin, end - begin + 1, replacement);
    }
    return value;
}

void ApplyToolbarCssProperty(std::string property, std::string value, const std::string &selector,
                             const std::unordered_map<std::string, std::string> &variables, SkinTokens &tokens)
{
    property = Lowercase(Trim(property));
    value = ResolveCssVariable(Trim(value), variables);
    const std::string loweredSelector = Lowercase(selector);
    const bool hover = loweredSelector.find(":hover") != std::string::npos || loweredSelector.find("hover") != std::string::npos;
    const bool selected = loweredSelector.find(":active") != std::string::npos ||
                          loweredSelector.find("selected") != std::string::npos ||
                          loweredSelector.find("active") != std::string::npos;
    auto colorTarget = [&](const std::optional<Rgba> &color, const std::string &customProperty) {
        if (!color) return;
        if (customProperty == "accent") tokens.accent = *color;
        else if (customProperty == "selected") tokens.selected = *color;
        else if (customProperty == "hover") tokens.hover = *color;
        else if (customProperty == "surface") tokens.surface = *color;
        else if (customProperty == "border") tokens.border = *color;
        else if (customProperty == "text") tokens.text = *color;
    };
    if (!property.empty() && property.front() == '-')
    {
        static const std::pair<const char *, const char *> names[] = {
            {"--toolbar-accent", "accent"}, {"--ftb-accent", "accent"}, {"--accent-color", "accent"},
            {"--accent-strong", "accent"},   {"--toolbar-selected", "selected"}, {"--ftb-selected", "selected"},
            {"--toolbar-hover", "hover"},    {"--ftb-hover", "hover"}, {"--toolbar-bg", "surface"},
            {"--toolbar-background", "surface"}, {"--toolbar-surface", "surface"}, {"--ftb-bg", "surface"},
            {"--toolbar-border", "border"},  {"--ftb-border", "border"}, {"--toolbar-text", "text"},
            {"--ftb-text", "text"},
        };
        for (const auto &[name, target] : names)
        {
            if (property == name)
            {
                colorTarget(FindCssColor(value), target);
                break;
            }
        }
        if (property == "--toolbar-radius" || property == "--ftb-radius")
        {
            if (const auto length = ParseCssLength(value)) tokens.radius = *length;
        }
        if (property == "--toolbar-border-width" || property == "--ftb-border-width")
        {
            if (const auto length = ParseCssLength(value)) tokens.borderWidth = *length;
        }
        if (property == "--toolbar-padding" || property == "--ftb-padding")
        {
            if (const auto length = ParseCssLength(value)) tokens.pad = *length;
        }
        return;
    }
    if (property == "color")
    {
        colorTarget(FindCssColor(value), "text");
    }
    else if (property == "background" || property == "background-color")
    {
        colorTarget(FindCssColor(value), selected ? "selected" : hover ? "hover" : "surface");
    }
    else if (property == "border-color" || property == "outline-color")
    {
        colorTarget(FindCssColor(value), "border");
    }
    else if (property == "border")
    {
        colorTarget(FindCssColor(value), "border");
        const std::string width = Trim(value.substr(0, value.find_first_of(" \\t")));
        if (const auto length = ParseCssLength(width)) tokens.borderWidth = *length;
    }
    else if (property == "accent-color")
    {
        colorTarget(FindCssColor(value), "accent");
    }
    else if (property == "border-radius")
    {
        if (const auto length = ParseCssLength(value)) tokens.radius = *length;
    }
    else if (property == "border-width")
    {
        if (const auto length = ParseCssLength(value)) tokens.borderWidth = *length;
    }
    else if (property == "padding" || property == "padding-left" || property == "padding-inline")
    {
        if (const auto length = ParseCssLength(value)) tokens.pad = *length;
    }
}

void ApplyToolbarStylesheet(const std::filesystem::path &skinsRoot, const SkinPackage &package, bool dark,
                            SkinTokens &tokens)
{
    constexpr std::size_t kMaxToolbarStylesheetBytes = 65536;
    if (package.toolbarStylesheet.empty()) return;
    const std::filesystem::path stylesheet = skinsRoot / package.id / package.toolbarStylesheet;
    std::error_code ec;
    if (!IsContained(skinsRoot / package.id, stylesheet) || !std::filesystem::is_regular_file(stylesheet, ec) || ec)
        return;
    std::ifstream stream(stylesheet);
    if (!stream) return;
    std::string stylesheetBytes(kMaxToolbarStylesheetBytes + 1, '\0');
    stream.read(stylesheetBytes.data(), static_cast<std::streamsize>(stylesheetBytes.size()));
    if (stream.bad() || stream.gcount() > static_cast<std::streamsize>(kMaxToolbarStylesheetBytes)) return;
    stylesheetBytes.resize(static_cast<std::size_t>(stream.gcount()));
    const std::string css = StripCssComments(std::move(stylesheetBytes));
    std::unordered_map<std::string, std::string> variables;
    std::size_t cursor = 0;
    while (cursor < css.size())
    {
        const std::size_t open = css.find('{', cursor);
        if (open == std::string::npos) break;
        const std::size_t close = css.find('}', open + 1);
        if (close == std::string::npos) break;
        const std::string selector = Trim(css.substr(cursor, open - cursor));
        cursor = close + 1;
        if (selector.empty() || selector.front() == '@') continue;
        const std::string lowered = Lowercase(selector);
        if ((lowered.find("dark") != std::string::npos && !dark) ||
            (lowered.find("light") != std::string::npos && dark))
            continue;
        std::vector<std::pair<std::string, std::string>> declarations;
        std::stringstream block(css.substr(open + 1, close - open - 1));
        std::string declaration;
        while (std::getline(block, declaration, ';'))
        {
            const std::size_t colon = declaration.find(':');
            if (colon == std::string::npos) continue;
            std::string property = Trim(declaration.substr(0, colon));
            std::string value = Trim(declaration.substr(colon + 1));
            if (property.empty() || value.empty()) continue;
            if (value.size() >= 10 && Lowercase(value.substr(value.size() - 10)) == "!important")
                value = Trim(value.substr(0, value.size() - 10));
            declarations.emplace_back(std::move(property), std::move(value));
        }
        for (const auto &[property, value] : declarations)
            if (!property.empty() && property.front() == '-') variables[Lowercase(Trim(property))] = value;
        for (const auto &[property, value] : declarations)
            ApplyToolbarCssProperty(property, value, selector, variables, tokens);
    }
    tokens.selectedText = ContrastingText(tokens.selected, tokens.text);
}
} // namespace

SkinTokens ToolbarSkinTokens(std::string_view id, bool dark, const std::filesystem::path &skinsRoot)
{
    const bool builtin = IsBuiltInSkinId(id);
    SkinTokens tokens = BuiltInSkinTokens(builtin ? id : "fluent", dark);
    // The default toolbar accent is intentionally lighter than the Fluent
    // candidate-card accent. External packages may override this native
    // palette only through the primitive properties understood above; CSS
    // layout, scripts, images and effects never enter the AppKit view.
    if (!builtin || id == "fluent") tokens.accent = Rgb(0x8E8CD8);
    if (!builtin)
    {
        std::string error;
        if (const auto package = LoadSkinPackage(skinsRoot, std::string(id), &error))
            ApplyToolbarStylesheet(skinsRoot, *package, dark, tokens);
    }
    return tokens;
}

// LoadSkinPackage and ScanSkinCatalog live in SkinManifestBridge.mm: manifests are validated by the shared client-core loader over the host C ABI.

bool SupportsSkin(const SkinPackage &package, std::string_view layout, std::string_view theme)
{
    const auto contains = [](const std::vector<std::string> &values, std::string_view value) {
        return std::find(values.begin(), values.end(), value) != values.end();
    };
    return contains(package.layouts, layout) && contains(package.themes, theme);
}

std::vector<SkinListEntry> ListSkins(const std::filesystem::path &skinsRoot)
{
    std::vector<SkinListEntry> entries = BuiltInSkinEntries();
    const SkinCatalog catalog = ScanSkinCatalog(skinsRoot);
    for (const SkinPackage &package : catalog.packages)
    {
        entries.push_back({package.id, package.name, false});
    }
    return entries;
}

ResolvedSkin ResolveSkin(std::string_view id, bool dark, const std::filesystem::path &skinsRoot)
{
    return ResolveSkin(id, dark, skinsRoot, {}, {});
}

ResolvedSkin ResolveSkin(std::string_view id, bool dark, const std::filesystem::path &skinsRoot,
                         std::string_view layout, std::string_view theme)
{
    const bool defaultRequested = id.empty();
    const std::string normalized = NormalizeSkinId(id);
    ResolvedSkin resolved;
    resolved.id = defaultRequested ? "willow_green" : "fluent";
    resolved.name = defaultRequested ? "杨柳青" : "Fluent";
    resolved.tokens = BuiltInSkinTokens(defaultRequested ? "willow_green" : "fluent", dark);
    if (IsBuiltInSkinId(normalized))
    {
        resolved.id = normalized;
        for (const SkinListEntry &entry : BuiltInSkinEntries())
        {
            if (entry.id == normalized)
            {
                resolved.name = entry.name;
                break;
            }
        }
        resolved.tokens = BuiltInSkinTokens(normalized, dark);
        return resolved;
    }
    std::optional<SkinPackage> package = LoadSkinPackage(skinsRoot, normalized);
    if (!package)
    {
        return resolved;
    }
    if ((!layout.empty() || !theme.empty()) && !SupportsSkin(*package, layout, theme))
    {
        return resolved;
    }
    resolved.id = package->id;
    resolved.name = package->name;
    resolved.tokens = BuiltInSkinTokens(package->base, dark);
    ApplyPackageColors(dark ? package->dark : package->light, resolved.tokens);
    resolved.decorationTopDip = package->decorationTopDip;
    resolved.decorationWidthDip = package->decorationWidthDip;
    resolved.minWidthDip = package->minWidthDip;
    if (!package->preview.empty())
    {
        resolved.decorationPath = (skinsRoot / package->id / package->preview).string();
    }
    return resolved;
}

std::filesystem::path DefaultSkinsRoot()
{
    if (!ConfiguredSkinsRoot.empty())
    {
        return ConfiguredSkinsRoot;
    }
    const char *home = std::getenv("HOME");
    if (home == nullptr || home[0] == '\0')
    {
        return {};
    }
    return std::filesystem::path(home) / "Library" / "Application Support" / "app.msime.client" / "skins";
}

void SetDefaultSkinsRoot(std::filesystem::path root)
{
    ConfiguredSkinsRoot = std::move(root);
}
} // namespace msime::mac
