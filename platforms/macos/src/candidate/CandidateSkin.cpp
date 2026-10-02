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

SkinTokens NativeCandidateTokens(bool dark)
{
    // design-tokens-desktop §1.2, the macOS column: a translucent system card, the brand accent as a solid selection with white text, and hairline borders and hovers drawn in the text colour's alpha.
    SkinTokens tokens;
    if (dark)
    {
        tokens.surface = Rgb(0x28282A, 0.97f);
        tokens.border = Rgba{1.0f, 1.0f, 1.0f, 0.12f};
        tokens.text = Rgb(0xF5F5F7);
        tokens.number = Rgb(0x98989D);
        tokens.accent = Rgb(0x5FBF84);
        tokens.hover = Rgba{1.0f, 1.0f, 1.0f, 0.10f};
    }
    else
    {
        tokens.surface = Rgb(0xFFFFFF, 0.97f);
        tokens.border = Rgba{0.0f, 0.0f, 0.0f, 0.12f};
        tokens.text = Rgb(0x1D1D1F);
        tokens.number = Rgb(0x6E6E73);
        tokens.accent = Rgb(0x2C7A4B);
        tokens.hover = Rgba{0.0f, 0.0f, 0.0f, 0.06f};
    }
    tokens.selected = tokens.accent;
    tokens.selectedHover = tokens.selected;
    tokens.selectedText = Rgb(0xFFFFFF);
    tokens.selectedNumber = Rgb(0xFFFFFF, 0.82f);
    return tokens;
}

void DeriveSelectedForegrounds(SkinTokens &tokens, bool text, bool number)
{
    if (text)
    {
        tokens.selectedText = ContrastingText(tokens.selected, tokens.text);
    }
    if (number)
    {
        const Rgba contrast = ContrastingText(tokens.selected, tokens.number);
        tokens.selectedNumber = tokens.selected.a < 0.85f ? tokens.number : Rgba{contrast.r, contrast.g, contrast.b, 0.82f};
    }
}

ResolvedSkin StyledCandidateSkin(ResolvedSkin skin, const CandidateWindowStyle &style)
{
    SkinTokens &tokens = skin.tokens;
    // Only a radius the user chose rounds the rows down with the card; a skin package's own radius keeps the host row radius it has always drawn.
    if (style.cornerRadius)
    {
        tokens.radius = static_cast<float>(std::clamp(*style.cornerRadius, 0.0, 32.0));
        tokens.candidateRadius = std::min(tokens.candidateRadius, tokens.radius);
        tokens.selectedRadius = std::min(tokens.selectedRadius, tokens.radius);
    }
    const float opacity = static_cast<float>(std::clamp(style.opacity, 0.0, 1.0));
    tokens.surface.a *= opacity;
    tokens.border.a *= opacity;
    skin.backgroundOpacity *= opacity;
    const double scale = style.scale > 0.0 ? style.scale : 1.0;
    tokens.radius = static_cast<float>(tokens.radius * scale);
    tokens.candidateRadius = static_cast<float>(tokens.candidateRadius * scale);
    tokens.selectedRadius = static_cast<float>(tokens.selectedRadius * scale);
    tokens.pad = static_cast<float>(tokens.pad * scale);
    skin.decorationTopDip *= scale;
    skin.decorationWidthDip *= scale;
    skin.minWidthDip *= scale;
    return skin;
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

SkinTokens ToolbarSkinTokens(const ResolvedSkin &skin, const std::filesystem::path &skinsRoot)
{
    // The toolbar and the menus derive from the candidate palette (THEME_CONTRACT §3). A drawn package may still restyle its toolbar through its manifest `[toolbar]` table and the primitive stylesheet properties understood above; CSS layout, scripts, images and effects never enter the AppKit view.
    SkinTokens tokens = skin.tokens;
    // A package's card radius is in skin.tokens.radius; the toolbar starts from the native radius instead and takes only the package's toolbar radius.
    tokens.radius = NativeCandidateTokens(skin.dark).radius;
    tokens.translation.reset();
    tokens.divider.reset();
    if (!skin.candidateSkin.empty())
    {
        std::string error;
        if (const auto package = LoadSkinPackage(skinsRoot, skin.candidateSkin, &error))
        {
            // The manifest's `[toolbar]` first, so the stylesheet below still wins over it.
            const SkinToolbarColors &colors = skin.dark ? package->toolbar.dark : package->toolbar.light;
            const auto apply = [](const std::string &value, Rgba &slot) {
                if (const auto color = ParseCssColor(value)) slot = *color;
            };
            apply(colors.background, tokens.surface);
            apply(colors.border, tokens.border);
            apply(colors.icon, tokens.text);
            apply(colors.hover, tokens.hover);
            if (const auto divider = ParseCssColor(colors.divider)) tokens.divider = *divider;
            // colors.handle has no target: the macOS toolbar's drag handle is the logo, which keeps the brand mark's colours.
            if (package->toolbar.cornerRadiusDip) tokens.radius = static_cast<float>(*package->toolbar.cornerRadiusDip);
            ApplyToolbarStylesheet(skinsRoot, *package, skin.dark, tokens);
        }
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

// Only external packages: the built-in looks are global themes now, listed by ThemeCatalog().
std::vector<SkinListEntry> ListSkins(const std::filesystem::path &skinsRoot)
{
    std::vector<SkinListEntry> entries;
    const SkinCatalog catalog = ScanSkinCatalog(skinsRoot);
    for (const SkinPackage &package : catalog.packages)
    {
        entries.push_back({package.id, package.name, false});
    }
    return entries;
}

// ThemeCatalog and ResolveSkin live in SkinManifestBridge.mm as well: both are answered by the shared client-core theme module over the host C ABI.

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
    // The same directory as MSIMEDefaultClientStateDirectory in RuntimeOptions.h.
    return std::filesystem::path(home) / "Library" / "Application Support" / "app.msime.macos" / "skins";
}

void SetDefaultSkinsRoot(std::filesystem::path root)
{
    ConfiguredSkinsRoot = std::move(root);
}
} // namespace msime::mac
