#pragma once

#include <windows.h>
#include <d2d1.h>
#include <dwrite_2.h>
#include <wrl/client.h>
#include <algorithm>
#include <atomic>
#include <filesystem>
#include <iterator>
#include <new>
#include <string>
#if defined(MSIME_WINDOWS7_EMOJI_SHAPING)
#include "Windows7EmojiShaping.h"
#endif

namespace msime::windows::compat {
namespace emoji_font_detail {
using Microsoft::WRL::ComPtr;

// 私有集合只读取程序旁随包分发的字体，不向系统字体目录写入或注册字体。
class Enumerator final : public IDWriteFontFileEnumerator {
public:
  Enumerator(IDWriteFactory* factory, const wchar_t* path) : factory_(factory), path_(path) {}
  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** result) override {
    if (!result) return E_POINTER;
    *result = nullptr;
    if (iid != __uuidof(IUnknown) && iid != __uuidof(IDWriteFontFileEnumerator)) return E_NOINTERFACE;
    *result = static_cast<IDWriteFontFileEnumerator*>(this); AddRef(); return S_OK;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return ++references_; }
  ULONG STDMETHODCALLTYPE Release() override { const auto count = --references_; if (!count) delete this; return count; }
  HRESULT STDMETHODCALLTYPE MoveNext(BOOL* current) override {
    if (!current) return E_POINTER;
    *current = FALSE;
    if (advanced_) { file_.Reset(); return S_OK; }
    advanced_ = true;
    const auto result = factory_->CreateFontFileReference(path_.c_str(), nullptr, &file_);
    if (SUCCEEDED(result)) *current = TRUE;
    return result;
  }
  HRESULT STDMETHODCALLTYPE GetCurrentFontFile(IDWriteFontFile** file) override {
    if (!file) return E_POINTER;
    *file = file_.Get(); if (!*file) return E_FAIL;
    (*file)->AddRef(); return S_OK;
  }
private:
  std::atomic<ULONG> references_{1};
  ComPtr<IDWriteFactory> factory_;
  std::wstring path_;
  ComPtr<IDWriteFontFile> file_;
  bool advanced_ = false;
};

class Loader final : public IDWriteFontCollectionLoader {
public:
  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** result) override {
    if (!result) return E_POINTER;
    *result = nullptr;
    if (iid != __uuidof(IUnknown) && iid != __uuidof(IDWriteFontCollectionLoader)) return E_NOINTERFACE;
    *result = static_cast<IDWriteFontCollectionLoader*>(this); AddRef(); return S_OK;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return ++references_; }
  ULONG STDMETHODCALLTYPE Release() override { const auto count = --references_; if (!count) delete this; return count; }
  HRESULT STDMETHODCALLTYPE CreateEnumeratorFromKey(IDWriteFactory* factory, const void* key, UINT32 size, IDWriteFontFileEnumerator** enumerator) override {
    if (!enumerator) return E_POINTER;
    *enumerator = nullptr;
    if (!factory || !key || size < sizeof(wchar_t) || size % sizeof(wchar_t)) return E_INVALIDARG;
    const auto* path = static_cast<const wchar_t*>(key);
    if (path[size / sizeof(wchar_t) - 1] != L'\0') return E_INVALIDARG;
    *enumerator = new (std::nothrow) Enumerator(factory, path);
    return *enumerator ? S_OK : E_OUTOFMEMORY;
  }
private:
  std::atomic<ULONG> references_{1};
};

struct Font {
  ComPtr<IDWriteFactory> factory;
  ComPtr<IDWriteFontCollectionLoader> loader;
  ComPtr<IDWriteFontCollection> collection;
  ComPtr<IDWriteFont> font;
  bool registered = false;
#if defined(MSIME_WINDOWS7_EMOJI_SHAPING)
  std::unique_ptr<Shaper> shaper;
#endif
  Font() {
    if (FAILED(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory), reinterpret_cast<IUnknown**>(factory.GetAddressOf())))) return;
    wchar_t executable[32768]{};
    const auto length = GetModuleFileNameW(nullptr, executable, static_cast<DWORD>(std::size(executable)));
    if (!length || length == std::size(executable)) return;
    const auto path = (std::filesystem::path(executable).parent_path() / L"fonts" / L"NotoEmoji.ttf").wstring();
    const auto attributes = GetFileAttributesW(path.c_str());
    if (attributes == INVALID_FILE_ATTRIBUTES || (attributes & FILE_ATTRIBUTE_DIRECTORY)) return;
    loader.Attach(new (std::nothrow) Loader()); if (!loader) return;
    registered = SUCCEEDED(factory->RegisterFontCollectionLoader(loader.Get()));
    if (!registered || FAILED(factory->CreateCustomFontCollection(loader.Get(), path.c_str(), static_cast<UINT32>((path.size() + 1) * sizeof(wchar_t)), &collection))) return;
    ComPtr<IDWriteFontFamily> family;
    if (collection->GetFontFamilyCount() && SUCCEEDED(collection->GetFontFamily(0, &family)))
      family->GetFirstMatchingFont(DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, &font);
