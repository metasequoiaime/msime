#include "../../common/Windows7EmojiFont.h"
#include <cassert>
#include <cstdio>
#include <thread>
#include <vector>

namespace {
class GlyphCheck final : public IDWriteTextRenderer {
public:
  unsigned glyphs = 0, missing = 0;
  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** result) override {
    if (!result) return E_POINTER;
    *result = nullptr;
    if (iid != __uuidof(IUnknown) && iid != __uuidof(IDWriteTextRenderer) && iid != __uuidof(IDWritePixelSnapping)) return E_NOINTERFACE;
    *result = static_cast<IDWriteTextRenderer*>(this); return S_OK;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return 1; }
  ULONG STDMETHODCALLTYPE Release() override { return 1; }
  HRESULT STDMETHODCALLTYPE IsPixelSnappingDisabled(void*, BOOL* disabled) override { *disabled = TRUE; return S_OK; }
  HRESULT STDMETHODCALLTYPE GetCurrentTransform(void*, DWRITE_MATRIX* matrix) override { *matrix = {1, 0, 0, 1, 0, 0}; return S_OK; }
  HRESULT STDMETHODCALLTYPE GetPixelsPerDip(void*, FLOAT* pixels) override { *pixels = 1; return S_OK; }
  HRESULT STDMETHODCALLTYPE DrawGlyphRun(void*, FLOAT, FLOAT, DWRITE_MEASURING_MODE, const DWRITE_GLYPH_RUN* run, const DWRITE_GLYPH_RUN_DESCRIPTION*, IUnknown*) override {
    for (UINT32 index = 0; index < run->glyphCount; ++index) { ++glyphs; if (!run->glyphIndices[index]) ++missing; }
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE DrawUnderline(void*, FLOAT, FLOAT, const DWRITE_UNDERLINE*, IUnknown*) override { return S_OK; }
  HRESULT STDMETHODCALLTYPE DrawStrikethrough(void*, FLOAT, FLOAT, const DWRITE_STRIKETHROUGH*, IUnknown*) override { return S_OK; }
  HRESULT STDMETHODCALLTYPE DrawInlineObject(void* context, FLOAT x, FLOAT y, IDWriteInlineObject* object, BOOL sideways, BOOL rtl, IUnknown* effect) override {
    return object->Draw(context, this, x, y, sideways, rtl, effect);
  }
};
}
int main() {
  using namespace msime::windows::compat;
  using Microsoft::WRL::ComPtr;
  auto& font = emoji_font_detail::bundled();
  assert(font.factory && font.collection && font.font);
  for (const auto scalar : {0x1F44Bu, 0x1F680u, 0x1FAE8u}) {
    BOOL present = FALSE; assert(SUCCEEDED(font.font->HasCharacter(scalar, &present)) && present);
  }
  ComPtr<IDWriteTextFormat> format;
  assert(SUCCEEDED(font.factory->CreateTextFormat(L"Microsoft YaHei", nullptr, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, 24, L"zh-CN", &format)));
  const std::wstring text = L"\u4F60\U0001F44B\u597D\U0001FAE8";
  ComPtr<IDWriteTextLayout> layout;
  assert(SUCCEEDED(font.factory->CreateTextLayout(text.c_str(), static_cast<UINT32>(text.size()), format.Get(), 400, 100, &layout)));
  assert(ApplyWindows7EmojiFallback(font.factory.Get(), layout.Get(), text.c_str(), static_cast<UINT32>(text.size())) == 2);
  wchar_t name[80]{}; DWRITE_TEXT_RANGE range{};
  assert(SUCCEEDED(layout->GetFontFamilyName(1, name, 80, &range)) && std::wstring(name) == L"Noto Emoji" && range.startPosition == 1 && range.length == 2);
  assert(SUCCEEDED(layout->GetFontFamilyName(0, name, 80)) && std::wstring(name) == L"Microsoft YaHei");
  assert(SUCCEEDED(layout->GetFontFamilyName(3, name, 80)) && std::wstring(name) == L"Microsoft YaHei");
  GlyphCheck check;
  assert(SUCCEEDED(layout->Draw(nullptr, &check, 0, 0)) && check.glyphs >= 4 && check.missing == 0);
  DWRITE_TEXT_METRICS metrics{};
  assert(SUCCEEDED(layout->GetMetrics(&metrics)) && metrics.width > 48 && metrics.height >= 24);
  const std::wstring clusters = L"\U0001F468\u200D\U0001F469\u200D\U0001F467 1\uFE0F\u20E3 \U0001F44B\U0001F3FD";
  layout.Reset();
  assert(SUCCEEDED(font.factory->CreateTextLayout(clusters.c_str(), static_cast<UINT32>(clusters.size()), format.Get(), 400, 100, &layout)));
  assert(ApplyWindows7EmojiFallback(font.factory.Get(), layout.Get(), clusters.c_str(), static_cast<UINT32>(clusters.size())) == 3);
  assert(SUCCEEDED(layout->GetFontFamilyName(0, name, 80, &range)) && range.startPosition == 0 && range.length == 8);
  GlyphCheck joined;
  assert(SUCCEEDED(layout->Draw(nullptr, &joined, 0, 0)) && joined.missing == 0 && joined.glyphs == 5);
  assert(SUCCEEDED(layout->GetMetrics(&metrics)) && metrics.width > 80 && metrics.width < 120);
  FLOAT x = 0, y = 0; DWRITE_HIT_TEST_METRICS hit{};
  assert(SUCCEEDED(layout->HitTestTextPosition(0, FALSE, &x, &y, &hit)) && hit.textPosition == 0 && hit.length == 8);
  // 模拟多个桌面的候选 UI 同时排版，检查共享字体与独立布局的字形和命中范围。
  std::vector<std::thread> desktops;
  for (unsigned desktop = 0; desktop < 4; ++desktop) {
    desktops.emplace_back([&] {
      for (unsigned iteration = 0; iteration < 64; ++iteration) {
        ComPtr<IDWriteTextLayout> concurrent;
        assert(SUCCEEDED(font.factory->CreateTextLayout(clusters.c_str(), static_cast<UINT32>(clusters.size()), format.Get(), 400, 100, &concurrent)));
        assert(ApplyWindows7EmojiFallback(font.factory.Get(), concurrent.Get(), clusters.c_str(), static_cast<UINT32>(clusters.size())) == 3);
        GlyphCheck rendered;
        assert(SUCCEEDED(concurrent->Draw(nullptr, &rendered, 0, 0)) && rendered.missing == 0 && rendered.glyphs == 5);
        FLOAT hit_x = 0, hit_y = 0; DWRITE_HIT_TEST_METRICS cluster_hit{};
        assert(SUCCEEDED(concurrent->HitTestTextPosition(0, FALSE, &hit_x, &hit_y, &cluster_hit)) && cluster_hit.textPosition == 0 && cluster_hit.length == 8);
      }
    });
  }
  for (auto& desktop : desktops) desktop.join();
  const std::wstring text_digit = L"1\uFE0E";
  layout.Reset();
  assert(SUCCEEDED(font.factory->CreateTextLayout(text_digit.c_str(), static_cast<UINT32>(text_digit.size()), format.Get(), 400, 100, &layout)));
  assert(ApplyWindows7EmojiFallback(font.factory.Get(), layout.Get(), text_digit.c_str(), static_cast<UINT32>(text_digit.size())) == 0);
  std::puts("Win7 private emoji font: waving hand and newer emoji shape without missing glyphs; UTF-16 ranges and Chinese font preserved");
}
