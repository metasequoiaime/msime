#pragma once
#include "CandidateActionAvailability.h"
#include "CandidateExpandPanel.h"
#include "CandidateGlossReadings.h"
#include "ChineseTextConversion.h"
#include "FocusGate.h"
#include "GlossColumnPolicy.h"
#include "InputSchemeTraits.h"
#include "PipeMetadata.h"
#include "PreeditDisplayCaret.h"
#include "PredictionPolicy.h"
#include "ReplyComposer.h"
#include "ShuangpinKeymapLayout.h"
#include "TranslationDisplay.h"
#include "WubiCodeHintPolicy.h"
#include <algorithm>
#include <memory>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace msime::windows {
// TSF can report the candidate show event before it has a usable text extent.
// Keep this sentinel aligned with the native Windows host contract.
inline constexpr int invalid_candidate_anchor_y = -100000;

struct PresentationCandidate {
  uint64_t session;
  uint64_t generation;
  size_t index;
  std::string text;
  bool highlighted;
  // Engine-corrected spellings are marked for the user, while `text` remains
  // the exact value selected by the candidate identity.
  bool corrected = false;
  std::string annotation;
  std::string badge;
  uint8_t fixed_position = 0;
  std::string translation;
  bool actions_available = true;
  // The Wubi code left after the typed prefix; shown only when the `wubi_code_hint` preference is on, see with_wubi_code_hints.
  std::string wubi_code_hint{};
  // A Korean Hanja's 훈음 (나라 이름 한), which the Engine sends as the row's annotation. It is drawn on the smaller secondary line whatever the translation preferences say, above the translation when there is one, and it is display only: nothing commits it, and `translation` keeps only what the Engine applied as a translation.
  std::string gloss{};
  // 释义每一行的读音，按 "\n" 分行，和 translation 的各行（U+2028 分行）逐行对应；只用于显示，从不上屏。由翻译工作线程算好、CandidateMailbox 按候选文字和释义挂上来，见 CandidateReading。
  std::string pronunciation{};
  // AI 占位行（Engine 候选来源 12）：等回答时是「…」，回答为空或失败后是「—」。画成次要色，选它什么都不上屏（#6888）。共享内核已经不再产出这种行（AI 回答到达时直接插在第一页最后一格），这里的处理留着，旧的库或数据里出现时仍按占位行对待。
  bool pending = false;
  // 授权的脚本插件给这一行加的注释（view 候选的 script_annotation，没有插件注释时为空）。和引擎的 annotation 分开放、分开画：单独占释义那一段的最后一行，斜体，从不替换引擎的注释，见 candidate_secondary_run。
  std::string script_annotation{};
  // view 候选的 translation_reserved：候选词翻译范围是「仅当前高亮」、这一页会请求释义时，高亮的那一行为真（为假时 JSON 里没有这个键）。竖排候选窗据此给这一行一直留出释义行，见 CandidateItemWidths::translation_reserved。
  bool translation_reserved = false;
};
// Whether this view's candidates are a Korean Hanja list: the Korean scheme under its own rules, outside the dedicated English mode and every local mode, where the Engine lists candidates only after MSIME_CONVERT_HANJA. ReplyComposer::korean_hanja reads the same three fields.
inline bool korean_hanja_view(const nlohmann::json &view) {
  return view.value("scheme", 0u) == candidate_scheme_korean &&
         !view.value("dedicated_english", false) &&
         view.value("local_mode", std::string("none")) == "none";
}
// 这个 view 的候选会不会带释义，和 host-api msime_client_translation_query 的判据一样：方案显示释义（scheme::ShowsGlosses，日文、粤语、注音、笔画、越南文、藏文都不显示），且不在临时日文组字和网址模式里。
inline bool candidate_view_shows_glosses(const nlohmann::json &view) {
  const auto mode = view.value("local_mode", std::string("none"));
  return scheme::ShowsGlosses(static_cast<int>(view.value("scheme", 0u))) && mode != "temporary_japanese" &&
         mode != "url";
}
// Move a Hanja row's 훈음 out of the annotation run, which follows the Hanja at full size, into `gloss`, so the main text is the Hanja alone.
inline void move_korean_hanja_gloss(PresentationCandidate &candidate) {
  candidate.gloss = std::move(candidate.annotation);
  candidate.annotation.clear();
}
// 释义部分画出来的样子：每种目标语言一行，有读音的行下面单独一行是它的读音（candidate_gloss_display）。with_readings 为 false 时只有释义各行，不带读音。reading_ranges 不为空时记下读音的位置，line_starts 不为空时记下每行释义的起点。
inline std::string candidate_translation_display(const PresentationCandidate &candidate,
                                                 bool with_readings = true,
                                                 std::vector<std::pair<size_t, size_t>> *reading_ranges = nullptr,
                                                 std::vector<size_t> *line_starts = nullptr) {
  std::vector<std::string> lines;
  if (!candidate.translation.empty())
    lines = translation_lines(candidate.translation);
  if (!with_readings)
    return candidate_gloss_display(lines, {}, nullptr, line_starts);
  std::vector<std::string> readings;
  for (size_t start = 0; !candidate.pronunciation.empty();) {
    const auto cut = candidate.pronunciation.find('\n', start);
    readings.push_back(candidate.pronunciation.substr(start, cut == std::string::npos ? std::string::npos : cut - start));
    if (cut == std::string::npos)
      break;
    start = cut + 1;
  }
  return candidate_gloss_display(lines, readings, reading_ranges, line_starts);
}
// 去掉首尾空白，和 macOS 的 whitespaceAndNewlineCharacterSet 一样：ASCII 空白、U+0085，以及 Unicode 的空格分隔符（U+00A0、U+1680、U+2000–U+200A、U+2028、U+2029、U+202F、U+205F、U+3000）。
inline std::string_view candidate_trim_unicode_spaces(std::string_view trimmed) {
  static constexpr std::string_view spaces[] = {
      "\t", "\n", "\v", "\f", "\r", " ", "\xC2\x85", "\xC2\xA0", "\xE1\x9A\x80",
      "\xE2\x80\x80", "\xE2\x80\x81", "\xE2\x80\x82", "\xE2\x80\x83", "\xE2\x80\x84",
      "\xE2\x80\x85", "\xE2\x80\x86", "\xE2\x80\x87", "\xE2\x80\x88", "\xE2\x80\x89",
      "\xE2\x80\x8A", "\xE2\x80\xA8", "\xE2\x80\xA9", "\xE2\x80\xAF", "\xE2\x81\x9F",
      "\xE3\x80\x80"};
  for (bool stripped = true; stripped && !trimmed.empty();) {
    stripped = false;
    for (const auto space : spaces) {
      if (trimmed.substr(0, space.size()) == space) {
        trimmed.remove_prefix(space.size());
        stripped = true;
      }
      if (trimmed.size() >= space.size() &&
          trimmed.substr(trimmed.size() - space.size()) == space) {
        trimmed.remove_suffix(space.size());
        stripped = true;
      }
    }
  }
  return trimmed;
}
// macOS 的 controlCharacterSet 里除了 Cc（C0、DEL、C1）之外的格式字符（Unicode 的 Cf）：软连字符、阿拉伯字母标记、零宽字符、双向控制符（U+202A–U+202E、U+2066–U+2069）、BOM 等。插件注释里的 U+202E 不去掉的话会把自己那一行倒过来画。
inline bool candidate_format_code_point(char32_t c) {
  return c == 0xAD || (c >= 0x600 && c <= 0x605) || c == 0x61C || c == 0x6DD || c == 0x70F ||
         c == 0x890 || c == 0x891 || c == 0x8E2 || c == 0x180E || (c >= 0x200B && c <= 0x200F) ||
         (c >= 0x202A && c <= 0x202E) || (c >= 0x2060 && c <= 0x2064) ||
         (c >= 0x2066 && c <= 0x206F) || c == 0xFEFF || (c >= 0xFFF9 && c <= 0xFFFB) ||
         c == 0x110BD || c == 0x110CD || (c >= 0x13430 && c <= 0x1343F) ||
         (c >= 0x1BCA0 && c <= 0x1BCA3) || (c >= 0x1D173 && c <= 0x1D17A) || c == 0xE0001 ||
         (c >= 0xE0020 && c <= 0xE007F);
}
// 脚本插件的注释收拾成能画成一行的文本，和 macOS 的 MSIMECandidateScriptAnnotationRun 一样：控制字符（Cc）、格式字符（Cf，见 candidate_format_code_point）和各种换行（含 U+0085、U+2028、U+2029，释义那一段正是按 "\n" 分行的）连成的一段换成一个空格，再去掉首尾空白（candidate_trim_unicode_spaces）。长度由共享内核按 32 个 UTF-16 码元截好。
inline std::string candidate_script_annotation_line(const std::string &value) {
  std::string line;
  line.reserve(value.size());
  bool broken = false;
  for (size_t index = 0; index < value.size();) {
    const auto byte = static_cast<unsigned char>(value[index]);
    const auto next = [&](size_t offset) {
      return index + offset < value.size() ? static_cast<unsigned char>(value[index + offset]) : 0u;
    };
    // 按 UTF-8 解出这个码位；不成形的字节原样留下，一次走一个字节。
    size_t length = 1;
    char32_t code = byte;
    if (byte >= 0xC0 && byte < 0xF8) {
      length = byte < 0xE0 ? 2 : byte < 0xF0 ? 3 : 4;
      code = byte & (0x7F >> length);
      for (size_t offset = 1; offset < length; ++offset) {
        if ((next(offset) & 0xC0) != 0x80) {
          length = 1;
          code = byte;
          break;
        }
        code = (code << 6) | (next(offset) & 0x3F);
      }
    }
    const bool breaking = code < 0x20 || (code >= 0x7F && code <= 0x9F) || code == 0x2028 ||
                          code == 0x2029 || candidate_format_code_point(code);
    if (breaking && (length > 1 || byte < 0x80)) {
      broken = true;
      index += length;
      continue;
    }
    if (broken)
      line.push_back(' ');
    broken = false;
    line.append(value, index, length);
    index += length;
  }
  return std::string(candidate_trim_unicode_spaces(line));
}
// 一行候选下面那段较小的次要文字，连同要换样式画的几段：只有 훈음、只有释义，或 훈음 在上、释义在下一行；脚本插件的注释单独占最后一行，排在输入法自己的读音、译文之后，不挪动它们的位置，和 macOS 的 MSIMECandidateGlossWithScriptAnnotation 一样。释义部分是 candidate_translation_display：每种目标语言一行，读音在它那一行下面单独一行。候选窗只画不超过 4096 字节的文字（CandidateWindow 的 wide()），读音加上去超过时就不带读音，插件注释再加上去超过时就不带插件注释，免得一条很长的释义让整个候选窗画不出来。
struct CandidateSecondaryRun {
  std::string text;
  // 读音在 text 里的位置（UTF-8 字节的起点和长度），候选窗按释义色的 0.72 倍透明度画。
  std::vector<std::pair<size_t, size_t>> readings;
  // 每行释义（每种目标语言一行，空行也算）在 text 里的起点（UTF-8 字节）；读音占了自己的行，第 N 行释义不一定是 text 的第 N 行，预选释义列的下划线按它找位置。没有释义时为空。
  std::vector<size_t> translation_line_starts;
  // 插件注释那一行在 text 里的起点（UTF-8 字节），到 text 末尾为止，候选窗画成斜体；没有时为 npos。
  size_t script_start = std::string::npos;
};
inline CandidateSecondaryRun candidate_secondary_run(const PresentationCandidate &candidate) {
  const auto compose = [&](bool with_readings) {
    CandidateSecondaryRun run;
    std::vector<std::pair<size_t, size_t>> readings;
    std::vector<size_t> line_starts;
    const auto translation =
        candidate_translation_display(candidate, with_readings, &readings, &line_starts);
    size_t offset = 0;
    if (candidate.gloss.empty()) {
      run.text = translation;
    } else if (translation.empty()) {
      run.text = candidate.gloss;
    } else {
      run.text = candidate.gloss + "\n" + translation;
      offset = candidate.gloss.size() + 1;
    }
    for (const auto &[start, length] : readings)
      run.readings.emplace_back(offset + start, length);
    for (const auto start : line_starts)
      run.translation_line_starts.push_back(offset + start);
    return run;
  };
  auto run = compose(true);
  if (run.text.size() > 4096 && !candidate.pronunciation.empty())
    run = compose(false);
  const auto script = candidate_script_annotation_line(candidate.script_annotation);
  const size_t start = run.text.empty() ? 0 : run.text.size() + 1;
  if (!script.empty() && start + script.size() <= 4096) {
    if (!run.text.empty())
      run.text.push_back('\n');
    run.script_start = run.text.size();
    run.text += script;
  }
  return run;
}
inline std::string candidate_secondary_text(const PresentationCandidate &candidate) {
  return candidate_secondary_run(candidate).text;
}
// 把翻译工作线程算好的读音挂到候选上，没有的清空：读音只在候选眼下的释义与算它时的释义相同才挂。
inline void attach_candidate_readings(std::vector<PresentationCandidate> &candidates,
                                      const CandidateReadings &readings) {
  for (auto &candidate : candidates) {
    candidate.pronunciation.clear();
    const auto found = readings.find(candidate.text);
    if (found == readings.end())
      continue;
    if (!candidate.translation.empty() && found->second.translation == candidate.translation)
      candidate.pronunciation = found->second.pronunciation;
  }
}
// The correction marker is display-only: keep it out of the text used for
// selection, dictionary actions and candidate identity.
inline std::string candidate_primary_text(const PresentationCandidate &candidate) {
  return candidate.text + (candidate.corrected ? "*" : "") + candidate.badge;
}
// 一行候选的注释那一段：引擎的注释（辅助码、五笔的剩余编码、emoji 说明、地名所属省份……）一律包在半角括号里、紧贴候选文字，画成「你好(wqvb)」，和 macOS 的 MSIMEParenthesizedAnnotation 一样；已经带半角或全角括号的原样保留，只有空白时不画。韩文汉字的 훈음 不在这里，它已经挪到释义那一段（move_korean_hanja_gloss）；脚本插件的注释也不在这里，见 candidate_secondary_run。
inline std::string candidate_annotation_run(const PresentationCandidate &candidate) {
  // 首尾空白和 macOS 的 whitespaceAndNewlineCharacterSet 一样去掉（candidate_trim_unicode_spaces）。
  const std::string_view trimmed = candidate_trim_unicode_spaces(candidate.annotation);
  if (trimmed.empty())
    return {};
  const std::string annotation(trimmed);
  const auto wrapped = [&](std::string_view open, std::string_view close) {
    return annotation.size() >= open.size() + close.size() &&
           annotation.compare(0, open.size(), open) == 0 &&
           annotation.compare(annotation.size() - close.size(), close.size(), close) == 0;
  };
  if (wrapped("(", ")") || wrapped("\xEF\xBC\x88", "\xEF\xBC\x89"))
    return annotation;
  // 加上的括号让这一段超过候选窗能画的 4096 字节时原样画，不让一条注释把整个候选窗弄得画不出来。
  if (annotation.size() + 2 > 4096)
    return annotation;
  return "(" + annotation + ")";
}
// 鼠标停在一行候选上时的提示：候选全文，有释义时下一行是释义，和 macOS 候选按钮的 toolTip 一样。长候选和长释义在卡片里会折行，提示里是完整的一段。
inline std::string candidate_tooltip_text(const PresentationCandidate &candidate) {
  auto text = candidate_primary_text(candidate);
  const auto secondary = candidate_secondary_text(candidate);
  if (!secondary.empty())
    text += "\n" + secondary;
  return text;
}
// candidate_secondary_text 在折行之前有几行：훈음、每种目标语言一行、每条读音一行，以及插件注释那一行。Engine 不收含控制字符的释义，훈음 表里也没有，所以每个换行都是 candidate_secondary_text 自己放进去的。候选窗按它排多行释义（CandidateItemWidths::translation_lines）：横排同一行的格子取最高一格的高度，所以一页里只要有一个候选有读音，每格都多出一行；竖排只有一行释义加它的读音时和 macOS 一样两行都留在候选右边（CandidateItemWidths::reading_lines，candidate_translation_beside），其他多行释义放在候选文字下面。
inline size_t candidate_secondary_lines(const PresentationCandidate &candidate) {
  const auto text = candidate_secondary_text(candidate);
  return 1 + static_cast<size_t>(std::count(text.begin(), text.end(), '\n'));
}
struct CandidatePresentation {
  FocusLease lease;
  uint64_t session;
  uint64_t generation;
  // Monotonic identity for the complete rendered snapshot, including async
  // provider updates that keep the same Engine generation.
  uint64_t render_serial = 0;
  // Ctrl+Enter 释义页的编号（view 的 translation_page，ReplyComposer::translation_page），Engine 自己的候选为 0。释义页沿用 Engine 列表的 session 和 generation，点击要带上它，Server 才分得清点的是哪一张表。
  uint64_t translation_page = 0;
  bool visible = false;
  // visible 为 false 时：是不是组字正常结束（上屏、取消，或切到英文）。只有这种收起候选窗才播皮肤的退场；失焦、UILess、宿主接管、断开这些涉及隐私和输入目标的收起都立即消失，不把旧候选留在新的前台上淡出，和 macOS 的 allowExit 只认 "empty"、"english_mode" 一样。
  bool composition_ended = false;
  int x = 0;
  int y = 0;
  std::string preedit;
  // Byte offset of the caret within `preedit`, or npos when it cannot be placed. The Engine reports the caret against editing_text, which is not always the same string as the preedit being drawn; putting the caret at the wrong character is worse than not drawing one, so it is only carried when the two agree or the preedit is editing_text's letters with display separators (preedit_display_caret).
  size_t preedit_caret = std::string::npos;
  std::vector<PresentationCandidate> candidates;
  bool traditional_output = false;
  // The 0-based page on show and how many pages the Engine has so far, for the pager in the preedit row. Both zero draw no pager. The count grows as the user pages, because the Engine fetches candidates lazily.
  size_t page = 0;
  size_t page_count = 0;
  // 每页候选数（view 的 page_size），卷轴展开的网格最多分这么多列；view 没带时为 0。
  size_t page_size = 0;
  // 卷轴展开打开时这一代的完整候选列表（msime_client_all_candidates），只在它和这一页同一会话、同一代时由 CandidateMailbox::snapshot 挂上，其余时候为空。共享不可变的一份：候选窗每轮都读一次快照，不必每次复制几百个候选。
  std::shared_ptr<const std::vector<ExpandedCandidate>> expanded;
  // Server 这一侧的卷轴展开是不是为这个租约开着（CandidateMailbox::expansion）。候选窗关闭（上屏、取消）时 Server 先清掉它，候选窗按它收起，两边不会一个开着一个关着。
  bool expansion_open = false;
  // Whether the mouse may pick a row, page the list or open a row's menu. False for a list driven from the keyboard only (scheme::KeyboardOnlyCandidateList).
  bool pointer_input = true;
  // 这一页的候选会不会有释义（candidate_view_shows_glosses）。为 false 时翻译查询为空，横排候选窗不为释义预留高度。
  bool shows_glosses = true;
  // 游戏会话（包上带 PipeMetadata::GameHost）：宿主给的锚点可能不可信，候选窗允许在游戏客户区里兜底定位。
  bool game_host = false;
  // 双拼组字时候选窗旁的键位提示（方案名和要高亮的键），按 view 判断，不是双拼组字时为空；开关由 Server 主循环另外看。
  std::optional<ShuangpinKeymapHint> shuangpin_keymap;
  // Tab 预选的释义列（1 或 2），候选窗给高亮候选的那一行释义画下划线；0 表示没有预选。见 GlossColumnPolicy.h。
  int armed_gloss_column = 0;
  // Show/Move 包报来的组字锚点（物理像素，文字底边的左端），没有报过时 caret_anchor_known 为 false。上面的 x/y 不能当光标用：按键包不带坐标，租约里还没有 Show/Move 时 x/y 是线上结构的默认值 (100,100)（之后 CandidateMailbox::delivered 沿用报过的位置），宿主还没有文字范围时又是 INVALID_Y。输入模式提示在没有系统光标时靠它摆位置，同一租约里跨按键沿用。
  bool caret_anchor_known = false;
  int caret_anchor_x = 0;
  int caret_anchor_y = 0;
  // 光标后编辑（view 的 caret_edit_suffix，偏好 `caret_edit_suffix`）正在生效：候选是组字光标后那段的，首选是整句。候选窗在首行右侧、翻页之前画 candidate_caret_edit_hint，见 candidate_presentation_caret_edit。
  bool caret_edit_suffix = false;
  // 光标后编辑里首选（index 为 0 的那一行）开头固定的那段文字（view 的 caret_edit_prefix），已按输出模式换成和候选文字一样的写法。候选窗把首选开头的这几个字画淡，见 candidate_caret_edit_dim。没有生效时为空。
  std::string caret_edit_prefix{};
  // 上屏后联想（view 的 prediction，idle_prediction_view）：没有拼音，候选是建议的后续文字，数字键、空格和点选都选它。CandidateMailbox 等 TIP 在光标处发 Show 之后才让它露出来。
  bool prediction = false;
};
// 光标后编辑生效时首行右侧的提示：只是一行字，不能点。
inline constexpr const char *candidate_caret_edit_hint = "候选编辑：光标后";
// 这一页首行右侧要画的提示：光标后编辑生效时是 candidate_caret_edit_hint，其余时候为空，候选窗的样子和没有这项功能时完全一样。
inline std::string candidate_header_hint(const CandidatePresentation &value) {
  return value.caret_edit_suffix ? std::string(candidate_caret_edit_hint) : std::string{};
}
// 一行候选开头要画淡的那一段（UTF-8 字节数）：只有光标后编辑里的首选（Engine 下标为 0 的那一行），而且它的文字确实以 caret_edit_prefix 开头时才有，其余为 0。按字节比对开头，和按 Unicode 标量数去数是同一段字；对不上（比如繁体换算改了字数）时宁可不画淡，也不把淡色画到别的字上。
inline size_t candidate_caret_edit_dim(const CandidatePresentation &value,
                                       const PresentationCandidate &candidate) {
  const auto &prefix = value.caret_edit_prefix;
  if (!value.caret_edit_suffix || prefix.empty() || candidate.index != 0 ||
      candidate.text.compare(0, prefix.size(), prefix) != 0)
    return 0;
  return prefix.size();
}
// 把 view 的光标后编辑两项抄进 output：caret_edit_suffix 不在时就是没有生效。固定那段按首选的来源和输出模式换成候选文字的写法（candidate_output），候选窗才能按字节比对它。Ctrl+Enter 释义页上的义项不是光标后那段的候选，不带这两项。
inline void candidate_presentation_caret_edit(CandidatePresentation &output,
                                              const nlohmann::json &view,
                                              bool traditional_output) {
  output.caret_edit_suffix = !output.translation_page && view.value("caret_edit_suffix", false);
  output.caret_edit_prefix.clear();
  if (!output.caret_edit_suffix)
    return;
  auto prefix = view.value("caret_edit_prefix", std::string{});
  if (prefix.empty() || prefix.size() > 4096)
    return;
  for (const auto &candidate : view.at("candidates"))
    if (candidate.at("id").at("index").get<size_t>() == 0) {
      output.caret_edit_prefix =
          candidate_output(prefix, traditional_output, candidate.value("source", uint8_t{}));
      return;
    }
}
// 高亮候选确实有第 column 列释义时才算预选，否则为 0：释义刷新、云候选插入后高亮候选可能已经没有那一列，ReplyComposer 上屏时按同一条规则判断。
inline int candidate_armed_gloss_column(const std::vector<PresentationCandidate> &candidates,
                                        int column) {
  if (column <= 0)
    return 0;
  for (const auto &candidate : candidates)
    if (candidate.highlighted)
      return candidate.gloss.empty() && !gloss_column_text(candidate.translation, column).empty()
                 ? column
                 : 0;
  return 0;
}
// 预选的那一列释义在 candidate_secondary_text 里的位置（UTF-8 字节的起点和长度），候选窗据此画下划线。候选带 훈음 时那一段另起一行在上面，这种候选（韩文）不接释义列，不画。
inline std::optional<std::pair<size_t, size_t>>
candidate_gloss_column_range(const PresentationCandidate &candidate, int column) {
  if (column <= 0 || !candidate.gloss.empty())
    return std::nullopt;
  const auto gloss = gloss_column_text(candidate.translation, column);
  if (gloss.empty())
    return std::nullopt;
  // 第 column 行释义的起点按 candidate_secondary_run 报的位置取，不数换行：前一行释义有读音时，读音单独占了一行。
  const auto run = candidate_secondary_run(candidate);
  const auto line = static_cast<size_t>(column - 1);
  if (line >= run.translation_line_starts.size())
    return std::nullopt;
  const size_t start = run.translation_line_starts[line];
  if (run.text.compare(start, gloss.size(), gloss) != 0)
    return std::nullopt;
  return std::make_pair(start, gloss.size());
}
// msime_client_all_candidates 的结果里属于 session、generation 的候选，按下标递增，文字是引擎的原文（繁体换算由 CandidateMailbox::expanded 做）。占位行（选它什么都不上屏）、空的和超过 4096 字节的（候选窗画不了）跳过；身份对不上或下标不递增时整份不要。
inline std::vector<ExpandedCandidate> expanded_candidates_from_snapshot(const nlohmann::json &snapshot,
                                                                        uint64_t session,
                                                                        uint64_t generation) {
  std::vector<ExpandedCandidate> items;
  if (!snapshot.is_object() || snapshot.value("session", uint64_t{0}) != session ||
      snapshot.value("generation", uint64_t{0}) != generation ||
      !snapshot.contains("candidates") || !snapshot.at("candidates").is_array())
    return items;
  items.reserve(snapshot.at("candidates").size());
  for (const auto &candidate : snapshot.at("candidates")) {
    const auto &id = candidate.at("id");
    if (id.at("session").get<uint64_t>() != session ||
        id.at("generation").get<uint64_t>() != generation)
      return {};
    const auto index = id.at("index").get<size_t>();
    if (!items.empty() && index <= items.back().index)
      return {};
    const auto source = candidate.value("source", uint8_t{});
    auto text = candidate.at("text").get<std::string>();
    if (text.empty() || text.size() > 4096 || source == candidate_source_ai_pending ||
        source == candidate_source_cloud_pending)
      continue;
    items.push_back({index, std::move(text), source});
  }
  return items;
}
// Copy the view's page position and page size into `output`, dropping a position that is not a page of the count rather than drawing "4 / 3".
inline void candidate_presentation_page(CandidatePresentation &output,
                                        const nlohmann::json &view) {
  const auto page = view.value("page", size_t{0});
  const auto count = view.value("page_count", size_t{0});
  output.page_size = view.value("page_size", size_t{0});
  if (page < count) {
    output.page = page;
    output.page_count = count;
  }
}
inline CandidatePresentation
candidate_presentation_from_view(const FocusLease &lease,
                                 const nlohmann::json &view, int x, int y,
                                 const std::string &prefix,
                                 bool traditional_output = false) {
  // Named rather than positional: this is an aggregate, so inserting a field
  // above silently shifts every following initializer onto the wrong member.
  CandidatePresentation output{};
  output.lease = lease;
  output.session = view.at("session").get<uint64_t>();
  output.generation = view.at("generation").get<uint64_t>();
  output.translation_page = view.value("translation_page", uint64_t{0});
  output.visible = false;
  output.x = x;
  output.y = y;
  const bool prediction = idle_prediction_view(view);
  if (!view.at("focused").get<bool>() ||
      (view.at("editing_text").get<std::string>().empty() && !prediction)) {
    output.composition_ended = view.at("focused").get<bool>();
    return output;
  }
  output.prediction = prediction;
  const auto text = view.at("preedit").get<std::string>();
  if (prefix.size() > 4096 ||
      text.size() > 4096 - prefix.size() || view.at("candidates").size() > 9)
    throw std::invalid_argument("Oversized candidate presentation");
  output.traditional_output = traditional_output;
  output.preedit = prediction ? std::string{} : prefix + text;
  // caret_position is a byte offset into the Engine's ASCII editing_text. It carries over to the drawn preedit when the two are the same string, or when the preedit is those letters with display separators added (segmented quanpin), see preedit_display_caret; the prefix this host prepends shifts it by its own length.
  const auto editing = view.at("editing_text").get<std::string>();
  const auto caret =
      preedit_display_caret(editing, text, view.value("caret_position", editing.size()));
  if (caret != std::string::npos && !prediction)
    output.preedit_caret = prefix.size() + caret;
  size_t highlighted = 0;
  const bool hanja = korean_hanja_view(view);
  for (const auto &candidate : view.at("candidates")) {
    const auto &id = candidate.at("id");
    const auto candidate_source = candidate.value("source", uint8_t{});
    // 云的占位席位不进候选窗。它是引擎内部用来固定云候选位置的一行（`CandidateSource::CloudPending`，文字就是「☁」），不是候选：选不中，却占着一格和一个编号——用户看到第 2 个数字键永远是废的，它后面每一行的编号还都比看到的大一位。移动三端从一开始就隐藏它并把序号映射回引擎索引（`CandidatePresentationPolicy.isVisible`），桌面端漏了这一半。每行仍带自己的 `index`（引擎身份），所以点击照常；数字键那一侧在 `ServerSession` 里同样只数可见行。
    if (!candidate_source_is_visible(candidate_source))
      continue;
    PresentationCandidate item{
        id.at("session").get<uint64_t>(), id.at("generation").get<uint64_t>(),
        id.at("index").get<size_t>(),
        candidate_output(candidate.at("text").get<std::string>(),
                         traditional_output, candidate_source),
        candidate.at("highlighted").get<bool>(),
        candidate.value("corrected", false),
        candidate.value("annotation", std::string{}),
        candidate_source == 2 ? " ☁️" : candidate_source == 3 ? " 🤖" : "",
        candidate.value("fixed_position", uint8_t{}),
        candidate.value("translation", std::string{}),
        candidate_actions_available(view.value("scheme", 0u), candidate_source)};
    item.pending = candidate_source == candidate_source_ai_pending || candidate_source == candidate_source_cloud_pending;
    // 释义页上的义项不是 Engine 的候选，置顶、删除、固定排位都无从谈起，菜单里不给。
    if (output.translation_page)
      item.actions_available = false;
    item.script_annotation = candidate.value("script_annotation", std::string{});
    item.translation_reserved = candidate.value("translation_reserved", false);
    if (hanja)
      move_korean_hanja_gloss(item);
    if (item.session != output.session ||
        item.generation != output.generation || item.text.size() > 4096 ||
        item.annotation.size() > 4096 || item.gloss.size() > 4096 ||
        item.badge.size() > 4096 ||
        item.translation.size() > 4096 || item.script_annotation.size() > 4096)
      throw std::invalid_argument("Invalid presented candidate");
    item.wubi_code_hint = wubi_code_hint(view, candidate);
    highlighted += item.highlighted;
    output.candidates.push_back(std::move(item));
  }
  if (!output.candidates.empty() && highlighted != 1)
    throw std::invalid_argument("Invalid candidate highlight");
  candidate_presentation_page(output, view);
  candidate_presentation_caret_edit(output, view, traditional_output);
  output.pointer_input = !scheme::KeyboardOnlyCandidateList(
      static_cast<int>(view.value("scheme", 0u)));
  output.shows_glosses = candidate_view_shows_glosses(view);
  output.shuangpin_keymap = shuangpin_keymap_hint(view);
  output.armed_gloss_column =
      candidate_armed_gloss_column(output.candidates, view.value("armed_gloss_column", 0));
  output.visible = true;
  return output;
}
// Value-only projection. Call only from the confirmed-delivery callback.
// A future UI consumer must revalidate focus before displaying or clicking.
inline CandidatePresentation
candidate_presentation(const FocusLease &lease, const PendingReply &reply,
                       const FanyImeNamedpipeData &packet) {
  const auto &source = reply.source;
  if (!lease.epoch || source.client_id != lease.transport.client ||
      source.activation_epoch != lease.epoch ||
      packet.client_id != source.client_id ||
      packet.request_id != source.request_id ||
      packet.event_type != FanyImePipeEventType::KeyEvent)
    throw std::invalid_argument("Invalid candidate presentation identity");
  const auto &view = source.transition.at("view");
  CandidatePresentation output{};
  output.lease = lease;
  output.session = view.at("session").get<uint64_t>();
  output.generation = view.at("generation").get<uint64_t>();
  output.translation_page = view.value("translation_page", uint64_t{0});
  output.x = packet.point[0];
  output.y = packet.point[1];
  output.game_host = (packet.modifiers_down & PipeMetadata::GameHost) != 0;
  const bool prediction = idle_prediction_view(view);
  if (!view.at("focused").get<bool>() ||
      (packet.modifiers_down & FanyImePipeFlags::UiLess) ||
      (view.at("editing_text").get<std::string>().empty() && !prediction)) {
    output.composition_ended = view.at("focused").get<bool>() &&
                               (packet.modifiers_down & FanyImePipeFlags::UiLess) == 0;
    return output;
  }
  output.prediction = prediction;
  const auto text = view.at("preedit").get<std::string>();
  if (reply.next_prefix.size() > 4096 ||
      text.size() > 4096 - reply.next_prefix.size() ||
      view.at("candidates").size() > 9)
    throw std::invalid_argument("Oversized candidate presentation");
  output.traditional_output = reply.traditional_output;
  // 联想没有拼音；上屏的前缀已经进了文档，不再画。
  output.preedit = prediction ? std::string{} : reply.next_prefix + text;
  size_t highlighted = 0;
  const bool hanja = korean_hanja_view(view);
  for (const auto &candidate : view.at("candidates")) {
    const auto &id = candidate.at("id");
    const auto candidate_source = candidate.value("source", uint8_t{});
    // 同上：云的占位席位不进候选窗，编号只数可见行。
    if (!candidate_source_is_visible(candidate_source))
      continue;
    PresentationCandidate item{
        id.at("session").get<uint64_t>(), id.at("generation").get<uint64_t>(),
        id.at("index").get<size_t>(),
        candidate_output(candidate.at("text").get<std::string>(),
                         reply.traditional_output, candidate_source),
        candidate.at("highlighted").get<bool>(),
        candidate.value("corrected", false),
        candidate.value("annotation", std::string{}),
        candidate_source == 2 ? " ☁️" : candidate_source == 3 ? " 🤖" : "",
        candidate.value("fixed_position", uint8_t{}),
        candidate.value("translation", std::string{}),
        candidate_actions_available(view.value("scheme", 0u), candidate_source)};
    item.pending = candidate_source == candidate_source_ai_pending || candidate_source == candidate_source_cloud_pending;
    // 释义页上的义项不是 Engine 的候选，置顶、删除、固定排位都无从谈起，菜单里不给。
    if (output.translation_page)
      item.actions_available = false;
    item.script_annotation = candidate.value("script_annotation", std::string{});
    item.translation_reserved = candidate.value("translation_reserved", false);
    if (hanja)
      move_korean_hanja_gloss(item);
    if (item.session != output.session ||
        item.generation != output.generation || item.text.size() > 4096 ||
        item.annotation.size() > 4096 || item.gloss.size() > 4096 ||
        item.badge.size() > 4096 ||
        item.translation.size() > 4096 || item.script_annotation.size() > 4096)
      throw std::invalid_argument("Invalid presented candidate");
    item.wubi_code_hint = wubi_code_hint(view, candidate);
    highlighted += item.highlighted;
    output.candidates.push_back(std::move(item));
  }
  if (!output.candidates.empty() && highlighted != 1)
    throw std::invalid_argument("Invalid candidate highlight");
  candidate_presentation_page(output, view);
  candidate_presentation_caret_edit(output, view, reply.traditional_output);
  output.pointer_input = !scheme::KeyboardOnlyCandidateList(
      static_cast<int>(view.value("scheme", 0u)));
  output.shows_glosses = candidate_view_shows_glosses(view);
  output.shuangpin_keymap = shuangpin_keymap_hint(view);
  output.armed_gloss_column =
      candidate_armed_gloss_column(output.candidates, view.value("armed_gloss_column", 0));
  output.visible = true;
  return output;
}
} // namespace msime::windows
