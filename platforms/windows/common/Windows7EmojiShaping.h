#pragma once

#include <dwrite.h>
#include <wrl/client.h>
#include <hb.h>
#include <hb-ot.h>
#include <atomic>
#include <filesystem>
#include <fstream>
#include <memory>
#include <vector>

namespace msime::windows::compat::emoji_font_detail {

// Win7 的系统排版不会合成现代表情序列。只为私有表情簇使用 HarfBuzz，
// 再通过内联对象交还给 DirectWrite，保留原始文本、UTF-16 索引和命中区域。
class ShapedEmoji final : public IDWriteInlineObject {
public:
  Microsoft::WRL::ComPtr<IDWriteFontFace> face;
  std::vector<UINT16> glyphs;
  std::vector<FLOAT> advances;
  std::vector<DWRITE_GLYPH_OFFSET> offsets;
  FLOAT size = 0;
  DWRITE_INLINE_OBJECT_METRICS metrics{};
  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** result) override {
    if (!result) return E_POINTER;
    *result = nullptr;
    if (iid != __uuidof(IUnknown) && iid != __uuidof(IDWriteInlineObject)) return E_NOINTERFACE;
    *result = static_cast<IDWriteInlineObject*>(this); AddRef(); return S_OK;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return ++references_; }
  ULONG STDMETHODCALLTYPE Release() override { const auto count = --references_; if (!count) delete this; return count; }
  HRESULT STDMETHODCALLTYPE Draw(void* context, IDWriteTextRenderer* renderer, FLOAT x, FLOAT y, BOOL sideways, BOOL rtl, IUnknown* effect) override {
    if (!renderer) return E_POINTER;
    const DWRITE_GLYPH_RUN run{face.Get(), size, static_cast<UINT32>(glyphs.size()), glyphs.data(), advances.data(), offsets.data(), sideways, rtl ? 1u : 0u};
    return renderer->DrawGlyphRun(context, x, y + metrics.baseline, DWRITE_MEASURING_MODE_NATURAL, &run, nullptr, effect);
  }
  HRESULT STDMETHODCALLTYPE GetMetrics(DWRITE_INLINE_OBJECT_METRICS* value) override {
    if (!value) return E_POINTER;
    *value = metrics; return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetOverhangMetrics(DWRITE_OVERHANG_METRICS* value) override {
    if (!value) return E_POINTER;
    *value = {}; return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GetBreakConditions(DWRITE_BREAK_CONDITION* before, DWRITE_BREAK_CONDITION* after) override {
    if (!before || !after) return E_POINTER;
    *before = *after = DWRITE_BREAK_CONDITION_NEUTRAL; return S_OK;
  }
private:
  std::atomic<ULONG> references_{1};
};

class Shaper final {
public:
  Shaper(const std::filesystem::path& path, IDWriteFont* direct_font) {
    if (!direct_font || FAILED(direct_font->CreateFontFace(&face_))) return;
    std::ifstream file(path, std::ios::binary | std::ios::ate);
    const auto length = file.tellg();
    if (!file || length <= 0 || length > 16 * 1024 * 1024) return;
    std::vector<char> bytes(static_cast<size_t>(length));
    file.seekg(0); if (!file.read(bytes.data(), static_cast<std::streamsize>(length))) return;
    auto* blob = hb_blob_create(bytes.data(), static_cast<unsigned>(bytes.size()), HB_MEMORY_MODE_DUPLICATE, nullptr, nullptr);
    auto* face = hb_face_create(blob, 0); hb_blob_destroy(blob);
    font_ = hb_font_create(face); hb_face_destroy(face);
    hb_ot_font_set_funcs(font_);
    face_->GetMetrics(&metrics_);
    hb_font_set_scale(font_, metrics_.designUnitsPerEm, metrics_.designUnitsPerEm);
    // 多桌面的候选线程共用整形字体；发布前冻结，缓冲区仍由每次调用独立持有。
    hb_font_make_immutable(font_);
  }
  ~Shaper() { if (font_) hb_font_destroy(font_); }
  Shaper(const Shaper&) = delete;
  Shaper& operator=(const Shaper&) = delete;
  Microsoft::WRL::ComPtr<ShapedEmoji> shape(const wchar_t* text, UINT32 length, FLOAT size) const {
    if (!font_ || !metrics_.designUnitsPerEm || length > 128) return {};
    auto* buffer = hb_buffer_create();
    hb_buffer_add_utf16(buffer, reinterpret_cast<const uint16_t*>(text), static_cast<int>(length), 0, static_cast<int>(length));
    hb_buffer_set_direction(buffer, HB_DIRECTION_LTR);
    hb_buffer_guess_segment_properties(buffer);
    hb_shape(font_, buffer, nullptr, 0);
    unsigned count = 0;
    const auto* info = hb_buffer_get_glyph_infos(buffer, &count);
    const auto* positions = hb_buffer_get_glyph_positions(buffer, nullptr);
    Microsoft::WRL::ComPtr<ShapedEmoji> result;
    result.Attach(new (std::nothrow) ShapedEmoji());
    if (result && hb_buffer_allocation_successful(buffer)) {
      result->face = face_; result->size = size;
      const auto scale = size / metrics_.designUnitsPerEm;
      result->metrics.baseline = metrics_.ascent * scale;
      result->metrics.height = (metrics_.ascent + metrics_.descent + metrics_.lineGap) * scale;
      for (unsigned index = 0; index < count; ++index) {
        if (!info[index].codepoint || info[index].codepoint > UINT16_MAX) { result.Reset(); break; }
        const auto glyph = static_cast<UINT16>(info[index].codepoint);
        DWRITE_GLYPH_METRICS ink{};
        // HarfBuzz 为变体选择符保留的空白控制字形不占空间，也不应当作可见表情计数。
        if (!positions[index].x_advance && SUCCEEDED(face_->GetDesignGlyphMetrics(&glyph, 1, &ink)) &&
            static_cast<INT32>(ink.advanceWidth) == ink.leftSideBearing + ink.rightSideBearing &&
            static_cast<INT32>(ink.advanceHeight) == ink.topSideBearing + ink.bottomSideBearing) continue;
        result->glyphs.push_back(glyph);
        const auto advance = positions[index].x_advance * scale;
        result->advances.push_back(advance);
        result->offsets.push_back({positions[index].x_offset * scale, positions[index].y_offset * scale});
        result->metrics.width += advance;
      }
      if (result && result->glyphs.empty()) result.Reset();
    } else result.Reset();
    hb_buffer_destroy(buffer); return result;
  }
private:
  hb_font_t* font_ = nullptr;
  Microsoft::WRL::ComPtr<IDWriteFontFace> face_;
  DWRITE_FONT_METRICS metrics_{};
};

} // namespace msime::windows::compat::emoji_font_detail
