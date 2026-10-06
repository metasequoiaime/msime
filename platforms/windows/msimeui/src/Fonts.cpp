#include "msimeui/Fonts.h"

#include <dwrite_2.h>
#include <map>
#include <mutex>
#include <utility>
#include <vector>
#include <wrl/client.h>

namespace msimeui
{
using Microsoft::WRL::ComPtr;

namespace
{
bool FontFamilyExists(IDWriteFactory *factory, const wchar_t *name)
{
    if (!factory || !name)
    {
        return false;
    }
    ComPtr<IDWriteFontCollection> fonts;
    if (FAILED(factory->GetSystemFontCollection(fonts.GetAddressOf())) || !fonts)
    {
        return false;
    }
    UINT32 index = 0;
    BOOL exists = FALSE;
    return SUCCEEDED(fonts->FindFamilyName(name, &index, &exists)) && exists;
}

IDWriteFactory *SharedFactory()
{
    // Called from several window threads, so the lazy creation needs a lock of
    // its own; the factory is never reset, so the returned pointer stays valid.
    static std::mutex mutex;
    static ComPtr<IDWriteFactory> factory;
    std::lock_guard<std::mutex> lock(mutex);
    if (!factory)
    {
        DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
                            reinterpret_cast<IUnknown **>(factory.GetAddressOf()));
    }
    return factory.Get();
}

constexpr wchar_t kFluentIconsFamily[] = L"Segoe Fluent Icons";
constexpr wchar_t kMdl2IconsFamily[] = L"Segoe MDL2 Assets";

ComPtr<IDWriteFont> FindFont(IDWriteFactory *factory, const wchar_t *name)
{
    if (!factory || !name)
    {
        return nullptr;
    }
    ComPtr<IDWriteFontCollection> fonts;
    if (FAILED(factory->GetSystemFontCollection(fonts.GetAddressOf())) || !fonts)
    {
        return nullptr;
    }
    UINT32 index = 0;
    BOOL exists = FALSE;
    if (FAILED(fonts->FindFamilyName(name, &index, &exists)) || !exists)
    {
        return nullptr;
    }
    ComPtr<IDWriteFontFamily> family;
    if (FAILED(fonts->GetFontFamily(index, family.GetAddressOf())))
    {
        return nullptr;
    }
    ComPtr<IDWriteFont> font;
    if (FAILED(family->GetFirstMatchingFont(DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
                                            DWRITE_FONT_STYLE_NORMAL, font.GetAddressOf())))
    {
        return nullptr;
    }
    return font;
}

struct IconFontEntry
{
    const wchar_t *family = nullptr;
    bool isMdl2 = false;
    ComPtr<IDWriteFont> font;
};

// Installed icon fonts, in preference order. Kept once the probe finds one, so
// a font installed while the process runs is only picked up after a restart. An
// empty result is not kept: a transient DirectWrite failure on the first call
// would otherwise degrade every icon to its text label for the whole process.
// Callers must hold the ResolveIconGlyph mutex, its only caller.
const std::vector<IconFontEntry> &IconFonts()
{
    static std::vector<IconFontEntry> entries;
    if (!entries.empty())
    {
        return entries;
    }
    IDWriteFactory *factory = SharedFactory();
    if (ComPtr<IDWriteFont> fluent = FindFont(factory, kFluentIconsFamily))
    {
        entries.push_back({kFluentIconsFamily, false, std::move(fluent)});
    }
    if (ComPtr<IDWriteFont> mdl2 = FindFont(factory, kMdl2IconsFamily))
    {
        entries.push_back({kMdl2IconsFamily, true, std::move(mdl2)});
    }
    return entries;
}

bool FontHasCodepoint(IDWriteFont *font, wchar_t codepoint)
{
    if (!font || codepoint == 0)
    {
        return false;
    }
    BOOL exists = FALSE;
    return SUCCEEDED(font->HasCharacter(codepoint, &exists)) && exists;
}
} // namespace

const wchar_t *UiFontFallbackFamily()
{
    return L"Microsoft YaHei";
}

const wchar_t *UiFontFamily()
{
    IDWriteFactory *factory = SharedFactory();
    static const wchar_t *family =
        FontFamilyExists(factory, L"Noto Sans SC") ? L"Noto Sans SC" : UiFontFallbackFamily();
    return family;
}

IconGlyph ResolveIconGlyph(wchar_t fluentCodepoint, wchar_t mdl2Codepoint)
{
    static std::mutex mutex;
    static std::map<std::pair<wchar_t, wchar_t>, IconGlyph> cache;

    const std::pair<wchar_t, wchar_t> key{fluentCodepoint, mdl2Codepoint};
    std::lock_guard<std::mutex> lock(mutex);
    const auto cached = cache.find(key);
    if (cached != cache.end())
    {
        return cached->second;
    }

    IconGlyph resolved;
    const std::vector<IconFontEntry> &fonts = IconFonts();
    if (fonts.empty())
    {
        // No icon font found yet. Report the text fallback but do not cache it,
        // so a later call can still pick one up once DirectWrite recovers.
        return resolved;
    }
    for (const IconFontEntry &entry : fonts)
    {
        const wchar_t codepoint = (entry.isMdl2 && mdl2Codepoint != 0) ? mdl2Codepoint : fluentCodepoint;
        if (FontHasCodepoint(entry.font.Get(), codepoint))
        {
            resolved.family = entry.family;
            resolved.codepoint = codepoint;
            break;
        }
    }
    cache.emplace(key, resolved);
    return resolved;
}