#if defined(MSIME_WINDOWS7_EMOJI_SHAPING)
    if (font) shaper = std::make_unique<Shaper>(path, font.Get());
#endif
  }
  ~Font() {
#if defined(MSIME_WINDOWS7_EMOJI_SHAPING)
    shaper.reset();
#endif
    font.Reset(); collection.Reset(); if (registered) factory->UnregisterFontCollectionLoader(loader.Get());
  }
};
inline Font& bundled() { static Font instance; return instance; }
inline bool native_fallback(IDWriteFactory* factory) {
  ComPtr<IDWriteFactory2> modern;
  return factory && SUCCEEDED(factory->QueryInterface(IID_PPV_ARGS(&modern)));
}
inline ComPtr<IDWriteFont> primary_font(IDWriteFactory* factory, IDWriteTextLayout* layout) {
  UINT32 count = 0; if (FAILED(layout->GetFontFamilyNameLength(0, &count)) || count > 256) return {};
  std::wstring name(count + 1, L'\0');
  if (FAILED(layout->GetFontFamilyName(0, name.data(), count + 1))) return {};
  ComPtr<IDWriteFontCollection> collection;
  if (FAILED(layout->GetFontCollection(0, &collection)) || !collection) factory->GetSystemFontCollection(&collection);
  if (!collection) return {};
  UINT32 index = 0; BOOL exists = FALSE;
  if (FAILED(collection->FindFamilyName(name.c_str(), &index, &exists)) || !exists) return {};
  ComPtr<IDWriteFontFamily> family; ComPtr<IDWriteFont> font;
  DWRITE_FONT_WEIGHT weight = DWRITE_FONT_WEIGHT_NORMAL;
  layout->GetFontWeight(0, &weight);
  if (SUCCEEDED(collection->GetFontFamily(index, &family)))
    family->GetFirstMatchingFont(weight, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, &font);
  return font;
}
} // namespace emoji_font_detail

// Win7 的 DirectWrite 没有字体回退构建器。只为主字体缺字且随包字体确实有字的完整 UTF-16 范围指定私有集合。
inline unsigned ApplyWindows7EmojiFallback(IDWriteFactory* factory, IDWriteTextLayout* layout, const wchar_t* text, UINT32 length) {
  using namespace emoji_font_detail;
  if (!factory || !layout || !text || !length) return 0;
  auto& emoji = bundled(); if (!emoji.font || !emoji.collection) return 0;
  const auto primary = primary_font(factory, layout);
  unsigned mapped = 0;
  const auto scalar_at = [&](UINT32& position) {
    UINT32 scalar = static_cast<UINT16>(text[position++]);
    if (scalar >= 0xD800 && scalar <= 0xDBFF && position < length && text[position] >= 0xDC00 && text[position] <= 0xDFFF)
      scalar = 0x10000 + ((scalar - 0xD800) << 10) + (static_cast<UINT16>(text[position++]) - 0xDC00);
    return scalar;
  };
  const auto covered = [&](UINT32 scalar) { BOOL present = FALSE; return SUCCEEDED(emoji.font->HasCharacter(scalar, &present)) && present; };
  const auto absent = [&](UINT32 scalar) { BOOL present = FALSE; return !primary || FAILED(primary->HasCharacter(scalar, &present)) || !present; };
  for (UINT32 index = 0; index < length;) {
    const UINT32 begin = index;
    const auto base = scalar_at(index);
    const auto base_end = index;
    if (!covered(base)) continue;
    bool needed = absent(base);
    bool regional = base >= 0x1F1E6 && base <= 0x1F1FF;
    // 变体选择符、肤色、键帽和 ZWJ 连接的表情必须整簇选字，不能逐个码点拆成不同字体。
    while (index < length) {
      UINT32 end = index; const auto scalar = scalar_at(end);
      if (scalar == 0xFE0E) { index = end; continue; }
      if (scalar == 0xFE0F || scalar == 0xFE0E || scalar == 0x20E3 ||
          (scalar >= 0x1F3FB && scalar <= 0x1F3FF) || (scalar >= 0xE0020 && scalar <= 0xE007F)) {
        needed = needed || scalar == 0xFE0F || scalar == 0x20E3 || absent(scalar);
        index = end; continue;
      }
      if (scalar == 0x200D && end < length) {
        const auto joined = scalar_at(end);
        if (!covered(joined)) break;
        needed = needed || absent(joined); index = end; continue;
      }
      if (regional && scalar >= 0x1F1E6 && scalar <= 0x1F1FF) {
        needed = needed || absent(scalar); index = end; regional = false;
      }
      break;
    }
    if (!needed) continue;
    const DWRITE_TEXT_RANGE range{begin, index - begin};
    if (SUCCEEDED(layout->SetFontCollection(emoji.collection.Get(), range)) && SUCCEEDED(layout->SetFontFamilyName(L"Noto Emoji", range))) {
#if defined(MSIME_WINDOWS7_EMOJI_SHAPING)
      FLOAT size = 0;
      if (index > base_end && emoji.shaper && SUCCEEDED(layout->GetFontSize(begin, &size))) {
        const auto shaped = emoji.shaper->shape(text + begin, index - begin, size);
        if (shaped) layout->SetInlineObject(shaped.Get(), range);
      }
#else
      (void)base_end;
#endif
      ++mapped;
    }
  }
  return mapped;
}

