#pragma once
#include "../../common/Windows7EmojiFont.h"
#include "../../../../shared/contracts/candidate_decoration_overlay.h"
#include <d2d1.h>
#include <dwrite.h>
#include <wrl/client.h>

namespace msime::windows {
struct CandidateDecorationText {
  Microsoft::WRL::ComPtr<IDWriteTextLayout> layout;
  D2D1_POINT_2F origin{};
  DWRITE_TEXT_METRICS metrics{};
};

// 使用真实字体回退链测量；缩字只修改本帧 layout，避免向格式缓存不断加入任意字号。
inline std::optional<CandidateDecorationText> candidate_decoration_text(
    IDWriteFactory *factory, IDWriteTextFormat *format, const std::wstring &word,
    const msime::candidate::DecorationCandidate &style, const D2D1_RECT_F &box) {
  if (!factory || !format || !style.valid() || word.empty() || word.size() > 4096 ||
      !(box.right > box.left) || !(box.bottom > box.top)) return std::nullopt;
  CandidateDecorationText result;
  const DWRITE_TEXT_RANGE range{0, static_cast<UINT32>(word.size())};
  if (FAILED(compat::CreateTextLayoutWithWindows7Emoji(factory, word.c_str(), range.length, format, 65536, 1000,
          result.layout.GetAddressOf()))) return std::nullopt;
  if (FAILED(result.layout->SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)) ||
      FAILED(result.layout->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)) ||
      FAILED(result.layout->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)) ||
      FAILED(result.layout->SetFontSize(static_cast<float>(style.fontSizeDip), range)) ||
      FAILED(result.layout->GetMetrics(&result.metrics))) return std::nullopt;
  const double fitted = style.fittedFont(result.metrics.widthIncludingTrailingWhitespace, box.right - box.left);
  if (fitted != style.fontSizeDip &&
      (FAILED(result.layout->SetFontSize(static_cast<float>(fitted), range)) ||
       FAILED(result.layout->GetMetrics(&result.metrics)))) return std::nullopt;
  result.origin = {box.left + static_cast<float>(style.left(box.right - box.left,
      result.metrics.widthIncludingTrailingWhitespace)),
      box.top + static_cast<float>(style.top(box.bottom - box.top, result.metrics.height))};
  return result;
}
} // namespace msime::windows