void ApplyUiFontFallback(IDWriteFactory *factory, IDWriteTextFormat *format)
{
    if (!factory || !format)
    {
        return;
    }
    ComPtr<IDWriteFactory2> factory2;
    if (FAILED(factory->QueryInterface(IID_PPV_ARGS(&factory2))))
    {
        return;
    }

    static ComPtr<IDWriteFontFallback> cachedFallback;
    static IDWriteFactory *cachedFactory = nullptr;
    if (!cachedFallback || cachedFactory != factory)
    {
        ComPtr<IDWriteFontFallbackBuilder> builder;
        if (FAILED(factory2->CreateFontFallbackBuilder(builder.GetAddressOf())))
        {
            return;
        }

        // Only CJK: mapping all of Unicode to Noto/YaHei replaces system fallback
        // and makes Segoe UI Emoji (COLR) tofu.
        static const DWRITE_UNICODE_RANGE kCjk[] = {
            {0x2E80, 0x2EFF},   {0x2F00, 0x2FDF},   {0x3000, 0x303F},   {0x3040, 0x30FF},   {0x3100, 0x312F},
            {0x31A0, 0x31BF},   {0x31C0, 0x31EF},   {0x3200, 0x32FF},   {0x3300, 0x33FF},   {0x3400, 0x4DBF},
            {0x4E00, 0x9FFF},   {0xF900, 0xFAFF},   {0xFF00, 0xFFEF},   {0x20000, 0x2A6DF}, {0x2A700, 0x2B73F},
            {0x2B740, 0x2B81F}, {0x2B820, 0x2CEAF}, {0x2CEB0, 0x2EBEF}, {0x30000, 0x3134F},
        };
        WCHAR const *cjkFamilies[] = {L"Noto Sans SC", UiFontFallbackFamily()};
        if (FAILED(builder->AddMapping(kCjk, static_cast<UINT32>(sizeof(kCjk) / sizeof(kCjk[0])), cjkFamilies, 2,
                                       nullptr, nullptr, nullptr, 1.0f)))
        {
            return;
        }

        if (FontFamilyExists(factory, L"Segoe UI Emoji"))
        {
            static const DWRITE_UNICODE_RANGE kEmoji[] = {
                {0x200D, 0x200D},   {0x2600, 0x27BF},   {0xFE00, 0xFE0F},   {0x1F000, 0x1F02F},
                {0x1F0A0, 0x1F0FF}, {0x1F300, 0x1FAFF}, {0x1F1E6, 0x1F1FF},
            };
            WCHAR const *emojiFamilies[] = {L"Segoe UI Emoji"};
            builder->AddMapping(kEmoji, static_cast<UINT32>(sizeof(kEmoji) / sizeof(kEmoji[0])), emojiFamilies, 1,
                                nullptr, nullptr, nullptr, 1.0f);
        }

        ComPtr<IDWriteFontFallback> customFallback;
        if (FAILED(builder->CreateFontFallback(customFallback.GetAddressOf())))
        {
            return;
        }
        cachedFallback = customFallback;
        cachedFactory = factory;
    }

    ComPtr<IDWriteTextFormat1> format1;
    if (SUCCEEDED(format->QueryInterface(IID_PPV_ARGS(&format1))))
    {
        format1->SetFontFallback(cachedFallback.Get());
    }
}

void ApplyFontFallback(IDWriteFactory *factory, IDWriteTextFormat *format, const std::vector<std::wstring> &families)
{
    if (!factory || !format)
        return;
    ComPtr<IDWriteFactory2> factory2;
    ComPtr<IDWriteTextFormat1> format1;
    if (FAILED(factory->QueryInterface(IID_PPV_ARGS(&factory2))) ||
        FAILED(format->QueryInterface(IID_PPV_ARGS(&format1))))
        return;
    ComPtr<IDWriteFontFallbackBuilder> builder;
    if (FAILED(factory2->CreateFontFallbackBuilder(&builder)))
        return;
    std::vector<const wchar_t *> names;
    names.reserve(families.size());
    for (const auto &family : families)
        if (!family.empty())
            names.push_back(family.c_str());
    const DWRITE_UNICODE_RANGE range = {0, 0x10FFFF};
    if (!names.empty() && FAILED(builder->AddMapping(&range, 1, names.data(), static_cast<UINT32>(names.size()))))
        return;
    ComPtr<IDWriteFontFallback> systemFallback;
    if (SUCCEEDED(factory2->GetSystemFontFallback(&systemFallback)))
        builder->AddMappings(systemFallback.Get());
    ComPtr<IDWriteFontFallback> fallback;
    if (SUCCEEDED(builder->CreateFontFallback(&fallback)))
        format1->SetFontFallback(fallback.Get());
}
} // namespace msimeui