inline HRESULT CreateTextLayoutWithWindows7Emoji(IDWriteFactory* factory, const wchar_t* text, UINT32 length, IDWriteTextFormat* format, FLOAT width, FLOAT height, IDWriteTextLayout** layout) {
  if (!factory) return E_POINTER;
  const auto result = factory->CreateTextLayout(text, length, format, width, height, layout);
  if (SUCCEEDED(result) && layout && *layout && !emoji_font_detail::native_fallback(factory)) ApplyWindows7EmojiFallback(factory, *layout, text, length);
  return result;
}

inline void DrawTextWithWindows7Emoji(IDWriteFactory* factory, ID2D1RenderTarget* target, const wchar_t* text, UINT32 length, IDWriteTextFormat* format, const D2D1_RECT_F& rectangle, ID2D1Brush* brush, D2D1_DRAW_TEXT_OPTIONS options = D2D1_DRAW_TEXT_OPTIONS_NONE, DWRITE_MEASURING_MODE mode = DWRITE_MEASURING_MODE_NATURAL) {
  if (emoji_font_detail::native_fallback(factory)) { target->DrawText(text, length, format, rectangle, brush, options, mode); return; }
  Microsoft::WRL::ComPtr<IDWriteTextLayout> layout;
  if (SUCCEEDED(CreateTextLayoutWithWindows7Emoji(factory, text, length, format, rectangle.right - rectangle.left, rectangle.bottom - rectangle.top, &layout)))
    target->DrawTextLayout(D2D1::Point2F(rectangle.left, rectangle.top), layout.Get(), brush, options);
  else target->DrawText(text, length, format, rectangle, brush, options, mode);
}

// 原生表情列表共用同一排版路径，GDI 只提供目标 DC，字形仍由 DirectWrite 选取。
inline bool DrawWindows7EmojiToDC(HDC dc, RECT rectangle, const std::wstring& text, float size, COLORREF foreground, COLORREF background) {
  using Microsoft::WRL::ComPtr;
  auto& emoji = emoji_font_detail::bundled();
  if (!emoji.factory) return false;
  ComPtr<ID2D1Factory> factory; ComPtr<ID2D1DCRenderTarget> target;
  if (FAILED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, factory.GetAddressOf()))) return false;
  const auto properties = D2D1::RenderTargetProperties(D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE));
  if (FAILED(factory->CreateDCRenderTarget(&properties, &target)) || FAILED(target->BindDC(dc, &rectangle))) return false;
  ComPtr<IDWriteTextFormat> format;
  if (FAILED(emoji.factory->CreateTextFormat(L"Microsoft YaHei", nullptr, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size, L"zh-CN", &format))) return false;
  format->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
  format->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
  const auto color = [](COLORREF value) { return D2D1::ColorF(GetRValue(value) / 255.0f, GetGValue(value) / 255.0f, GetBValue(value) / 255.0f); };
  ComPtr<ID2D1SolidColorBrush> brush;
  if (FAILED(target->CreateSolidColorBrush(color(foreground), &brush))) return false;
  target->BeginDraw(); target->Clear(color(background));
  DrawTextWithWindows7Emoji(emoji.factory.Get(), target.Get(), text.c_str(), static_cast<UINT32>(text.size()), format.Get(),
      D2D1::RectF(6, 0, static_cast<FLOAT>(rectangle.right - rectangle.left - 6), static_cast<FLOAT>(rectangle.bottom - rectangle.top)), brush.Get());
  return SUCCEEDED(target->EndDraw());
}

} // namespace msime::windows::compat
