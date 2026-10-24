// 脚本插件的注释（view 候选的 script_annotation）进候选窗的投影：和引擎的 annotation 分开放，五笔剩余编码、韩文汉字的 훈음 都不动它。和 macOS 一样，注释那一段只有包在括号里的引擎注释，插件注释单独占释义那一段的最后一行（斜体从它的起点画起），加上去超过候选窗能画的长度时不带它。
#include "CandidatePresentation.h"
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>
#include <utility>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Script annotation presentation test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
nlohmann::json candidate(size_t index, const std::string &text, const std::string &code,
                         const std::string &annotation) {
  return nlohmann::json{{"id", {{"session", 1}, {"generation", 2}, {"index", index}}},
                        {"text", text},
                        {"highlighted", index == 0},
                        {"code", code},
                        {"annotation", annotation},
                        {"translation", ""}};
}
// 全拼的一页：第一行插件和引擎都有注释，第二行只有插件的，第三行没有插件注释（键不出现，和没有插件时的 JSON 一样）。
nlohmann::json pinyin_view() {
  auto first = candidate(0, "你好", "nihao", "ni");
  first["script_annotation"] = "问候";
  auto second = candidate(1, "拟好", "nihao", "");
  second["script_annotation"] = "常用";
  return nlohmann::json{{"session", 1},
                        {"generation", 2},
                        {"focused", true},
                        {"scheme", 0},
                        {"local_mode", "none"},
                        {"preedit", "nihao"},
                        {"editing_text", "nihao"},
                        {"candidates", {first, second, candidate(2, "你号", "nihao", "hao")}}};
}
} // namespace
int main() {
  try {
    const FocusLease lease{{42, {1, 2, 3}}, 1, 1};
    const auto projected = candidate_presentation_from_view(lease, pinyin_view(), 0, 0, "");
    require(projected.visible && projected.candidates.size() == 3);
    // 插件的注释另放一格，引擎的注释原样留着。
    const auto &hello = projected.candidates[0];
    require(hello.annotation == "ni" && hello.script_annotation == "问候");
    require(projected.candidates[1].annotation.empty() && projected.candidates[1].script_annotation == "常用");
    require(projected.candidates[2].annotation == "hao" && projected.candidates[2].script_annotation.empty());
    // 注释那一段：引擎注释包在括号里紧贴候选文字，插件注释不在这里。
    require(candidate_annotation_run(hello) == "(ni)");
    require(candidate_annotation_run(projected.candidates[1]).empty());
    require(candidate_annotation_run(projected.candidates[2]) == "(hao)");
    // 插件注释单独占释义那一段的最后一行；没有释义时它就是唯一的一行，斜体从开头画起。悬停提示里也有它，和 macOS 的 toolTip 一样。
    const auto hello_secondary = candidate_secondary_run(hello);
    require(hello_secondary.text == "问候" && hello_secondary.script_start == 0 &&
            hello_secondary.readings.empty());
    require(candidate_secondary_lines(hello) == 1);
    require(candidate_tooltip_text(hello) == "你好\n问候");
    require(candidate_secondary_run(projected.candidates[2]).script_start == std::string::npos &&
            candidate_secondary_text(projected.candidates[2]).empty());
    // 有释义和读音时插件注释排在它们后面，不挪动它们的位置；读音的位置照样报出来，候选窗按 0.72 倍透明度画它。
    auto glossed = hello;
    glossed.translation = "hello";
    glossed.pronunciation = "/hə/";
    const auto glossed_run = candidate_secondary_run(glossed);
    require(glossed_run.text == "hello  /hə/\n问候");
    require(glossed_run.readings.size() == 1 && glossed_run.readings[0].first == 7 &&
            glossed_run.readings[0].second == std::string("/hə/").size());
    require(glossed_run.script_start == std::string("hello  /hə/\n").size());
    require(candidate_secondary_lines(glossed) == 2);
    // 繁体输出只转候选文字，插件的注释是插件自己的文本，不转。
    const auto traditional = candidate_presentation_from_view(lease, pinyin_view(), 0, 0, "", true);
    require(traditional.candidates[0].script_annotation == "问候");
    // 插件注释收拾成一行：控制字符和换行（含 U+2028）连成的一段换成一个空格，去掉首尾空白。
    require(candidate_script_annotation_line(" 常\n\n用\u2028x\t ") == "常 用 x");
    require(candidate_script_annotation_line("\r\n").empty());
    // 格式字符（双向控制符、零宽字符、BOM）和控制字符一样换成空格，和 macOS 的 controlCharacterSet 一样：U+202E 不能把这一行倒过来画。首尾的全角空格、不换行空格也去掉。
    require(candidate_script_annotation_line("\xE3\x80\x80\xE2\x80\xAE" "abc\xE2\x80\x8B" "def\xEF\xBB\xBF\xC2\xA0") ==
            "abc def");
    require(candidate_script_annotation_line("a\xE2\x81\xA6" "b\xE2\x81\xA9" "c\xC2\xAD" "d") == "a b c d");
    // 别的非 ASCII 文字原样留着，C1 控制字符（U+0085）照旧算换行。
    require(candidate_script_annotation_line("\xE5\xB8\xB8\xC2\x85\xE7\x94\xA8") == "\xE5\xB8\xB8 \xE7\x94\xA8");
    PresentationCandidate multiline{};
    multiline.text = "你";
    multiline.translation = "you";
    multiline.script_annotation = "常\n用";
    require(candidate_secondary_text(multiline) == "you\n常 用" && candidate_secondary_lines(multiline) == 2);
    // 引擎注释的括号：已经带半角或全角括号的不再加，首尾空白去掉，只有空白时不画。
    PresentationCandidate noted{};
    noted.text = "你好";
    for (const auto &[raw, drawn] : std::vector<std::pair<std::string, std::string>>{
             {"wqvb", "(wqvb)"},
             {" wqvb ", "(wqvb)"},
             {"(wqvb)", "(wqvb)"},
             {"（广东）", "（广东）"},
             {"(广东", "((广东)"},
             {"  ", ""},
             {"\xE3\x80\x80wqvb\xC2\xA0", "(wqvb)"},
             {"\xE3\x80\x80\xE2\x80\x83\xC2\xA0", ""},
             {"", ""}}) {
      noted.annotation = raw;
      require(candidate_annotation_run(noted) == drawn);
    }

    // 送达按键时的投影（Server 画候选窗用的那一条）读同一个键。
    PendingReply reply{};
    reply.source = {42, 1, 7, false, nlohmann::json{{"commit", nullptr}, {"view", pinyin_view()}}};
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.request_id = 7;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    const auto delivered = candidate_presentation(lease, reply, packet);
    require(delivered.candidates[0].annotation == "ni" && delivered.candidates[0].script_annotation == "问候");
    require(delivered.candidates[2].script_annotation.empty());

    // 五笔剩余编码换掉的是引擎的注释，本来就带括号，不再加一层；插件的注释照样在释义那一段。
    auto wubi = pinyin_view();
    wubi["scheme"] = 2;
    wubi["preedit"] = "wq";
    wubi["editing_text"] = "wq";
    wubi["candidates"] = nlohmann::json::array({candidate(0, "你", "wqiy", "")});
    wubi["candidates"][0]["script_annotation"] = "常用";
    const auto hinted = with_wubi_code_hints(candidate_presentation_from_view(lease, wubi, 0, 0, ""), true);
    require(hinted.candidates[0].annotation == "(iy)" && hinted.candidates[0].script_annotation == "常用");
    require(candidate_annotation_run(hinted.candidates[0]) == "(iy)");
    require(candidate_secondary_text(hinted.candidates[0]) == "常用");

    // 韩文汉字的 훈음 从注释挪到释义一行，插件的注释排在 훈음 后面另起一行。
    auto hanja = pinyin_view();
    hanja["scheme"] = 4;
    hanja["dedicated_english"] = false;
    hanja["preedit"] = "한";
    hanja["editing_text"] = "gks";
    hanja["candidates"] = nlohmann::json::array({candidate(0, "韓", "gks", "나라 이름 한")});
    hanja["candidates"][0]["script_annotation"] = "常用";
    const auto korean = candidate_presentation_from_view(lease, hanja, 0, 0, "");
    require(korean.candidates[0].annotation.empty() && korean.candidates[0].gloss == "나라 이름 한");
    require(korean.candidates[0].script_annotation == "常用");
    require(candidate_annotation_run(korean.candidates[0]).empty());
    const auto korean_run = candidate_secondary_run(korean.candidates[0]);
    require(korean_run.text == "나라 이름 한\n常用" &&
            korean_run.script_start == std::string("나라 이름 한\n").size());

    // 释义加上插件注释超过 4096 字节时只画释义，插件的注释不让这一段画不出来；正好 4096 时两段都画。
    PresentationCandidate crowded{};
    crowded.text = "长";
    crowded.translation = std::string(4090, 'a');
    crowded.script_annotation = "常用";
    const auto dropped = candidate_secondary_run(crowded);
    require(dropped.text == crowded.translation && dropped.script_start == std::string::npos);
    crowded.translation = std::string(4089, 'a');
    const auto fitted = candidate_secondary_run(crowded);
    require(fitted.text.size() == 4096 && fitted.script_start == 4090);

    // 和别的字段一样，超过 4096 字节的插件注释让整个投影作废，不画半截。
    auto oversized = pinyin_view();
    oversized["candidates"][0]["script_annotation"] = std::string(4097, 'x');
    bool rejected = false;
    try {
      (void)candidate_presentation_from_view(lease, oversized, 0, 0, "");
    } catch (const std::invalid_argument &) {
      rejected = true;
    }
    require(rejected);
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  std::cout << "Script annotation presentation checks passed\n";
  return 0;
}
