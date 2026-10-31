// Real Fcitx input contexts and the real Host API. All input is synthetic.
#include "../FcitxEngine.cpp"
#include "../../tests/voice/voice_provider_fixture.h"
#include <fcitx-config/option.h>
#include <array>
#include <cstdlib>
#include <iostream>
#include <filesystem>
#include <functional>
#include <stdexcept>
#include <thread>
#include <chrono>
#include <limits>
#include <map>
#include <tuple>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <sys/stat.h>
#include <cstring>
#include <poll.h>

#ifdef MSIME_FCITX5_HINT_FONT
// 只统计测试程序自身创建的字体映射，不拦截动态加载的 classicui：重复提示不能重新排版。
static int hint_font_map_creations = 0;
extern "C" PangoFontMap *__real_pango_cairo_font_map_new();
extern "C" PangoFontMap *__wrap_pango_cairo_font_map_new() {
  ++hint_font_map_creations;
  return __real_pango_cairo_font_map_new();
}
#endif

// 只替换测试程序调用的版本入口，模拟旧开发库构建的插件在新运行库上启动。
static const char *font_runtime_version = nullptr;
extern "C" const char *__real__ZN5fcitx8Instance7versionEv();
extern "C" const char *__wrap__ZN5fcitx8Instance7versionEv() {
  return font_runtime_version ? font_runtime_version : __real__ZN5fcitx8Instance7versionEv();
}

using namespace msime::fcitx_host;
class FixtureContext : public fcitx::InputContext {
public:
  explicit FixtureContext(fcitx::InputContextManager &manager, const std::string &program = "msime-test")
      : InputContext(manager, program) { created(); }
  ~FixtureContext() override { destroy(); }
  const char *frontend() const override { return "msime-test"; }
  void commitStringImpl(const std::string &text) override {
    committed += text;
    if (on_commit) on_commit();
  }
  void deleteSurroundingTextImpl(int, unsigned int) override {}
  void forwardKeyImpl(const fcitx::ForwardKeyEvent &event) override {
    forwarded.emplace_back(event.rawKey(), event.isRelease());
  }
  void updatePreeditImpl() override {}
  std::string committed;
  std::function<void()> on_commit;
  std::vector<std::pair<fcitx::Key, bool>> forwarded;
};
void require(bool ok, const char *message) { if (!ok) throw std::runtime_error(message); }
void removeFixtureTree(const std::filesystem::path &directory) {
  // A detached private-file write may rename its temporary file while remove_all walks the tree.
  // Retry only that transient missing-entry error and still require the fixture to be gone.
  for (int attempt = 0; attempt < 20; ++attempt) {
    std::error_code error;
    std::filesystem::remove_all(directory, error);
    if (error && error != std::errc::no_such_file_or_directory)
      throw std::filesystem::filesystem_error("remove fixture", directory, error);
    if (!std::filesystem::exists(directory)) return;
    std::this_thread::sleep_for(std::chrono::milliseconds(10));
  }
  require(!std::filesystem::exists(directory), "native fixture directory removed");
}
// The autocorrect marker is display-only: Windows appends '*' to the row text of a candidate whose spelling the Engine corrected, and the IBus host does the same. It sits right after the word and before the cloud/AI badge, and the text the candidate selects with stays the Engine's. Runs before the resource fixture so it needs nothing but the plugin code.
void autocorrectMarker() {
  // FcitxCandidate::text() is the Engine text it selects with; the row the panel draws is the base class's.
  const auto shown = [](const FcitxCandidate &word) {
    return static_cast<const fcitx::CandidateWord &>(word).text().toString();
  };
  const auto row = [](Json candidate, bool traditional = false) {
    candidate["id"] = Json{{"session", 1}, {"generation", 1}, {"index", 0}};
    return FcitxCandidate(nullptr, candidate, traditional, false, std::string());
  };
  const auto corrected = row(Json{{"text", "你好"}, {"corrected", true}});
  require(shown(corrected) == "你好*", "corrected candidate shows the marker");
  require(corrected.text() == "你好", "marker stays out of the selected text");
  require(shown(row(Json{{"text", "你好"}, {"corrected", false}})) == "你好", "uncorrected candidate has no marker");
  require(shown(row(Json{{"text", "你好"}})) == "你好", "a view without the field has no marker");
  require(shown(row(Json{{"text", "在线"}, {"corrected", true}, {"source", 2}})) == "在线* ☁️",
          "marker comes before the cloud badge, one space before it as macOS draws it");
  require(shown(row(Json{{"text", "智能"}, {"source", 3}})) == "智能 🤖", "the AI badge is one space after the word");
  require(shown(row(Json{{"text", "汉语"}, {"corrected", true}}, true)) == "漢語*",
          "marker follows the traditional-converted word");
  // 候选行按会话的繁体变体转换；繁体关着时变体不起作用。
  const auto variantRow = [](bool traditional, uint32_t variant) {
    return FcitxCandidate(nullptr,
                          Json{{"text", "软件里面"}, {"id", Json{{"session", 1}, {"generation", 1}, {"index", 0}}}},
                          traditional, false, std::string(), variant);
  };
  require(shown(variantRow(true, 0)) == "軟件裏面", "s2t row");
  require(shown(variantRow(true, 1)) == "軟件裡面", "s2tw row uses Taiwan glyphs");
  require(shown(variantRow(true, 2)) == "軟體裡面", "s2twp row uses Taiwan phrases");
  require(shown(variantRow(false, 2)) == "软件里面", "variant is ignored while traditional output is off");
  require(variantRow(true, 2).text() == "软件里面", "the selected text stays the Engine's");
}
// A Hanja row's 훈음 is drawn as the row's secondary gloss: its own italic, uncommittable segment where a translation goes, shown whatever the annotation and translation settings say, with a translation after it. Any other annotation stays plain text gated by the annotation setting. Runs before the resource fixture.
void koreanHanjaGlossRow() {
  const auto word = [](Json candidate, bool annotations, const std::string &gloss) {
    candidate["id"] = Json{{"session", 1}, {"generation", 1}, {"index", 0}};
    return FcitxCandidate(nullptr, candidate, false, annotations, gloss);
  };
  const auto shown = [](const FcitxCandidate &candidate) -> const fcitx::Text & {
    return static_cast<const fcitx::CandidateWord &>(candidate).text();
  };
  const fcitx::TextFormatFlags secondary{fcitx::TextFormatFlag::Italic, fcitx::TextFormatFlag::DontCommit};
  const Json hanja{{"text", "韓"}, {"annotation", "나라 이름 한, 한나라 한"}};
  const auto plain = word(hanja, false, "나라 이름 한, 한나라 한");
  require(shown(plain).toString() == "韓  나라 이름 한, 한나라 한", "a Hanja row shows its 훈음 even with annotations off");
  require(shown(plain).size() == 2 && shown(plain).stringAt(0) == "韓" &&
              shown(plain).formatAt(0) == fcitx::TextFormatFlags(fcitx::TextFormatFlag::NoFlag) &&
              shown(plain).stringAt(1) == "  나라 이름 한, 한나라 한" && shown(plain).formatAt(1) == secondary,
          "the 훈음 is a separate italic segment, never committed");
  require(plain.text() == "韓", "the 훈음 stays out of the selected text");
  auto translated = hanja;
  translated["translation"] = "Korea";
  const auto glossed = word(translated, true, "나라 이름 한, 한나라 한");
  require(shown(glossed).toString() == "韓  나라 이름 한, 한나라 한\nKorea" && shown(glossed).size() == 3 &&
              shown(glossed).formatAt(1) == secondary &&
              shown(glossed).formatAt(2) == fcitx::TextFormatFlags(fcitx::TextFormatFlag::NoFlag),
          "a translation follows the 훈음 and the annotation is not drawn twice");
  require(glossed.display().body == "韓  나라 이름 한, 한나라 한" && glossed.display().translation == "Korea",
          "原生画布和 classicui 保留同一训音与释义，候选正文不含释义");
  require(glossed.display().word == "韓" && glossed.display().reading == "나라 이름 한, 한나라 한",
          "原生画布的 macOS 结构把训音单独交给释义行的第一行，正文不带它");
  const auto helpcode = word(Json{{"text", "你"}, {"annotation", "ab"}}, true, std::string());
  require(shown(helpcode).toString() == "你(ab)" && shown(helpcode).size() == 1,
          "a helpcode stays plain row text, parenthesized right after the word as macOS draws it");
  require(shown(word(Json{{"text", "你"}, {"annotation", " (ab) "}}, true, std::string())).toString() == "你(ab)",
          "an annotation already in parentheses is not wrapped twice");
  require(shown(word(Json{{"text", "云"}, {"annotation", "ab"}, {"source", 2}}, true, std::string())).toString() == "云(ab) ☁️",
          "the badge follows the annotation");
  require(word(Json{{"text", "你"}, {"annotation", "ab"}}, false, std::string()).text() == "你" &&
              shown(word(Json{{"text", "你"}, {"annotation", "ab"}}, false, std::string())).toString() == "你",
          "annotations off still hide a helpcode");
  // 固定了排位的行：classicui 画不出单行颜色，正文后加只供显示的「  固定N」；原生画布拿到不带它的正文和 fixed 标记，自己画 #379AD3。
  const auto fixed = word(Json{{"text", "你"}, {"annotation", "ab"}, {"fixed_position", 2}, {"translation", "you"}},
                          true, std::string());
  require(shown(fixed).toString() == "你(ab)  固定2\nyou" && shown(fixed).size() == 3 &&
              shown(fixed).stringAt(1) == "  固定2" &&
              shown(fixed).formatAt(1) == fcitx::TextFormatFlags(fcitx::TextFormatFlag::DontCommit),
          "a fixed row carries a display-only position marker before its translation");
  require(fixed.display().body == "你(ab)" && fixed.display().fixed && fixed.text() == "你",
          "the native canvas gets the row without the marker, and selection keeps the Engine text");
  for (const int position : {0, 6}) {
    const auto unfixed = word(Json{{"text", "你"}, {"fixed_position", position}}, true, std::string());
    require(shown(unfixed).toString() == "你" && !unfixed.display().fixed, "only positions 1-5 are marked");
  }
}
// 脚本插件的注释（`script_annotation`）在行尾单独一段，括在六角括号里、标 DontCommit：辅助码设置关着也画，不替换引擎注释，也不进训音；选中用的文本不变，占位行和不合规的文本不画。和上面两个一样不需要资源 fixture。
// 光标后编辑：首选开头固定的那几个字交给原生画布画淡（dim_prefix_bytes，按转换后的文字计字节）；classicui 的 fcitx::Text 没有颜色格式，那一行照常是一段不带格式的文字。别的行和没有生效时都不画淡。
void caretEditSuffixRow() {
  namespace host = msime::linux_host;
  const auto mode = host::caret_edit_suffix(Json{{"caret_edit_suffix", true}, {"caret_edit_prefix", "我好现"}});
  const auto row = [&](const char *text, size_t index, bool traditional = false) {
    const Json candidate{{"text", text}, {"id", Json{{"session", 1}, {"generation", 1}, {"index", index}}}};
    return FcitxCandidate(nullptr, candidate, traditional, false, std::string(), 0,
                          host::caret_edit_dim_scalars(mode, candidate));
  };
  const auto first = row("我好现吃火锅", 0);
  require(first.display().dim_prefix_bytes == std::string("我好现").size(),
          "the fixed part of the first candidate is dimmed on the native canvas");
  const auto &classic = static_cast<const fcitx::CandidateWord &>(first).text();
  require(classic.toString() == "我好现吃火锅" && classic.size() == 1 &&
              classic.formatAt(0) == fcitx::TextFormatFlags(fcitx::TextFormatFlag::NoFlag),
          "classicui draws the first candidate as one plain segment");
  require(first.text() == "我好现吃火锅", "the selected text stays the Engine's");
  require(row("我好现吃火锅", 0, true).display().dim_prefix_bytes == 9,
          "the dimmed range is counted on the traditional-converted text");
  require(row("吃火锅", 1).display().dim_prefix_bytes == 0, "other rows are not dimmed");
  const Json plain{{"text", "我好现吃火锅"}, {"id", Json{{"session", 1}, {"generation", 1}, {"index", 0}}}};
  require(FcitxCandidate(nullptr, plain, false, false, std::string()).display().dim_prefix_bytes == 0,
          "nothing is dimmed while the mode is off");
}
void scriptAnnotationRow() {
  const auto word = [](Json candidate, bool annotations, const std::string &gloss = std::string()) {
    candidate["id"] = Json{{"session", 1}, {"generation", 1}, {"index", 0}};
    return FcitxCandidate(nullptr, candidate, false, annotations, gloss);
  };
  const auto shown = [](const FcitxCandidate &candidate) -> const fcitx::Text & {
    return static_cast<const fcitx::CandidateWord &>(candidate).text();
  };
  const fcitx::TextFormatFlags displayOnly{fcitx::TextFormatFlag::DontCommit};
  const auto off = word(Json{{"text", "你"}, {"annotation", "ab"}, {"script_annotation", "常用"}}, false);
  require(shown(off).toString() == "你  〔常用〕" && shown(off).size() == 2 &&
              shown(off).stringAt(1) == "  〔常用〕" && shown(off).formatAt(1) == displayOnly,
          "a script annotation shows with annotations off, as its own display-only segment");
  require(off.text() == "你", "the script annotation stays out of the selected text");
  const auto both = word(Json{{"text", "你"}, {"annotation", "ab"}, {"script_annotation", "常用"},
                              {"translation", "you"}}, true);
  require(shown(both).toString() == "你(ab)\nyou  〔常用〕",
          "the engine annotation and the translation stay, the script annotation comes last");
  require(both.display().body == "你(ab)" && both.display().translation == "you" &&
              both.display().suffix == "  〔常用〕" && both.text() == "你",
          "原生画布独立保留正文、释义与插件后缀，上屏文字不变");
  require(both.display().word == "你(ab)" && both.display().script_annotation == "常用" && !both.display().pending,
          "原生画布的 macOS 结构拿到收拾过的插件注释，画在释义行最后一行");
  const Json runs = Json::array({Json{{"start", 0}, {"length", 2}, {"role", "pos"}}});
  const auto segmented = word(Json{{"text", "你"}, {"translation", "n. synthetic"}, {"translation_runs", runs}}, false);
  require(segmented.display().translation_runs == runs && segmented.display().translation == "n. synthetic",
          "原生画布接收共享层的同源释义分段，不在宿主重新解析");
  const auto lines = shown(segmented).splitByLine();
  require(lines.size() == 2 && lines[0].toString() == "你" && lines[1].toString() == "n. synthetic",
          "classicui 通过真实 Text 分行把译文放到候选下方");
  const auto empty = word(Json{{"text", "你"}, {"translation", ""}}, false);
  require(shown(empty).toString() == "你" && shown(empty).splitByLine().size() == 1,
          "空译文不增加空白行");
  const auto hanja = word(Json{{"text", "韓"}, {"annotation", "나라 이름 한"}, {"script_annotation", "注"}},
                          false, "나라 이름 한");
  require(shown(hanja).toString() == "韓  나라 이름 한  〔注〕",
          "a Hanja row keeps its 훈음 and draws the script annotation after it");
  require(shown(word(Json{{"text", "…"}, {"source", 12}, {"script_annotation", "注"}}, true)).toString() == "…",
          "the AI placeholder draws no script annotation");
  require(shown(word(Json{{"text", "你"}, {"script_annotation", "a\nb"}}, true)).toString() == "你",
          "a script annotation that would break the row is not drawn");
  require(shown(word(Json{{"text", "你"}, {"script_annotation", std::string(33, 'a')}}, true)).toString() == "你",
          "an over-long script annotation is not drawn");
  require(shown(word(Json{{"text", "你"}, {"annotation", "ab"}}, true)).toString() == "你(ab)",
          "a row without a script annotation is unchanged");
}
// An installed skin's decoration reaches the classic UI as the msime theme's overlay, with the image copied beside theme.conf; a built-in skin, or a switch back to one, leaves no image behind. This composes the theme exactly as applyCandidatePanelTheme does, against the shared layer's built-in catalogue, but writes it without the classic UI addon, which the fixture instance does not load. Runs before the resource fixture.
void candidateThemeDecoration() {
  namespace host = msime::linux_host;
  char temporary[] = "/tmp/msime-fcitx5-theme-XXXXXX";
  const auto *directory = mkdtemp(temporary);
  require(directory != nullptr, "theme fixture directory");
  const std::filesystem::path root(directory);
  const auto image = root / "skins" / "sakura" / "images" / "ears.png";
  std::filesystem::create_directories(image.parent_path());
  {
    std::ofstream out(image, std::ios::binary);
    // PNG signature and IHDR header of a 32 x 12 image.
    out << std::string("\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x20\0\0\0\x0c\x08\x06\0\0\0", 29);
  }
  const Json catalog = {{"packages", Json::array({{{"id", "sakura"},
                                                   {"title", "樱花"},
                                                   {"base", "system"},
                                                   {"layouts", Json::array({"horizontal", "vertical"})},
                                                   {"candidate", {{"light", Json::object()}}},
                                                   {"decoration_top_dip", 24.5},
                                                   {"decoration_width_dip", 180},
                                                   {"decoration_image", image.string()}}})}};
  const auto themeFor = [&](const Json &preferences) {
    const auto theme = resolveCandidateTheme(preferences, false, catalog);
    const auto decoration = host::candidate_skin_decoration(catalog, theme.candidate_skin);
    const auto file = host::fcitx_theme_file((root / "data").c_str(), nullptr);
    require(file && host::write_fcitx_candidate_theme(*file, theme.colors, theme.dark, decoration), "candidate theme written");
    std::ifstream in(*file, std::ios::binary);
    return std::string(std::istreambuf_iterator<char>(in), {});
  };
  const auto themeDirectory = root / "data" / "fcitx5" / "themes" / "msime";
  const auto copies = [&] {
    std::vector<std::string> names;
    for (const auto &entry : std::filesystem::directory_iterator(themeDirectory))
      if (entry.path().filename().string().rfind("decoration-", 0) == 0) names.push_back(entry.path().filename().string());
    return names;
  };
  // The shared layer reports the package as drawn (candidate_skin) only because the manifest declares this layout and mode; that is what brings its decoration.
  const auto decorated = themeFor(Json{{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "sakura"}}}});
  const auto named = decorated.find("\nOverlay=decoration-");
  require(named != std::string::npos, "decorated skin names its overlay");
  const auto copy = decorated.substr(named + 9, decorated.find('\n', named + 1) - named - 9);
  require(copies() == std::vector<std::string>{copy}, "overlay image staged beside theme.conf");
  require(std::filesystem::file_size(themeDirectory / copy) == std::filesystem::file_size(image), "overlay is the skin's image");
  require(decorated.find("Gravity=Top Right\nOverlayOffsetX=16\nOverlayOffsetY=25\nHideOverlayIfOversize=False\n") !=
              std::string::npos,
          "overlay pinned top right, its bottom one padding below the card top (8 + 25 + 4 - 12)");
  require(decorated.find("[InputPanel/ContentMargin]\nLeft=16\nRight=16\nTop=37\n") != std::string::npos,
          "band reserved above the candidates");
  require(decorated.find("[InputPanel/ShadowMargin]\nLeft=12\nRight=12\nTop=33\n") != std::string::npos,
          "band counted in the shadow margin, so X11 places the card at the cursor");
  // The rounded card is an image generated beside theme.conf.
  const auto card = decorated.find("[InputPanel/Background]\nImage=shape-");
  require(card != std::string::npos, "card drawn from a generated image");
  const auto cardImage = decorated.substr(card + 30, decorated.find('\n', card + 30) - card - 30);
  require(std::filesystem::is_regular_file(themeDirectory / cardImage), "card image written beside theme.conf");
  const auto plain = themeFor(Json{{"global_theme", "paper"}});
  require(plain.find("Overlay") == std::string::npos, "built-in theme has no overlay");
  require(copies().empty(), "previous skin's overlay removed");
  std::filesystem::remove_all(root);
}
// The mode badge takes the macOS badge's mode rule and the resolved theme's palette. Runs before the resource fixture.
void modeBadgeTheme() {
  const auto dark = [](const Json &preferences, bool system_dark) {
    return fcitx_mode_badge_theme(preferences, system_dark, Json()).dark;
  };
  // 角标跟随颜色模式：全局的 "system" 跟随桌面，浅色桌面得到浅色角标。
  require(!dark(Json{{"theme", "system"}}, false), "system on a light desktop gives a light badge");
  require(dark(Json{{"theme", "system"}}, true), "system on a dark desktop gives a dark badge");
  require(dark(Json{{"theme", "dark"}}, false), "the badge follows a dark colour mode");
  // 存下的 toolbar_theme 不再起作用（各界面一律跟随颜色模式），候选窗的明暗也不参与。
  require(dark(Json{{"toolbar_theme", "light"}, {"theme", "dark"}, {"candidate_theme", "light"}}, false),
          "a stored light toolbar theme is ignored");
  require(!dark(Json{{"toolbar_theme", "dark"}, {"theme", "light"}, {"candidate_theme", "dark"}}, true),
          "a stored dark toolbar theme is ignored");
  require(!dark(Json{{"toolbar_theme", "follow"}, {"theme", "system"}, {"candidate_theme", "dark"}}, false),
          "the candidate mode does not colour the badge");
  // A global theme with a fixed appearance draws the panel in it, so the badge follows it too.
  require(dark(Json{{"global_theme", "ink"}, {"toolbar_theme", "light"}}, false), "a dark global theme gives a dark badge");
  require(!dark(Json{{"global_theme", "paper"}, {"toolbar_theme", "dark"}}, true), "a light global theme gives a light badge");
  // The colours are the resolved theme's card, the same the candidate panel draws in that mode.
  for (const auto &preferences : {Json{{"global_theme", "ink"}}, Json{{"global_theme", "paper"}}, Json::object()}) {
    const auto badge = msime::linux_host::floating_surface_colors(fcitx_mode_badge_theme(preferences, false, Json()));
    const auto panel = resolveCandidateTheme(preferences, false, Json()).colors;
    require(badge.surface == *panel.background && badge.text == *panel.text && badge.accent == *panel.accent,
            "the badge draws the panel's surface, text and accent");
    require(badge.border == (panel.border_width > 0 ? panel.border : std::nullopt), "the badge outline is the panel's");
  }
  // The voice overlay resolves the same theme in its own mode.
  const auto voice = resolveVoiceOverlayTheme(Json{{"global_theme", "ink"}, {"voice_theme", "light"}}, false, Json());
  require(voice.dark, "a fixed-appearance theme overrides the voice overlay's mode too");
  // 语音浮层同样跟随颜色模式，存下的 voice_theme 被忽略。
  require(!resolveVoiceOverlayTheme(Json{{"voice_theme", "dark"}, {"theme", "light"}}, true, Json()).dark,
          "a stored dark voice theme is ignored");
  require(resolveVoiceOverlayTheme(Json{{"voice_theme", "light"}, {"theme", "dark"}}, false, Json()).dark,
          "a stored light voice theme is ignored");
}
// classicui's options belong to every input method, so the first takeover records what it replaced for msime-linux-setup --unregister, and a later write keeps that value while the option still holds MSIME's. Runs against a scratch XDG_STATE_HOME before the resource fixture; the fixture instance does not load classicui, so this drives the recording step the addon's writes go through.
void classicuiTakeoverRecord() {
  char temporary[] = "/tmp/msime-fcitx5-restore-XXXXXX";
  const auto *directory = mkdtemp(temporary);
  require(directory != nullptr, "restore fixture directory");
  const std::filesystem::path root(directory);
  const auto *saved = std::getenv("XDG_STATE_HOME");
  const std::optional<std::string> savedStateHome = saved ? std::optional<std::string>(saved) : std::nullopt;
  setenv("XDG_STATE_HOME", (root / "state").c_str(), 1);
  const auto record = root / "state" / "msime-client" / "panel-restore.json";
  const auto read = [&] {
    std::ifstream in(record);
    return Json::parse(in);
  };
  fcitx::RawConfig stock;
  stock.setValueByPath("Theme", "default");
  stock.setValueByPath("DarkTheme", "default-dark");
  stock.setValueByPath("Font", "Sans 10");
  fcitx::RawConfig theme;
  theme.setValueByPath("Theme", std::string(msime::linux_host::kFcitxCandidateTheme));
  record_classicui_takeover(stock, theme);
  require(read() == Json{{"fcitx5", {{"Theme", {{"prior", "default"}, {"written", "msime"}}}}}},
          "first takeover records the stock theme it replaced");
  // A skin change writes the theme again: the stock theme is still the one to restore.
  fcitx::RawConfig taken;
  taken.setValueByPath("Theme", "msime");
  taken.setValueByPath("DarkTheme", "default-dark");
  taken.setValueByPath("Font", "Sans 10");
  theme.setValueByPath("DarkTheme", "msime");
  record_classicui_takeover(taken, theme);
  fcitx::RawConfig font;
  font.setValueByPath("Font", "Noto Sans SC 18px");
  record_classicui_takeover(taken, font);
  require(read() == Json{{"fcitx5",
                          {{"Theme", {{"prior", "default"}, {"written", "msime"}}},
                           {"DarkTheme", {{"prior", "default-dark"}, {"written", "msime"}}},
                           {"Font", {{"prior", "Sans 10"}, {"written", "Noto Sans SC 18px"}}}}}},
          "later writes keep the replaced values and record each option on its first change");
  // An earlier build set MSIME's theme without keeping a record: the stock themes it stands in for are recorded, since uninstall removes MSIME's.
  std::filesystem::remove(record);
  fcitx::RawConfig upgraded;
  upgraded.setValueByPath("Theme", "msime");
  upgraded.setValueByPath("DarkTheme", "msime");
  record_classicui_takeover(upgraded, theme);
  require(read() == Json{{"fcitx5",
                          {{"Theme", {{"prior", "default"}, {"written", "msime"}}},
                           {"DarkTheme", {{"prior", "default-dark"}, {"written", "msime"}}}}}},
          "a theme option already naming MSIME's theme records the stock theme");
  if (savedStateHome) setenv("XDG_STATE_HOME", savedStateHome->c_str(), 1);
  else unsetenv("XDG_STATE_HOME");
  std::filesystem::remove_all(root);
}

// 经典界面的替身：与 fcitx5 的 ClassicUI 一样，`setConfig` 载入后立即把整份配置写回 conf/classicui.conf。真实的 `getConfig()` 每次都扫描并解析全部已装主题（#5988），这里只数它被调了几次。
FCITX_CONFIGURATION(FakeClassicUiConfig,
                    fcitx::Option<std::string> theme{this, "Theme", "Theme", "default"};
                    fcitx::Option<std::string> darkTheme{this, "DarkTheme", "Dark Theme", "default-dark"};);
class FakeClassicUi : public fcitx::AddonInstance {
public:
  const fcitx::Configuration *getConfig() const override {
    ++scans;
    return &config;
  }
  void setConfig(const fcitx::RawConfig &raw) override {
    ++writes;
    config.load(raw, true);
    fcitx::safeSaveAsIni(config, "conf/classicui.conf");
  }
  FakeClassicUiConfig config;
  mutable int scans = 0;
  int writes = 0;
};
// 每 250 ms 一拍的主题同步（#5988）：用户选了第三方主题时只看一次经典界面的现值，之后的每一拍既不再调 `getConfig()` 也不栅格化；在 fcitx5-configtool 里改回默认主题后下一拍恢复接管；接管之后的拍子不重写主题，卸载还原或用户手改 classicui.conf 之后只重读一次现值刷新提示，不再接管回去；配色变化照常重写；写主题失败不在每一拍上重试，到点再试。classicui.conf 落在 main 开头指定的临时 XDG_CONFIG_HOME 里，主题和接管记录写进这里的临时目录。
void classicuiThemeTicks(FcitxEngine &engine) {
  namespace host = msime::linux_host;
  char temporary[] = "/tmp/msime-fcitx5-ticks-XXXXXX";
  const auto *directory = mkdtemp(temporary);
  require(directory != nullptr, "tick fixture directory");
  const std::filesystem::path root(directory);
  const auto saved = [](const char *name) {
    const auto *value = std::getenv(name);
    return value ? std::optional<std::string>(value) : std::nullopt;
  };
  const auto savedDataHome = saved("XDG_DATA_HOME");
  const auto savedStateHome = saved("XDG_STATE_HOME");
  const auto savedAutogenerate = saved("MSIME_FCITX5_THEME_AUTOGENERATE");
  unsetenv("MSIME_FCITX5_THEME_AUTOGENERATE");
  setenv("XDG_DATA_HOME", (root / "data").c_str(), 1);
  setenv("XDG_STATE_HOME", (root / "state").c_str(), 1);
  const auto *configHome = std::getenv("XDG_CONFIG_HOME");
  require(configHome != nullptr, "the test runs with a scratch XDG_CONFIG_HOME");
  const auto classicuiConf = std::filesystem::path(configHome) / "fcitx5/conf/classicui.conf";
  const auto themeFile = root / "data/fcitx5/themes" / std::string(host::kFcitxCandidateTheme) / "theme.conf";
  const auto inode = [&] {
    struct stat info {};
    return ::stat(themeFile.c_str(), &info) == 0 ? info.st_ino : 0;
  };
  const auto tick = [&](FakeClassicUi &classicui, const Json &preferences, bool dark) {
    engine.applyCandidatePanelTheme(&classicui, preferences, dark, Json(), false);
  };
  FakeClassicUi classicui;
  classicui.config.theme.setValue("nord");
  require(fcitx::safeSaveAsIni(classicui.config, "conf/classicui.conf") && std::filesystem::is_regular_file(classicuiConf),
          "classicui.conf is read from the scratch config home");
  // 默认值会被 fcitx 写成注释；落盘缺失 DarkTheme 等同于 default-dark。
  require(read_classicui_theme_selection().theme == "nord" &&
              read_classicui_theme_selection().dark_theme.value_or("default-dark") == "default-dark",
          "the persisted selection is what classicui saved");
  engine.candidate_theme_applied_.clear();
  engine.candidate_theme_attempt_.clear();
  const Json ink{{"global_theme", "ink"}};
  for (int count = 0; count < 4; ++count) tick(classicui, ink, false);
  require(classicui.scans == 1 && classicui.writes == 0 && !std::filesystem::exists(themeFile),
          "a third-party classicui theme is looked at once, not scanned and rasterized on every tick");
  require(engine.candidate_theme_retry_at_ == std::chrono::steady_clock::time_point::max(),
          "a third-party theme waits for the selection to change, not for a timer");
  // fcitx5-configtool 把主题改回默认：它同样经 setConfig 落盘。
  fcitx::RawConfig stock;
  stock.setValueByPath("Theme", "default");
  classicui.setConfig(stock);
  tick(classicui, ink, false);
  require(classicui.scans == 2 && classicui.writes == 2 &&
              classicui.config.theme.value() == host::kFcitxCandidateTheme &&
              classicui.config.darkTheme.value() == host::kFcitxCandidateTheme && std::filesystem::exists(themeFile),
          "switching classicui back to a stock theme resumes the takeover on the next tick");
  const auto applied = inode();
  for (int count = 0; count < 4; ++count) tick(classicui, ink, false);
  require(classicui.scans == 2 && classicui.writes == 2 && inode() == applied,
          "ticks after the takeover neither scan the themes nor write the theme again");
  tick(classicui, Json{{"global_theme", "paper"}}, false);
  require(classicui.scans == 3 && classicui.writes == 3 && inode() != applied, "a palette change writes the theme again");
  const Json paper{{"global_theme", "paper"}};
  // 卸载时 `msime-linux-setup --unregister` 经 D-Bus 的 SetConfig 把主题还原成默认，同样落盘；仍在运行的插件不能在下一拍又把它接管回去。
  classicui.setConfig(stock);
  for (int count = 0; count < 4; ++count) tick(classicui, paper, false);
  require(classicui.scans == 4 && classicui.writes == 4 && classicui.config.theme.value() == "default",
          "还原主题只重读一次提示配置，不重新接管或逐拍扫描");
  // fcitx5 运行时用户手改 classicui.conf（第三方主题的安装说明常这么写，改完再重启 fcitx5）：同样不去覆盖。
  FakeClassicUiConfig edited;
  edited.theme.setValue("Material-Color-Pink");
  require(fcitx::safeSaveAsIni(edited, "conf/classicui.conf"), "classicui.conf edited by hand");
  for (int count = 0; count < 4; ++count) tick(classicui, paper, false);
  require(classicui.scans == 5 && classicui.writes == 4 && read_classicui_theme_selection().theme == "Material-Color-Pink",
          "手改主题只重读一次提示配置，不重写或逐拍扫描");
  // 主题目录的位置被一个普通文件占住，写主题失败：不在之后的每一拍上重试，到了重试时间才再试，目录恢复可写后自己接上。
  std::filesystem::create_directories(root / "blocked/fcitx5/themes");
  std::ofstream(root / "blocked/fcitx5/themes" / std::string(host::kFcitxCandidateTheme)) << "synthetic";
  setenv("XDG_DATA_HOME", (root / "blocked").c_str(), 1);
  const Json night{{"global_theme", "night"}};
  for (int count = 0; count < 4; ++count) tick(classicui, night, false);
  require(classicui.scans == 6 && classicui.writes == 4, "a failed theme write is not retried on every tick");
  require(engine.candidate_theme_retry_at_ <= std::chrono::steady_clock::now() + FcitxEngine::kCandidateThemeRetry,
          "a failed theme write schedules its retry");
  setenv("XDG_DATA_HOME", (root / "data").c_str(), 1);
  tick(classicui, night, false);
  require(classicui.scans == 6 && classicui.writes == 4, "the retry waits for its time");
  engine.candidate_theme_retry_at_ = std::chrono::steady_clock::now() - std::chrono::seconds(1);
  tick(classicui, night, false);
  require(classicui.scans == 7 && classicui.writes == 5 && classicui.config.theme.value() == host::kFcitxCandidateTheme,
          "a failed theme write is retried once its time comes, without any input changing");
  tick(classicui, Json{{"global_theme", "system"}}, false);
  require(classicui.scans == 8 && classicui.writes == 6, "the next change of the inputs writes the theme again");
#ifdef MSIME_FCITX5_NATIVE_CANVAS
  // 即使本轮写入了浅色主题，也不能用原生窗口盖住用户保留的第三方深色主题。
  FakeClassicUi partial;
  partial.config.darkTheme.setValue("synthetic-dark");
  require(fcitx::safeSaveAsIni(partial.config, "conf/classicui.conf"), "保存合成的深色主题");
  engine.candidate_theme_applied_.clear();
  engine.candidate_theme_attempt_.clear();
  tick(partial, ink, false);
  require(partial.config.theme.value() == host::kFcitxCandidateTheme &&
              partial.config.darkTheme.value() == "synthetic-dark" && !engine.native_canvas_owned_,
          "保留第三方深色主题时不接管原生画布");
  engine.applyCandidatePanelTheme(&partial, ink, false, Json(), true);
  require(engine.native_canvas_owned_, "主动选择水杉主题后可以接管原生画布");
  fcitx::RawConfig external;
  external.setValueByPath("Theme", "synthetic-light");
  partial.setConfig(external);
  tick(partial, ink, false);
  require(!engine.native_canvas_owned_, "第三方主题热更新立即退出原生画布所有权");
#endif
  // 先确认原生画布实际持有主题，再验证关闭开关会主动交还绘制。
  engine.applyCandidatePanelTheme(&classicui, ink, false, Json(), true);
#ifdef MSIME_FCITX5_NATIVE_CANVAS
  require(engine.native_canvas_owned_, "关闭生成前原生画布已经接管主题");
#endif
  // 关闭自动生成后，配色变化和主动选择均不覆盖人工主题、图片或经典界面的选择。
  const auto customImage = themeFile.parent_path() / "shape-customized.png";
  std::ofstream(themeFile) << "[Metadata]\nName=Synthetic customized theme\n";
  std::ofstream(customImage) << "synthetic image";
  const auto snapshot = [&] {
    std::map<std::string, std::tuple<std::string, ino_t, std::filesystem::file_time_type>> files;
    for (const auto &entry : std::filesystem::directory_iterator(themeFile.parent_path())) {
      if (!entry.is_regular_file()) continue;
      std::ifstream in(entry.path(), std::ios::binary);
      struct stat info {};
      require(::stat(entry.path().c_str(), &info) == 0, "读取合成主题文件状态");
      files.emplace(entry.path().filename().string(),
                    std::make_tuple(std::string(std::istreambuf_iterator<char>(in), {}), info.st_ino,
                                    std::filesystem::last_write_time(entry.path())));
    }
    return files;
  };
  const auto customized = snapshot();
  const auto customInode = inode();
  const auto customWritten = std::filesystem::last_write_time(themeFile);
  const int scans = classicui.scans, writes = classicui.writes;
  setenv("MSIME_FCITX5_THEME_AUTOGENERATE", "0", 1);
  tick(classicui, ink, true);
  engine.applyCandidatePanelTheme(&classicui, paper, false, Json(), true);
  require(snapshot() == customized && inode() == customInode &&
              std::filesystem::last_write_time(themeFile) == customWritten,
          "关闭生成后主题及图片的内容和时间戳不变，不删除用户图片");
  require(classicui.scans == scans && classicui.writes == writes, "关闭生成后不扫描或改写经典界面配置");
#ifdef MSIME_FCITX5_NATIVE_CANVAS
  require(!engine.native_canvas_owned_ && !engine.native_canvas_assets_, "关闭生成后由经典界面绘制人工主题");
#endif
  setenv("XDG_DATA_HOME", (root / "generation-disabled").c_str(), 1);
  engine.applyCandidatePanelTheme(&classicui, ink, true, Json(), true);
  require(!std::filesystem::exists(root / "generation-disabled"), "关闭生成后不创建缺失的主题目录");
  setenv("XDG_DATA_HOME", (root / "data").c_str(), 1);
  setenv("MSIME_FCITX5_THEME_AUTOGENERATE", "1", 1);
  engine.applyCandidatePanelTheme(&classicui, ink, true, Json(), true);
  require(snapshot() != customized, "重新允许自动生成后可按偏好恢复主题");
  if (savedAutogenerate) setenv("MSIME_FCITX5_THEME_AUTOGENERATE", savedAutogenerate->c_str(), 1);
  else unsetenv("MSIME_FCITX5_THEME_AUTOGENERATE");
  if (savedDataHome) setenv("XDG_DATA_HOME", savedDataHome->c_str(), 1);
  else unsetenv("XDG_DATA_HOME");
  if (savedStateHome) setenv("XDG_STATE_HOME", savedStateHome->c_str(), 1);
  else unsetenv("XDG_STATE_HOME");
  std::filesystem::remove(classicuiConf);
  engine.candidate_theme_applied_.clear();
  engine.candidate_theme_attempt_.clear();
  std::filesystem::remove_all(root);
}

void translationPreferenceChangesIncludeAccount() {
  const Json before = Json{{"translation_account", false}};
  auto after = before;
  after["translation_account"] = true;
  require(FcitxState::translationPreferencesChanged(before, after),
          "translation account changes invalidate translation requests");
  after = before;
  after["candidate_pronunciation"] = true;
  require(FcitxState::translationPreferencesChanged(before, after),
          "pronunciation changes invalidate translation requests");
  after = before;
  after["candidate_translation_scope"] = "all";
  require(FcitxState::translationPreferencesChanged(before, after),
          "改翻译范围要作废进行中的翻译请求和记下的回答");
}
// 「仅当前高亮」的高亮行（视图的 translation_reserved）：原生画布拿到要留的释义行数，classicui 在回答到之前用不上屏的全角空格行占住译文的位置；别的行、已有译文的行和不预留时都不加。
void reservedTranslationRow() {
  const auto word = [](Json candidate, int reserved) {
    candidate["id"] = Json{{"session", 1}, {"generation", 1}, {"index", 0}};
    return FcitxCandidate(nullptr, candidate, false, false, std::string(), 0, 0, reserved);
  };
  const auto shown = [](const FcitxCandidate &candidate) -> const fcitx::Text & {
    return static_cast<const fcitx::CandidateWord &>(candidate).text();
  };
  const auto waiting = word(Json{{"text", "你"}, {"translation_reserved", true}}, 1);
  require(waiting.display().reserved_gloss_lines == 1 && waiting.display().translation.empty(),
          "高亮行把要留的释义行数交给原生画布");
  require(shown(waiting).toString() == "你\n\u3000" && shown(waiting).splitByLine().size() == 2 &&
              shown(waiting).formatAt(shown(waiting).size() - 1) ==
                  fcitx::TextFormatFlags(fcitx::TextFormatFlag::DontCommit),
          "classicui 用一行不上屏的占位留住还没到的译文");
  require(waiting.text() == "你", "占位不进上屏文字");
  require(shown(word(Json{{"text", "你"}, {"translation_reserved", true}}, 2)).splitByLine().size() == 3,
          "两种目标语言留两行");
  const auto answered = word(Json{{"text", "你"}, {"translation", "you"}, {"translation_reserved", true}}, 1);
  require(shown(answered).toString() == "你\nyou" && answered.display().reserved_gloss_lines == 1,
          "译文到了就不再加占位");
  const auto other = word(Json{{"text", "拟"}}, 1);
  require(shown(other).toString() == "拟" && other.display().reserved_gloss_lines == 0,
          "非高亮行不留");
  const auto off = word(Json{{"text", "你"}, {"translation_reserved", true}}, 0);
  require(shown(off).toString() == "你" && off.display().reserved_gloss_lines == 0,
          "没有译文服务和离线释义时不留");
}

// 「显示读音」：读音在它那一行释义下面单独一行，classicui 和原生画布拿到同一份；没有释义时不画读音，选词的文字不带它。
void pronunciationRow() {
  const auto word = [](Json candidate, const std::string &pronunciation) {
    candidate["id"] = Json{{"session", 1}, {"generation", 1}, {"index", 0}};
    return FcitxCandidate(nullptr, candidate, false, false, std::string(), 0, 0, 0, pronunciation);
  };
  const auto shown = [](const FcitxCandidate &candidate) {
    return static_cast<const fcitx::CandidateWord &>(candidate).text().toString();
  };
  const auto read = word(Json{{"text", "爱"}, {"translation", "love; affection"}}, "/l\xCA\x8Cv/");
  require(shown(read) == "爱\nlove; affection\n/l\xCA\x8Cv/", "classicui 把读音画在释义下面单独一行");
  require(read.display().translation == "love; affection" && read.display().pronunciation == "/l\xCA\x8Cv/",
          "原生画布拿到不带读音的释义和单独的读音");
  require(read.text() == "爱", "读音不进上屏文字");
  const auto bare = word(Json{{"text", "爱"}}, "/l\xCA\x8Cv/");
  require(shown(bare) == "爱" && bare.display().pronunciation.empty(), "没有释义时不画读音");
  require(shown(word(Json{{"text", "爱"}, {"translation", "love"}}, std::string())) == "爱\nlove",
          "没有读音时释义照旧");
}

void emojiGroupResultFencing() {
  const Json current = Json{{"_generation", 7}, {"_category", "symbols"}};
  require(emojiGroupsResultCurrent(current, 7, "symbols"),
          "emoji groups accept a result for the active category");
  require(!emojiGroupsResultCurrent(current, 7, "kaomoji"),
          "emoji groups reject a result from a different category");
  require(!emojiGroupsResultCurrent(Json{{"_generation", 7}}, 7, "symbols"),
          "emoji groups reject results without a category fence");
}

// 通过产品的包 API、配色解析与经典界面接管路径验证 canvas；所有路径与图片均为合成值。
int candidateThemeCanvas() {
  namespace host = msime::linux_host;
  char temporary[] = "/tmp/msime-fcitx-theme-canvas-XXXXXX";
  const auto *directory = mkdtemp(temporary);
  require(directory != nullptr, "canvas fixture directory");
  const std::filesystem::path root(directory), skins = root / "skins", skin = skins / "canvas-fixture";
  for (const auto *name : {"XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR"})
    setenv(name, directory, 1);
  setenv("MSIME_FCITX5_OPTIONS", (root / "missing-options.json").c_str(), 1);
  std::filesystem::create_directories(skin);
  const auto paint = [&](const char *name, std::uint32_t color) {
    host::FcitxCanvas image(12, 12, 1);
    for (int y = 0; y < 12; ++y) for (int x = 0; x < 12; ++x) image.blend(x, y, color, 1);
    std::ofstream(skin / name, std::ios::binary) << host::fcitx_png_encode(image);
  };
  paint("panel.png", 0x102030);
  paint("light.png", 0x403020);
  std::ofstream(skin / "skin.toml") <<
      "schema_version = 2\nid = 'canvas-fixture'\nname = 'Canvas fixture'\nversion = '1'\nbase = 'canvas'\n"
      "[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['light', 'dark']\n"
      "[candidate_window]\ncontent_insets_dip = { top = 6, right = 6, bottom = 6, left = 6 }\nouter_insets_dip = { top = 2, right = 2, bottom = 2, left = 2 }\n"
      "[candidate_window.border_image]\nsource = 'panel.png'\nlight_source = 'light.png'\nslice_dip = [3]\nwidth_dip = [5]\n"
      "[candidate.light]\ntext = '#ABCDEF'\n[candidate.dark]\ntext = '#FEDCBA'\n";
  const Json entry{
      {"id", "canvas-fixture"}, {"title", "Canvas fixture"}, {"base", "system"},
      {"layouts", {"horizontal", "vertical"}},
      {"candidate", {
          {"light", {{"surface", "#FFFFFF"}, {"text", "#1A1A1A"}}},
          {"dark", {{"surface", "#202020"}, {"text", "#F2F2F2"}}}
      }}
  };
  const Json catalog{{"directory", skins.string()}, {"packages", Json::array({entry})}};
  Json preferences{{"global_theme", "custom"}, {"custom_theme", {{"base", "system"}, {"candidate_skin", "canvas-fixture"}}}};
  const auto theme_file = root / "fcitx5/themes/msime/theme.conf";
  const auto theme_text = [&] { std::ifstream in(theme_file); return std::string(std::istreambuf_iterator<char>(in), {}); };
  char program[] = "msime-canvas-test", disabled[] = "--disable=all", enabled[] = "--enable=classicui";
  char *args[] = {program, disabled, enabled, nullptr};
  {
    fcitx::Instance instance(3, args);
    instance.addonManager().registerDefaultLoader(nullptr);
    instance.initialize();
    if (!instance.addonManager().addonInfo("classicui")) {
      std::filesystem::remove_all(root);
      std::cout << "skipped: classicui runtime is not installed\n";
      return 77;
    }
    auto *classicui = instance.addonManager().addon("classicui", true);
    require(classicui && classicui->getConfig(), "真实 classicui 已加载");
    FcitxEngine engine(&instance);
    require(engine.applyCandidatePanelTheme(preferences, true, catalog, true), "首次套用需要刷新候选面板");
    const auto dark = theme_text();
    require(dark.find("Image=canvas-") != std::string::npos && dark.find("NormalColor=#fedcba\n") != std::string::npos,
            "真实宿主写入深色 canvas 贴图与共享贴图配色");
    require(dark.find("[InputPanel/ContentMargin]\nLeft=8\nRight=8\nTop=8\nBottom=8") != std::string::npos,
            "共享清单的内容留白与外留白进入经典面板");
    const auto written = std::filesystem::last_write_time(theme_file);
    require(!engine.applyCandidatePanelTheme(preferences, true, catalog, false), "同一资源不重复刷新候选面板");
    require(std::filesystem::last_write_time(theme_file) == written, "不变资源不重写主题");
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    const auto light = theme_text();
    require(light != dark && light.find("NormalColor=#abcdef\n") != std::string::npos, "明暗切换同时更新贴图与文字色");
    preferences["custom_theme"]["candidate_colors"]["text"] = "#112233";
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    require(theme_text().find("NormalColor=#112233\n") != std::string::npos, "共享取色器继续覆盖皮肤文字色");
    preferences["custom_theme"].erase("candidate_colors");
    paint("light.png", 0x665544);
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    require(theme_text() != light, "原地更新图片使缓存失效");
    std::ofstream(skin / "light.png", std::ios::binary) << "broken";
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    require(theme_text().find("Image=canvas-") == std::string::npos && theme_text().find("NormalColor=#1a1a1a\n") != std::string::npos,
            "包校验失败回到可读配色与原生卡片");
    paint("light.png", 0x665544);
    engine.candidate_canvas_retry_at_ = std::chrono::steady_clock::time_point{};
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    require(theme_text().find("Image=canvas-") != std::string::npos, "坏图修复后到期重新读取包");
    fcitx::RawConfig external;
    external.setValueByPath("Theme", "Nord-Dark");
    classicui->setConfig(external);
    paint("light.png", 0x223344);
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    fcitx::RawConfig current;
    classicui->getConfig()->save(current);
    require(current.valueByPath("Theme") && *current.valueByPath("Theme") == "Nord-Dark", "资源更新不夺回第三方主题");
  }
  removeFixtureTree(root);
  std::cout << "真实 classicui 的 canvas 资源、明暗、配色协商与所有权通过\n";
  return 0;
}

// 用真实 classicui 配置验证接管与恢复：缺省和切回「系统」保留水杉样式，只有主动选择才从第三方主题手里接管；无覆盖自定义主题与解析失败仍恢复持有项。每个 fcitx::Instance 就是一次进程启动（内存里的缓存与所有权状态从零开始），所有路径都指向合成目录；机器上只有 classicui 开发库而没有运行库时跳过（77）。
int candidateThemePriority() {
  char temporary[] = "/tmp/msime-fcitx-theme-priority-XXXXXX";
  const auto *directory = mkdtemp(temporary);
  require(directory != nullptr, "theme priority fixture directory");
  const std::filesystem::path root(directory);
  for (const auto *name : {"XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR"})
    setenv(name, directory, 1);
  setenv("MSIME_FCITX5_OPTIONS", (root / "missing-options.json").c_str(), 1);
  const auto conf = root / "fcitx5/conf/classicui.conf";
  const auto record = root / "msime-client/panel-restore.json";
  const auto theme_file = root / "fcitx5/themes/msime/theme.conf";
  // fcitx5-configtool 选了一个第三方主题，另有水杉写入的字体：恢复主题不能顺手动它。
  const auto choose_external = [&] {
    std::filesystem::create_directories(conf.parent_path());
    std::ofstream(conf) << "Theme=Nord-Dark\nDarkTheme=Nord-Dark\nFont=Noto Sans SC 18px\n";
  };
  choose_external();
  char program[] = "msime-theme-test";
  char disabled[] = "--disable=all";
  char enabled[] = "--enable=classicui";
  char *args[] = {program, disabled, enabled, nullptr};
  {
    fcitx::Instance instance(3, args);
    instance.addonManager().registerDefaultLoader(nullptr);
    instance.initialize();
    if (!instance.addonManager().addonInfo("classicui")) {
      std::filesystem::remove_all(root);
      std::cout << "skipped: classicui runtime is not installed\n";
      return 77;
    }
    auto *classicui = instance.addonManager().addon("classicui", true);
    require(classicui && classicui->getConfig(), "real classicui loaded");
    const auto option = [&](const char *key) {
      fcitx::RawConfig config;
      classicui->getConfig()->save(config);
      const auto *value = config.valueByPath(key);
      return value ? *value : std::string();
    };
    // 用户在 fcitx5-configtool 里换主题，写进运行中的 addon；`light_only` 只改 Theme（Fcitx5 的 DarkTheme 单独判断）。
    const auto pick_third_party = [&](bool light_only) {
      fcitx::RawConfig external;
      external.setValueByPath("Theme", "Nord-Dark");
      if (!light_only && !option("DarkTheme").empty()) external.setValueByPath("DarkTheme", "Nord-Dark");
      classicui->setConfig(external);
    };
    // Fcitx5 5.0.x 没有 DarkTheme；同一套测试仍验证 Theme，支持时再验证深色项。
    const bool has_dark_theme = !option("DarkTheme").empty();
    FcitxEngine engine(&instance);
    // 新装缺省仍使用水杉卡片，系统明暗只改变配色。
    fcitx::RawConfig stock;
    stock.setValueByPath("Theme", "default");
    stock.setValueByPath("DarkTheme", "default-dark");
    classicui->setConfig(stock);
    engine.applyCandidatePanelTheme(Json::object(), false, Json(), false);
    require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"),
            "新装缺省跟随系统保留水杉样式");
    const auto theme_text = [&] {
      std::ifstream in(theme_file);
      return std::string(std::istreambuf_iterator<char>(in), {});
    };
    require(theme_text().find("Color=#ffffff\n") != std::string::npos, "跟随系统的浅色候选底色");
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, true, Json(), false);
    require(theme_text().find("Color=#303030\n") != std::string::npos, "跟随系统的深色候选底色");
    // logo 按 show_app_logo 画：缺这个键的旧偏好文档照旧画，内容左边距让出 logo 和间距（16 + 16 + 6）；关掉后内容回到卡片内边距处，切换开关本身就让主题重画。
    require(theme_text().find("[InputPanel/ContentMargin]\nLeft=38\n") != std::string::npos,
            "旧偏好文档缺 show_app_logo 时照旧画 logo");
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}, {"show_app_logo", false}}, true, Json(), false);
    require(theme_text().find("[InputPanel/ContentMargin]\nLeft=16\n") != std::string::npos,
            "关掉 show_app_logo 后主题不画 logo");
    // 页码开关和整体大小也是主题输入：关掉页码不画 ‹ ›，150% 时内边距随之放大（12 + 6；阴影边距不缩放，Wayland 上候选窗离光标的偏移不变）。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}, {"show_app_logo", false},
                                         {"show_candidate_page_number", false}, {"candidate_scale_percent", 150}},
                                    true, Json(), false);
    require(theme_text().find("PrevPage") == std::string::npos &&
                theme_text().find("[InputPanel/ShadowMargin]\nLeft=12\nRight=12\nTop=8\nBottom=16\n") != std::string::npos &&
                theme_text().find("[InputPanel/ContentMargin]\nLeft=18\n") != std::string::npos,
            "页码开关和整体大小写进主题");
    require(option("Theme") == "msime", "系统明暗变化保留水杉样式");
    pick_third_party(false);
    // 启动与焦点同步只是重读同一份偏好：不碰用户已经选的第三方主题。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "paper"}}, false, Json(), false);
    require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "Nord-Dark"),
            "启动与焦点同步保留第三方主题");
    // 「系统」有默认样式，但后台同步也不接管第三方主题。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, false, Json(), false);
    require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "Nord-Dark"), "系统主题保留第三方主题");
    // 畸形或其他字段的偏好文档不抛异常，也不能因此接管第三方主题。
    for (const auto &document : {Json(nullptr), Json::array({Json("paper")}), Json(7),
                                 Json{{"global_theme", "retired-skin"}}, Json{{"global_theme", 7}}})
      engine.applyCandidatePanelTheme(document, false, Json(), false);
    require(option("Theme") == "Nord-Dark", "畸形偏好文档不接管第三方主题");
    // 用户在主题菜单里主动选择水杉内置主题：即使当前是第三方主题也接管，并先记下被替换的值。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "paper"}}, false, Json(), true);
    require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"), "主动选择水杉主题得以接管第三方主题");
    {
      std::ifstream in(record);
      const auto written = Json::parse(in).at("fcitx5");
      require(written.at("Theme").at("prior") == "Nord-Dark" &&
                  (!has_dark_theme || written.at("DarkTheme").at("prior") == "Nord-Dark"),
              "接管前记下要恢复的第三方主题");
    }
    // 主动切回「系统」只改为原生配色，仍画水杉样式。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, false, Json(), true);
    require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"),
            "主动切回系统保留水杉 Theme 与 DarkTheme");
    // 再次主动选择重新接管；这一次只把 Theme 换成第三方主题。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "paper"}}, false, Json(), true);
    require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"), "再次主动选择重新接管");
    pick_third_party(true);
    engine.applyCandidatePanelTheme(Json{{"global_theme", "paper"}}, false, Json(), false);
    require(option("Theme") == "Nord-Dark", "偏好同步不夺回第三方主题");
    engine.applyCandidatePanelTheme(Json{{"global_theme", "paper"}}, true, Json(), false);
    require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "msime"),
            "系统明暗变化不夺回第三方主题，DarkTheme 仍由水杉持有");
    // 系统偏好的后台同步不接管第三方项，也不退出仍属于水杉的深色项。
    const auto record_before = [&] { std::ifstream in(record); return Json::parse(in); };
    const auto before_sync = record_before();
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, false, Json(), false);
    require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "msime"),
            "系统后台同步保留第三方 Theme 与水杉 DarkTheme");
    require(option("Font") == "Noto Sans SC 18px", "系统后台同步不改变字体");
    require(record_before() == before_sync, "未接管的后台同步不重写恢复记录");
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, false, Json(), true);
    require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"),
            "主动选择系统可重新接管第三方主题");
    // 两项都换成第三方主题后，后台同步一项都不夺回。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "ink"}}, false, Json(), true);
    require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"), "主动选择再次接管");
    pick_third_party(false);
    engine.applyCandidatePanelTheme(Json{{"global_theme", "paper"}}, false, Json(), false);
    require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "Nord-Dark"),
            "两项都换成第三方主题后焦点与偏好同步都不夺回");
    // 系统偏好的后台同步：两项都已是第三方主题，一项也不动。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, false, Json(), false);
    require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "Nord-Dark"),
            "系统后台同步不覆盖用户自己改过的项");
    // 「系统」基底、没有皮肤也没有颜色槽位的自定义主题没有实际覆盖：即使是一次主动选择也退出接管。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "ink"}}, false, Json(), true);
    engine.applyCandidatePanelTheme(Json{{"global_theme", "custom"}, {"custom_theme", {{"base", "system"}}}}, false,
                                    Json(), true);
    require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "Nord-Dark"),
            "没有候选覆盖的自定义主题不接管");
    // 有效的外部皮肤（清单声明了当前布局与明暗）接管，颜色写进主题文件。
    const Json catalog{{"packages", Json::array({{{"id", "omarchy"},
                                                  {"title", "Omarchy"},
                                                  {"base", "system"},
                                                  {"layouts", Json::array({"horizontal", "vertical"})},
                                                  {"candidate", {{"light", {{"surface", "#123456"}}}}}}})}};
    engine.applyCandidatePanelTheme(
        Json{{"global_theme", "custom"}, {"custom_theme", {{"base", "system"}, {"candidate_skin", "omarchy"}}}},
        false, catalog, true);
    require(option("Theme") == "msime", "有效的外部皮肤接管");
    {
      std::ifstream in(theme_file);
      const std::string theme(std::istreambuf_iterator<char>(in), {});
      require(theme.find("Color=#123456") != std::string::npos, "皮肤颜色写进 classicui 主题");
    }
    // 皮肤不支持当前明暗时没有实际覆盖：后台同步退出接管。
    engine.applyCandidatePanelTheme(
        Json{{"global_theme", "custom"}, {"custom_theme", {{"base", "system"}, {"candidate_skin", "omarchy"}}}},
        true, catalog, false);
    require(option("Theme") == "Nord-Dark", "皮肤不支持当前明暗时不接管");
    // 两个真实会话轮流聚焦：旧窗口晚读到已处理的主题选择，不能把第三方主题再抢回来。
    {
      const auto state_directory = (root / "preferences").string();
      const auto initial = response(msime_client_load_preferences(
          reinterpret_cast<const uint8_t *>(state_directory.data()), state_directory.size()));
      Json options{{"api_version", 1}, {"preferences_directory", state_directory},
                   {"preferences", initial.at("preferences")}};
      for (const auto *key : {"resources", "user_data", "cache", "dictionaries"}) {
        const auto path = root / key;
        std::filesystem::create_directories(path);
        options[key] = path.string();
      }
      const auto options_file = root / "context-options.json";
      std::ofstream(options_file) << options.dump();
      setenv("MSIME_FCITX5_OPTIONS", options_file.c_str(), 1);
      FixtureContext old_window(instance.inputContextManager());
      old_window.focusIn();
      auto *old_state = old_window.propertyFor(&engine.factory_);
      require(old_state->ensure(), "旧窗口建立真实会话");
      old_window.focusOut();
      FixtureContext active_window(instance.inputContextManager());
      active_window.focusIn();
      auto *active_state = active_window.propertyFor(&engine.factory_);
      require(active_state->ensure(), "活动窗口建立真实会话");
      const auto save_theme = [&](const char *theme) {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(state_directory.data()), state_directory.size()));
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["global_theme"] = theme;
        const auto encoded = snapshot.dump();
        response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(state_directory.data()), state_directory.size(), revision,
            reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size()));
        return snapshot["preferences"];
      };
      const auto refresh = [&](FcitxState *state, const Json &expected) {
        // 不跑事件循环：只驱动真实异步偏好读取，避免计时器把测试的焦点顺序打乱。
        state->refreshPreferences();
        require(state->preferences_job_.valid(), "偏好读取已排队");
        state->preferences_job_.wait();
        state->refreshPreferences();
        require(state->preferences_ == expected, "上下文读到最新偏好");
      };
      pick_third_party(false);
      const auto paper = save_theme("paper");
      refresh(active_state, paper);
      require(option("Theme") == "msime", "设置页的主动主题选择接管第三方主题");
      // 存储里是 paper 时会话写出的主题文件，下面的新会话按字节比对，不在测试里另抄一份颜色。
      const auto paper_theme = theme_text();
      pick_third_party(false);
      active_window.focusOut();
      old_window.focusIn();
      require(old_state->ensure(), "切回旧窗口复用会话");
      refresh(old_state, paper);
      require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "Nord-Dark"),
              "旧窗口补读已处理的偏好不能夺回第三方主题");
      refresh(old_state, save_theme("ink"));
      require(option("Theme") == "msime", "旧窗口中的下一次主动主题选择仍能接管");
      const auto ink_theme = theme_text();
      require(ink_theme != paper_theme, "ink 与 paper 写出的主题文件不同");
      pick_third_party(false);
      require(old_state->setThemeChoice("paper"), "主题菜单主动选择成功");
      old_state->preferences_save_job_.wait();
      old_state->waitForPreferenceSave();
      require(option("Theme") == "msime", "菜单主动选择立即接管");
      pick_third_party(false);
      old_window.focusOut();
      active_window.focusIn();
      refresh(active_state, paper);
      require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "Nord-Dark"),
              "另一个窗口补读菜单保存的选择不能再次接管");
      require(active_state->setThemeChoice("system"), "主题菜单主动切回系统成功");
      active_state->preferences_save_job_.wait();
      active_state->waitForPreferenceSave();
      require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"),
              "菜单主动切回系统重新接管并保留水杉样式");
      pick_third_party(false);
      active_window.focusOut();
      old_window.focusIn();
      refresh(old_state, active_state->preferences_);
      require(option("Theme") == "Nord-Dark" && (!has_dark_theme || option("DarkTheme") == "Nord-Dark"),
              "旧窗口补读菜单的系统选择不夺回第三方主题");
      old_window.focusOut();
      // 设置页会同时更新存储与 runtime options，返回输入框时可能需要新建会话。
      old_state->close();
      active_state->close();
      for (const bool publish_options : {false, true}) {
        pick_third_party(false);
        options["preferences"] = save_theme(publish_options ? "paper" : "ink");
        if (publish_options) std::ofstream(options_file) << options.dump();
        FixtureContext settings_return_window(instance.inputContextManager());
        settings_return_window.focusIn();
        auto *settings_return_state = settings_return_window.propertyFor(&engine.factory_);
        require(settings_return_state->ensure(), "设置页保存后返回输入框建立会话");
        settings_return_state->refreshProviderSockets();
        require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"),
                "设置页的新选择不能在新会话建立基线时丢掉主动接管");
        // 只写了存储、runtime options 还是旧主题时，新会话画的也是存储里的主题，而不是文件里的。
        require(theme_text() == (publish_options ? paper_theme : ink_theme),
                "设置页保存后的新会话按存储里的主题写主题文件");
        pick_third_party(false);
        settings_return_state->refreshProviderSockets();
        require(option("Theme") == "Nord-Dark", "新会话处理过选择后也不能后台夺回第三方主题");
        settings_return_window.focusOut();
        settings_return_state->close();
      }
      // 托盘里的选择只写存储、不重写 runtime options（此时文件里仍是 paper）：切到别的程序新建会话后，皮肤、候选布局、候选窗明暗和云候选都不能回到文件里的旧值（#6540）。
      {
        FixtureContext menu_window(instance.inputContextManager());
        menu_window.focusIn();
        auto *menu_state = menu_window.propertyFor(&engine.factory_);
        require(menu_state->ensure(), "托盘选择的窗口建立会话");
        const auto wait_save = [&] {
          menu_state->preferences_save_job_.wait();
          menu_state->waitForPreferenceSave();
        };
        require(menu_state->setThemeChoice("ink"), "主题菜单选择 ink");
        wait_save();
        require(option("Theme") == "msime" && theme_text() == ink_theme, "菜单选择立即写出 ink 主题");
        menu_window.focusOut();
        FixtureContext next_window(instance.inputContextManager());
        next_window.focusIn();
        auto *next_state = next_window.propertyFor(&engine.factory_);
        require(next_state->ensure(), "切到另一个程序新建会话");
        next_state->refreshProviderSockets();
        require(next_state->preferences_.value("global_theme", std::string()) == "ink",
                "新会话的偏好是托盘选的 ink");
        require(next_state->currentThemeChoice() == "ink", "新会话的主题菜单勾选 ink");
        require(option("Theme") == "msime" && theme_text() == ink_theme,
                "新会话不把主题文件改回 runtime options 里的 paper");
        const auto layout = next_state->preferences_.value("candidate_layout", std::string("vertical"));
        const auto appearance = next_state->preferences_.value("candidate_theme", std::string("follow"));
        const bool cloud = next_state->preferences_.value("cloud_candidates", true);
        require(next_state->cycleCandidateLayout(), "托盘切换候选布局");
        require(next_state->cycleCandidateTheme(), "托盘切换候选窗明暗");
        require(next_state->toggleCloudCandidates(), "托盘切换云候选");
        // 宿主自己另存一份的几项（繁体、中文标点、成对标点、智能标点、以词定字、标点锁定）：建会话时若先按文件读好、再换成存储，它们留着文件里的值。标点锁定放最后，锁定后中文标点的切换不生效。
        const bool traditional = next_state->traditional_;
        const bool chinese_punctuation = next_state->chinese_punctuation_;
        const bool paired_punctuation = next_state->paired_punctuation_;
        const bool word_character = next_state->word_character_enabled_;
        const auto punctuation_lock = next_state->punctuation_lock_;
        const bool smart_punctuation = next_state->smart_punctuation_;
        require(next_state->toggleTraditional(), "托盘切换繁体");
        require(next_state->toggleChinesePunctuation(), "托盘切换中文标点");
        require(next_state->togglePairedPunctuation(), "托盘切换成对标点");
        require(next_state->toggleTopLevelBoolean("smart_punctuation", true), "托盘切换智能标点");
        require(next_state->toggleWordCharacter(), "托盘切换以词定字");
        require(next_state->cyclePunctuationLock(), "托盘切换标点锁定");
        next_state->preferences_save_job_.wait();
        next_state->waitForPreferenceSave();
        next_state->refreshProviderSockets();
        const auto toggled = next_state->preferences_;
        const auto toggled_theme = theme_text();
        require(toggled.value("candidate_layout", layout) != layout &&
                    toggled.value("candidate_theme", appearance) != appearance &&
                    toggled.value("cloud_candidates", cloud) != cloud,
                "托盘的三项选择在当前会话生效");
        require(next_state->traditional_ != traditional && next_state->chinese_punctuation_ != chinese_punctuation &&
                    next_state->paired_punctuation_ != paired_punctuation &&
                    next_state->smart_punctuation_ != smart_punctuation &&
                    next_state->word_character_enabled_ != word_character &&
                    next_state->punctuation_lock_ != punctuation_lock,
                "托盘的繁体、标点和以词定字选择在当前会话生效");
        next_window.focusOut();
        // 设置应用另存了一张屏幕键盘照片（合成的 1x1 PNG）：存储里有，新会话交给 Host API 的偏好里没有。
        {
          auto snapshot = response(msime_client_load_preferences(
              reinterpret_cast<const uint8_t *>(state_directory.data()), state_directory.size()));
          const auto revision = snapshot.at("revision").get<uint64_t>();
          snapshot["preferences"]["custom_theme"]["keyboard"]["photo"] =
              "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
          const auto encoded = snapshot.dump();
          response(msime_client_save_preferences(
              reinterpret_cast<const uint8_t *>(state_directory.data()), state_directory.size(), revision,
              reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size()));
        }
        FixtureContext third_window(instance.inputContextManager());
        third_window.focusIn();
        auto *third_state = third_window.propertyFor(&engine.factory_);
        require(third_state->ensure(), "再切到一个程序新建会话");
        third_state->refreshProviderSockets();
        for (const auto *key : {"global_theme", "candidate_layout", "candidate_theme", "cloud_candidates"})
          require(third_state->preferences_.value(key, Json()) == toggled.value(key, Json()),
                  "新会话保留托盘选的皮肤、候选布局、候选窗明暗和云候选");
        require(third_state->traditional_ == !traditional &&
                    engine.traditional_action_.isChecked(&third_window) == !traditional,
                "新会话的繁体输出和菜单勾选跟随托盘的选择");
        require(third_state->chinese_punctuation_ == !chinese_punctuation &&
                    third_state->session_chinese_punctuation_ == !chinese_punctuation &&
                    engine.chinese_punctuation_action_.isChecked(&third_window) == !chinese_punctuation,
                "新会话的中文标点、交给会话的标点和菜单勾选跟随托盘的选择");
        require(third_state->paired_punctuation_ == !paired_punctuation &&
                    engine.paired_punctuation_action_.isChecked(&third_window) == !paired_punctuation,
                "新会话的成对标点和菜单勾选跟随托盘的选择");
        require(third_state->smart_punctuation_ == !smart_punctuation &&
                    engine.smart_punctuation_action_.isChecked(&third_window) == !smart_punctuation,
                "新会话的智能标点和菜单勾选跟随托盘的选择");
        require(third_state->word_character_enabled_ == !word_character &&
                    third_state->punctuation_lock_ == next_state->punctuation_lock_,
                "新会话的以词定字和标点锁定跟随托盘的选择");
        require(theme_text() == toggled_theme, "新会话的主题文件与托盘选择后的一致");
        require(third_state->preferences_snapshot_.at("preferences").at("custom_theme").at("keyboard").contains("photo"),
                "存储快照保留键盘照片");
        require(!third_state->preferences_.at("custom_theme").at("keyboard").contains("photo") &&
                    !third_state->voice_host_options_.at("preferences").at("custom_theme").at("keyboard").contains("photo"),
                "建立会话的偏好不带键盘照片");
        third_window.focusOut();
        third_state->close();
        next_state->close();
        menu_state->close();
      }
      // 不带偏好存储的合法 runtime options 仍可建立会话，不能对 null 快照调用 value()。
      old_state->close();
      options.erase("preferences_directory");
      options["preferences"]["global_theme"] = "ink";
      pick_third_party(false);
      std::ofstream(options_file) << options.dump();
      old_window.focusIn();
      require(old_state->ensure(), "没有偏好存储时仍可建立会话");
      require(option("Theme") == "Nord-Dark", "没有存储快照的 runtime options 变化不能算主动选择");
      old_window.focusOut();
      setenv("MSIME_FCITX5_OPTIONS", (root / "missing-options.json").c_str(), 1);
    }
    // 旧版本留下水杉主题但恢复记录缺失或损坏：系统保留样式，无覆盖自定义仍安全恢复。
    for (const auto *contents : {"", "broken json"}) {
      fcitx::RawConfig held;
      held.setValueByPath("Theme", "msime");
      held.setValueByPath("DarkTheme", "msime");
      classicui->setConfig(held);
      std::filesystem::remove(record);
      if (*contents) std::ofstream(record) << contents;
      engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, false, Json(), false);
      require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"),
              "旧用户默认系统主题在恢复记录缺失或损坏时仍保留水杉样式");
      engine.applyCandidatePanelTheme(Json{{"global_theme", "custom"}, {"custom_theme", {{"base", "system"}}}},
                                      false, Json(), false);
      require(option("Theme") == "default" && (!has_dark_theme || option("DarkTheme") == "default-dark"),
              "无覆盖主题在恢复记录缺失或损坏时仍恢复自带主题");
      require(option("Font") == "Noto Sans SC 18px", "默认主题兜底不改变字体");
    }
  }
  // 进程重启：缓存清空，classicui 仍是用户选的第三方主题，启动同步不能接管；自带主题时仍正常接管。
  choose_external();
  {
    fcitx::Instance instance(3, args);
    instance.addonManager().registerDefaultLoader(nullptr);
    instance.initialize();
    auto *classicui = instance.addonManager().addon("classicui", true);
    require(classicui && classicui->getConfig(), "real classicui loaded after restart");
    const auto option = [&](const char *key) {
      fcitx::RawConfig config;
      classicui->getConfig()->save(config);
      const auto *value = config.valueByPath(key);
      return value ? *value : std::string();
    };
    const bool has_dark_theme = !option("DarkTheme").empty();
    require(option("Theme") == "Nord-Dark", "重启后读到的是用户的第三方主题");
    FcitxEngine engine(&instance);
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, false, Json(), false);
    require(option("Theme") == "Nord-Dark", "默认系统主题重启后不夺回第三方主题");
    fcitx::RawConfig stock;
    stock.setValueByPath("Theme", "default");
    stock.setValueByPath("DarkTheme", "default-dark");
    classicui->setConfig(stock);
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, false, Json(), false);
    require(option("Theme") == "msime" && (!has_dark_theme || option("DarkTheme") == "msime"), "默认系统主题在自带主题下重启后仍接管");
  }
  std::filesystem::remove_all(root);
  std::cout << "Fcitx5 candidate theme ownership passed\n";
  return 0;
}
#ifdef MSIME_FCITX5_HINT_FONT
// 模式提示的宽度：classicui 画水杉主题且主题带装饰图时，「中」「英」要补到装饰完整显示；
// 第三方主题（含由 DarkTheme 决定的活动主题）、没有装饰都保持原样。全部用合成目录与合成图。
int candidateThemeHint() {
  char temporary[] = "/tmp/msime-fcitx-hint-XXXXXX";
  const auto *directory = mkdtemp(temporary);
  require(directory != nullptr, "theme hint fixture directory");
  const std::filesystem::path root(directory);
  for (const auto *name : {"XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR"})
    setenv(name, directory, 1);
  setenv("MSIME_FCITX5_OPTIONS", (root / "missing-options.json").c_str(), 1);
  // PNG 文件头声明 200 x 12；缩放器解不开它，装饰按原图暂存，宽度就是 200。
  const std::string png_header("\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\xc8\0\0\0\x0c\x08\x06\0\0\0", 29);
  // 纯几何：提示宽度只由主题写的边距与装饰宽度决定，与字体无关。
  require(fcitx_draws_candidate_theme("msime", "Nord-Dark", false, true), "浅色画水杉 Theme");
  require(!fcitx_draws_candidate_theme("msime", "Nord-Dark", true, true), "深色画第三方 DarkTheme");
  require(fcitx_draws_candidate_theme("msime", "msime", true, true), "深色画水杉 DarkTheme");
  require(!fcitx_draws_candidate_theme("Nord-Dark", "msime", false, true), "第三方 Theme 不算水杉");
  require(fcitx_overlay_panel_width(32, "Top Center", 0, 13, 13) == 58, "居中装饰两边各留 clip 边距");
  require(fcitx_overlay_panel_width(32, "Top Left", 19, 13, 13) == 64, "靠边装饰量 OverlayOffsetX 与对面 clip 边距");
  require(fcitx_pad_hint_label("中", 58, 16, 16) == "中\u3000\u3000\u3000", "按全角空格一次补到目标宽度");
  require(fcitx_pad_hint_label("中", 16, 16, 16) == "中", "已经够宽就不再补");
  require(fcitx_pad_hint_label("中", 40, 16, 0) == "中", "量不出空格宽度就不补");
  // 读回主题：居中装饰的宽度与四边边距决定提示要占的宽度，没有装饰就是 0。
  const auto synthetic = root / "synthetic";
  std::filesystem::create_directories(synthetic);
  {
    std::ofstream out(synthetic / "decoration-synthetic.png", std::ios::binary);
    out << png_header;
  }
  std::ofstream(synthetic / "theme.conf")
      << "[InputPanel/Background]\nOverlay=decoration-synthetic.png\nGravity=Top Center\nOverlayOffsetX=0\n"
         "[InputPanel/Background/OverlayClipMargin]\nLeft=13\nRight=13\nTop=8\nBottom=17\n"
         "[InputPanel/ContentMargin]\nLeft=19\nRight=19\nTop=14\nBottom=23\n"
         "[InputPanel/TextMargin]\nLeft=10\nRight=12\nTop=6\nBottom=6\n";
  require(fcitx_hint_width_from_theme(synthetic / "theme.conf") == 200 + 2 * 13 - 19 - 19 - 10 - 12,
          "居中的装饰按两边 clip 边距算提示宽度");
  std::ofstream(synthetic / "plain.conf") << "[InputPanel/Background]\nColor=#ffffff\n";
  require(fcitx_hint_width_from_theme(synthetic / "plain.conf") == 0, "没有装饰就没有要预留的宽度");
  std::filesystem::create_directories(root / "fcitx5/conf");
  // 从第三方主题主动切换后，第一次提示就必须补宽，不依赖下一次焦点或偏好同步。
  std::ofstream(root / "fcitx5/conf/classicui.conf") << "Theme=Nord-Dark\nDarkTheme=Nord-Dark\nFont=Sans 10\n";
  const auto image = root / "skins/sakura/ears.png";
  std::filesystem::create_directories(image.parent_path());
  {
    std::ofstream out(image, std::ios::binary);
    out << png_header;
  }
  const Json catalog = {{"packages", Json::array({{{"id", "sakura"},
                                                  {"title", "樱花"},
                                                  {"base", "system"},
                                                  {"layouts", Json::array({"horizontal", "vertical"})},
                                                  {"candidate", {{"light", Json::object()}, {"dark", Json::object()}}},
                                                  {"decoration_top_dip", 24.5},
                                                  {"decoration_width_dip", 200},
                                                  {"decoration_image", image.string()}}})}};
  char program[] = "msime-theme-hint-test";
  char disabled[] = "--disable=all";
  char enabled[] = "--enable=classicui";
  char *args[] = {program, disabled, enabled, nullptr};
  {
    fcitx::Instance instance(3, args);
    instance.addonManager().registerDefaultLoader(nullptr);
    instance.initialize();
    if (!instance.addonManager().addonInfo("classicui")) {
      std::filesystem::remove_all(root);
      std::cout << "skipped: classicui runtime is not installed\n";
      return 77;
    }
    auto *classicui = instance.addonManager().addon("classicui", true);
    require(classicui && classicui->getConfig(), "real classicui loaded");
    fcitx::RawConfig theme_config;
    classicui->getConfig()->save(theme_config);
    const bool has_dark_theme = theme_config.valueByPath("DarkTheme") != nullptr;
    FcitxEngine engine(&instance);
    classicuiThemeTicks(engine);
    const Json preferences{{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "sakura"}}}};
    engine.applyCandidatePanelTheme(preferences, false, catalog, true);
    const auto theme_file = root / "fcitx5/themes/msime/theme.conf";
    require(std::filesystem::is_regular_file(theme_file), "装饰主题已写入");
    const int target = fcitx_hint_width_from_theme(theme_file);
    require(target > 0, "带装饰的主题给出提示宽度");
    const auto resolution = fcitx_default_font_resolution();
    const auto chinese = engine.modeHintLabel("x11::0", "中");
    require(chinese.size() > std::string("中").size(), "水杉主题的提示被补宽");
    require(fcitx_measure_text("Sans 10", resolution, chinese) >= target, "补宽后的提示容得下装饰");
    require(fcitx_measure_text("Sans 10", resolution, engine.modeHintLabel("x11::0", "英")) >= target, "英同样补宽");
    const auto english = engine.modeHintLabel("x11::0", "英");
    const int measured = hint_font_map_creations;
    for (int index = 0; index < 20; ++index) {
      require(engine.modeHintLabel("x11::0", "中") == chinese && engine.modeHintLabel("x11::0", "英") == english,
              "重复切换用同一份结果");
    }
    require(hint_font_map_creations == measured, "重复切换不再创建 Pango 字体映射");
    // 水杉偏好不变时，外部换主题也要刷新提示，但不能重新接管用户还原的自带主题。
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    fcitx::RawConfig changed;
    changed.setValueByPath("Theme", "Nord-Dark");
    classicui->setConfig(changed);
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    require(engine.modeHintLabel("x11::0", "中") == "中", "只换第三方 Theme 时立即停止补宽");
    for (const auto *font : {"Sans 14", "Sans 18"}) {
      fcitx::RawConfig changed_font;
      changed_font.setValueByPath("Font", font);
      classicui->setConfig(changed_font);
      engine.applyCandidatePanelTheme(preferences, false, catalog, false);
      require(engine.hint_inputs_.font == font, "第三方主题下连续修改字体也刷新提示缓存");
    }
    changed.setValueByPath("Theme", "default");
    classicui->setConfig(changed);
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    fcitx::RawConfig stock_current;
    classicui->getConfig()->save(stock_current);
    require(stock_current.valueByPath("Theme") && *stock_current.valueByPath("Theme") == "default",
            "已接管后外部还原自带主题不会被相同偏好抢回");
    require(engine.modeHintLabel("x11::0", "中") == "中", "外部还原自带主题时停止补宽");
    changed.setValueByPath("Theme", "msime");
    changed.setValueByPath("Font", "Sans 10");
    classicui->setConfig(changed);
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    require(engine.modeHintLabel("x11::0", "中") == chinese, "外部换回水杉主题时恢复缓存的补宽结果");
    changed.setValueByPath("UseDarkTheme", "True");
    changed.setValueByPath("DarkTheme", "msime");
    classicui->setConfig(changed);
    engine.applyCandidatePanelTheme(preferences, true, catalog, false);
    require(engine.modeHintLabel("x11::0", "中") == chinese, "深色画水杉时补宽");
    // 5.0.x 没有独立深色项，改实际绘制的 Theme；新版本只改 DarkTheme。
    changed.setValueByPath(has_dark_theme ? "DarkTheme" : "Theme", "Nord-Dark");
    classicui->setConfig(changed);
    engine.applyCandidatePanelTheme(preferences, true, catalog, false);
    require(engine.modeHintLabel("x11::0", "中") == "中", "换成第三方活动主题时立即停止补宽");
    changed.setValueByPath("Theme", "msime");
    changed.setValueByPath("UseDarkTheme", "False");
    changed.setValueByPath("DarkTheme", "msime");
    classicui->setConfig(changed);
    engine.applyCandidatePanelTheme(preferences, false, catalog, false);
    engine.applyCandidatePanelTheme(Json{{"global_theme", "system"}}, false, Json(), true);
    require(engine.hint_inputs_.active && engine.modeHintLabel("x11::0", "中") == "中",
            "跟随系统仍画水杉样式，但没有皮肤装饰就不补宽");
    engine.applyCandidatePanelTheme(preferences, false, catalog, true);
    require(engine.modeHintLabel("x11::0", "中") == chinese, "从系统配色选回皮肤后恢复提示宽度");
    // 无覆盖自定义主题退出后，再选回同一皮肤必须重新初始化，不能被旧 stamp 短路。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "custom"}, {"custom_theme", {{"base", "system"}}}},
                                    false, Json(), true);
    require(engine.modeHintLabel("x11::0", "中") == "中", "退出接管后不补宽");
    engine.applyCandidatePanelTheme(preferences, false, catalog, true);
    require(engine.modeHintLabel("x11::0", "中") == chinese, "选回同一皮肤后恢复提示宽度");
    // 热路径不碰文件：删掉主题与装饰图后，中/英切换仍用写主题时算好的宽度。
    std::filesystem::remove_all(root / "fcitx5/themes/msime");
    require(engine.modeHintLabel("x11::0", "中") == chinese, "提示不依赖主题文件");
    // 桌面切深色、DarkTheme 是第三方：classicui 画的是第三方主题，提示保持原样。
    fcitx::RawConfig external;
    external.setValueByPath("UseDarkTheme", "True");
    external.setValueByPath(has_dark_theme ? "DarkTheme" : "Theme", "Nord-Dark");
    classicui->setConfig(external);
    engine.applyCandidatePanelTheme(preferences, true, catalog, false);
    require(engine.modeHintLabel("x11::0", "中") == "中", "第三方深色主题不补宽");
    // 新版本关掉跟随系统后画回水杉 Theme；5.0.x 直接恢复 Theme，提示重新补宽。
    fcitx::RawConfig light;
    if (!has_dark_theme) light.setValueByPath("Theme", "msime");
    light.setValueByPath("UseDarkTheme", "False");
    classicui->setConfig(light);
    engine.applyCandidatePanelTheme(preferences, true, catalog, false);
    require(engine.modeHintLabel("x11::0", "中").size() > std::string("中").size(), "回到水杉主题后重新补宽");
    // 换成没有装饰的内置主题：没有要放的装饰，提示保持原样。
    engine.applyCandidatePanelTheme(Json{{"global_theme", "paper"}}, false, Json(), true);
    require(engine.modeHintLabel("x11::0", "中") == "中", "没有装饰就不补宽");
    // 第三方主题：即使水杉自己的主题文件还在，也不补宽。
    fcitx::RawConfig third;
    third.setValueByPath("Theme", "Nord-Dark");
    classicui->setConfig(third);
    engine.applyCandidatePanelTheme(Json{{"global_theme", "ink"}}, false, Json(), false);
    require(engine.modeHintLabel("x11::0", "中") == "中", "第三方主题不补宽");
  }
  std::filesystem::remove_all(root);
  std::cout << "Fcitx5 candidate theme hint passed\n";
  return 0;
}
#endif
// 不需要词库的真实上下文回归，普通 Linux 构建门禁也必须执行。
int cloudClipboardResultFencing() {
  char temporary[] = "/tmp/msime-fcitx-cloud-route-XXXXXX";
  const auto *directory = mkdtemp(temporary);
  require(directory != nullptr, "创建合成云剪贴板测试目录");
  for (const auto *name : {"XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR"})
    setenv(name, directory, 1);
  {
    char program[] = "msime-cloud-route-test";
    char disabled[] = "--disable=all";
    char *args[] = {program, disabled, nullptr};
    fcitx::Instance instance(2, args);
    instance.initialize();
    FixtureContext ic(instance.inputContextManager());
    ic.focusIn();
    FcitxState state(ic, nullptr, instance.eventLoop(), false);
    state.options_path_ = directory;
    state.updateCloudClipboardPreferences();
    {
      // 中继空间或偏好目录变化后，即使旧请求已经完成，也不能采用旧条目。
      for (const auto &relay : {
               Json{{"url", "https://relay.example.test/one"}, {"token", "synthetic-one"}},
               Json{{"url", "https://relay.example.test/one"}, {"token", "synthetic-two"}},
               Json{{"url", "https://relay.example.test/two"}, {"token", "synthetic-two"}}}) {
        const auto generationBefore = state.cloud_clipboard_generation_;
        state.cloud_clipboard_items_ = Json::array({Json{{"text", "旧空间合成条目"}}});
        std::promise<Json> completed;
        state.cloud_clipboard_job_ = completed.get_future().share();
        completed.set_value(Json{{"enabled", true}, {"items", state.cloud_clipboard_items_},
            {"_socket", state.cloud_clipboard_socket_}, {"_directory", state.options_path_},
            {"_generation", generationBefore}});
        state.preferences_["cloud_clipboard_relay"] = relay;
        state.refreshCloudClipboard();
        require(state.cloud_clipboard_generation_ > generationBefore &&
                    state.cloud_clipboard_items_.empty() && !state.cloud_clipboard_job_.valid(),
                "切换中继地址或令牌使旧空间列表和已完成请求失效");
      }
      const auto generationBefore = state.cloud_clipboard_generation_;
      state.cloud_clipboard_items_ = Json::array({Json{{"text", "旧目录合成条目"}}});
      state.options_path_ = (std::filesystem::path(directory) / "other-preferences").string();
      state.refreshCloudClipboard();
      require(state.cloud_clipboard_generation_ > generationBefore && state.cloud_clipboard_items_.empty(),
              "切换偏好目录使缓存列表失效");
    }
  }
  std::filesystem::remove_all(directory);
  std::cout << "Fcitx5 云剪贴板旧结果失效回归通过\n";
  return 0;
}

int candidateRuntimeFontUnits() {
  char pattern[] = "/tmp/msime-font-runtime-XXXXXX";
  const auto *directory = mkdtemp(pattern);
  require(directory != nullptr, "创建合成字号测试目录");
  const std::filesystem::path root(directory);
  for (const auto *name : {"XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR"})
    setenv(name, directory, 1);
  setenv("MSIME_FCITX5_OPTIONS", (root / "missing-options.json").c_str(), 1);
  char program[] = "msime-font-runtime-test";
  char disabled[] = "--disable=all";
  char *args[] = {program, disabled, nullptr};
  for (const auto &[version, expected] : std::array<std::pair<const char *, const char *>, 4>{{
           {"5.0.21", "Synthetic Sans, 20px"}, {"5.1.17", "Synthetic Sans, 20px"},
           {"5.1.18", "Synthetic Sans, 15"}, {"5.1.23", "Synthetic Sans, 15"}}}) {
    font_runtime_version = version;
    fcitx::Instance instance(2, args);
    instance.addonManager().registerDefaultLoader(nullptr);
    instance.initialize();
    FcitxEngine engine(&instance);
    const auto description = engine.candidate_font_sync_.next({"Synthetic Sans", {}, 20});
    require(description && *description == expected, "真实宿主按运行版本选择字号单位，不受编译版本限制");
  }
  font_runtime_version = nullptr;
  std::filesystem::remove_all(root);
  std::cout << "Fcitx5 runtime font units passed\n";
  return 0;
}

int main(int argc, char **argv) {
  try {
    if (argc == 2 && std::string(argv[1]) == "--font-runtime") return candidateRuntimeFontUnits();
    if (argc == 2 && std::string(argv[1]) == "--cloud-clipboard") return cloudClipboardResultFencing();
    if (argc == 2 && std::string(argv[1]) == "--theme-priority") return candidateThemePriority();
    if (argc == 2 && std::string(argv[1]) == "--theme-canvas") return candidateThemeCanvas();
#ifdef MSIME_FCITX5_HINT_FONT
    if (argc == 2 && std::string(argv[1]) == "--theme-hint") return candidateThemeHint();
#endif
    // 只跑不需要资源的候选行用例，门禁里没有已验证词库时也能跑到。
    if (argc == 2 && std::string(argv[1]) == "--rows") {
      autocorrectMarker();
      koreanHanjaGlossRow();
      scriptAnnotationRow();
      caretEditSuffixRow();
      translationPreferenceChangesIncludeAccount();
      reservedTranslationRow();
      pronunciationRow();
      const auto sentence = FcitxState::sentenceTranslationQuery(
          Json{{"generation", 17}, {"candidates", Json::array()},
               {"gloss_query", Json{{"generation", 17}, {"provider", "none"}, {"candidates", Json::array()}}}},
          "合成候选", "fr");
      require(sentence.at("sentence") == true && sentence.at("target_language") == "fr",
              "manual sentence translation keeps the requested mode and language");
      require(!sentence.contains("gloss_query"), "整句翻译不把整页离线释义的查询发给 provider");
      auto providerTexts = Json::array();
      for (const auto &candidate : sentence.at("candidates"))
        providerTexts.push_back(candidate.at("text"));
      require(providerTexts == Json::array({"合成候选"}),
              "manual sentence candidate reaches the translation provider as text");
      std::cout << "Fcitx5 candidate rows passed\n";
      return 0;
    }
    autocorrectMarker();
    koreanHanjaGlossRow();
    scriptAnnotationRow();
    candidateThemeDecoration();
    modeBadgeTheme();
    classicuiTakeoverRecord();
    translationPreferenceChangesIncludeAccount();
    emojiGroupResultFencing();
    require(argc == 2 || (argc == 3 && (std::string(argv[2]) == "--helpcode-trigger" ||
                                       std::string(argv[2]) == "--voice-hotkey" ||
                                       std::string(argv[2]) == "--ai" ||
                                       std::string(argv[2]) == "--ctrl-space" ||
                                       std::string(argv[2]) == "--modifier-focus" ||
                                       std::string(argv[2]) == "--mode-field-hints" ||
                                       std::string(argv[2]) == "--caps-lock" ||
                                       std::string(argv[2]) == "--app-rules" ||
                                       std::string(argv[2]) == "--japanese-toggle" ||
                                       std::string(argv[2]) == "--local-modes" ||
                                       std::string(argv[2]) == "--smart-abc" ||
                                       std::string(argv[2]) == "--punctuation-context")),
            "usage: fcitx5-native-test <verified-resources> [--voice-hotkey|--ai|--ctrl-space|--modifier-focus|--mode-field-hints|--caps-lock|--app-rules|--japanese-toggle|--local-modes|--smart-abc|--punctuation-context|--helpcode-trigger]");
    const bool ai = argc == 3 && std::string(argv[2]) == "--ai";
    const std::string suggestion = ai ? "合成候选" : "在线";
    char temporary[] = "/tmp/msime-fcitx5-test-XXXXXX";
    const auto *directory = mkdtemp(temporary);
    require(directory != nullptr, "fixture directory");
    // Fcitx5 的 StandardPath 在第一次使用时（下面创建 Instance 时）记下 XDG_CONFIG_HOME，之后不再读环境变量。先指到临时目录，conf/classicui.conf 之类的 Fcitx5 配置就只在这里读写，不碰运行测试的用户自己的配置。
    setenv("XDG_CONFIG_HOME", (std::filesystem::path(directory) / "config").c_str(), 1);
    const auto request = Json{{"resources", argv[1]}, {"state_root", directory}}.dump();
    auto options = response(msime_client_prepare_host(
        reinterpret_cast<const uint8_t *>(request.data()), request.size()));
    options["preferences"]["learning"] = false;
    if (argc == 3 && std::string(argv[2]) == "--helpcode-trigger") {
      options["helpcode_fallback_resources"] = MSIME_HELPCODE_FIXTURE_RESOURCES;
      options["preferences"]["shuangpin_profile"] = "microsoft";
      for (const auto *family : {"quanpin_helpcode", "shuangpin_helpcode"})
        options["preferences"][family] = {{"enabled", true}, {"schema", "xiaohe"},
            {"show_in_candidate_window", false}, {"trigger_key", std::string(family) == "quanpin_helpcode" ? "/" : ";"}};
    }
    options["preferences"]["candidate_page_size"] = 2;
    options["preferences"]["clipboard_history"] = true;
    // 存着关掉的空格锁定：宿主不再读它，下面检查锁定照旧打开。
    options["preferences"]["voice_input"]["hotkey_hold_space_lock"] = false;
    // A stored commit strategy, which the Linux settings page does not offer and the hosts ignore: the streaming preedit below must still appear.
    options["preferences"]["voice_input"]["commit_mode"] = "ctrl_v";
    const auto clipboardPath = std::filesystem::path(options.at("preferences_directory").get<std::string>()) /
                               "clipboard_history.json";
    std::ofstream(clipboardPath) << Json::array({"剪贴板合成测试", "第二条"}).dump();
    options["clipboard_history_path"] = clipboardPath.string();
    options["preferences"]["cloud_candidates"] = !ai && !(argc == 3 && std::string(argv[2]) == "--helpcode-trigger");
    options["preferences"]["ai_assistant"]["enabled"] = ai;
    options["preferences"]["ai_assistant"]["candidate_limit"] = 1;
    options["preferences"]["ai_assistant"]["endpoint"] = "https://synthetic.invalid/v1/chat/completions";
    options["preferences"]["ai_assistant"]["model"] = "synthetic";
    options["preferences"]["ai_assistant"]["token"] = "synthetic-token";
    options["candidate_skin_catalog"] = Json{{"packages", Json::array({
        Json{{"id", "solarized"}, {"title", "Solarized"}, {"base", "system"}, {"layouts", Json::array({"horizontal", "vertical"})}},
        Json{{"id", "unsafe/id"}, {"title", "Ignored"}, {"base", "system"}, {"layouts", Json::array({"vertical"})}},
        Json{{"id", "kite"}, {"title", "纸鸢"}, {"base", "paper"}, {"layouts", Json::array({"horizontal", "vertical"})}},
    })}};
    const auto socketPath = std::string(directory) + "/online.sock";
    const int providerServer = socket(AF_UNIX, SOCK_STREAM, 0);
    require(providerServer >= 0, "online provider socket");
    sockaddr_un providerAddress{};
    providerAddress.sun_family = AF_UNIX;
    require(socketPath.size() < sizeof(providerAddress.sun_path), "online socket path length");
    std::strncpy(providerAddress.sun_path, socketPath.c_str(), sizeof(providerAddress.sun_path) - 1);
    require(bind(providerServer, reinterpret_cast<sockaddr *>(&providerAddress), sizeof(providerAddress)) == 0,
            "online provider bind");
    require(listen(providerServer, 1) == 0, "online provider listen");
    options["online_provider_socket"] = socketPath;
    const auto cloudSocketPath = std::string(directory) + "/cloud-clipboard.sock";
    const int cloudServer = socket(AF_UNIX, SOCK_STREAM, 0);
    require(cloudServer >= 0, "cloud clipboard socket");
    sockaddr_un cloudAddress{};
    cloudAddress.sun_family = AF_UNIX;
    std::strncpy(cloudAddress.sun_path, cloudSocketPath.c_str(), sizeof(cloudAddress.sun_path) - 1);
    require(bind(cloudServer, reinterpret_cast<sockaddr *>(&cloudAddress), sizeof(cloudAddress)) == 0 &&
            listen(cloudServer, 1) == 0, "cloud clipboard listener");
    options["cloud_clipboard_provider_socket"] = cloudSocketPath;
    const auto voiceSocketPath = std::string(directory) + "/voice.sock";
    const int voiceServer = socket(AF_UNIX, SOCK_STREAM, 0);
    require(voiceServer >= 0, "voice socket");
    sockaddr_un voiceAddress{};
    voiceAddress.sun_family = AF_UNIX;
    std::strncpy(voiceAddress.sun_path, voiceSocketPath.c_str(), sizeof(voiceAddress.sun_path) - 1);
    require(bind(voiceServer, reinterpret_cast<sockaddr *>(&voiceAddress), sizeof(voiceAddress)) == 0 &&
            listen(voiceServer, 1) == 0, "voice listener");
    options["voice_provider_socket"] = voiceSocketPath;
    std::unique_ptr<VoiceProviderFixture> customVoiceProvider;
    if (argc == 3 && std::string(argv[2]) == "--voice-hotkey") {
      const auto customSocket = std::string(directory) + "/custom-voice.sock";
      customVoiceProvider = std::make_unique<VoiceProviderFixture>(customSocket);
      options["voice_provider_socket"] = customSocket;
      options["preferences"]["keybindings"]["toggle_voice_input"] = "Ctrl+Alt+KeyQ";
    }

    const auto path = std::string(directory) + "/runtime-options.json";
    // 设置页的做法：先写偏好存储，再把同一份偏好抄进 runtime options。新会话以存储为准，所以夹具要生效的偏好两边都写；`edit` 同时改存储和 `options`。
    const auto settingsPageSaves = [&](const auto &edit) {
      const auto store = options.at("preferences_directory").get<std::string>();
      auto snapshot = response(msime_client_load_preferences(
          reinterpret_cast<const uint8_t *>(store.data()), store.size()));
      const auto revision = snapshot.at("revision").get<uint64_t>();
      edit(snapshot["preferences"]);
      edit(options["preferences"]);
      const auto document = snapshot.dump();
      // 内容没变的保存保留原修订号，所以这里只要求保存成功（失败时 response 抛出）。
      response(msime_client_save_preferences(
          reinterpret_cast<const uint8_t *>(store.data()), store.size(), revision,
          reinterpret_cast<const uint8_t *>(document.data()), document.size()));
      std::ofstream(path) << options.dump();
    };
    const auto fixturePreferences = options.at("preferences");
    settingsPageSaves([&](Json &preferences) { preferences = fixturePreferences; });
    // Online and cloud clipboard requests arrive after earlier native checks; voice starts its listener immediately before its own key test below.
    constexpr int kProviderAcceptMs = 30000;
    std::thread provider([providerServer, ai, suggestion] {
      const auto reply = Json{{"candidates", Json::array({Json{{"text", suggestion}, {"source", ai ? 1 : 0}}})}}.dump() + "\n";
      // AI input sends a cache-only probe on every change before the real request (#594). The probe is answered with no candidates, as the provider does on a cache miss, and does not use up the one real request this fixture serves; the connection cap only bounds a runaway host.
      for (int served = 0, connections = 0; served < 1 && connections < 64; ++connections) {
        pollfd descriptor{providerServer, POLLIN, 0};
        if (poll(&descriptor, 1, kProviderAcceptMs) <= 0) break;
        const int client = accept(providerServer, nullptr, nullptr);
        if (client < 0) break;
        std::string request;
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
        while (request.size() < 16384 && request.find('\n') == std::string::npos &&
               std::chrono::steady_clock::now() < deadline) {
          pollfd input{client, POLLIN, 0};
          if (poll(&input, 1, 100) <= 0) continue;
          char chunk[1024];
          const auto count = read(client, chunk, sizeof(chunk));
          if (count <= 0) break;
          request.append(chunk, count);
        }
        bool cacheOnly = false;
        try {
          cacheOnly = Json::parse(request.substr(0, request.find('\n')))
                          .at("query").value("ai_cache_only", false);
        } catch (...) {}
        const auto answer = cacheOnly ? std::string("{\"candidates\":[]}\n") : reply;
        if (request.find('\n') == std::string::npos ||
            send(client, answer.data(), answer.size(), MSG_NOSIGNAL) != static_cast<ssize_t>(answer.size())) {
          close(client); break;
        }
        close(client);
        if (!cacheOnly) ++served;
      }
      close(providerServer);
    });
    struct ProviderJoiner { std::thread &thread; ~ProviderJoiner() { if (thread.joinable()) thread.join(); } } providerJoiner{provider};
    auto cloudProvider = std::async(std::launch::async, [cloudServer] {
      pollfd ready{cloudServer, POLLIN, 0};
      if (poll(&ready, 1, kProviderAcceptMs) <= 0) { close(cloudServer); return false; }
      const int client = accept(cloudServer, nullptr, nullptr);
      if (client < 0) { close(cloudServer); return false; }
      char request[4096]{};
      const auto count = read(client, request, sizeof(request) - 1);
      const auto reply = "{\"enabled\":true,\"items\":[{\"id\":\"synthetic-1\",\"text\":\"云剪贴板测试\",\"updated_at\":\"2026-01-01T00:00:00Z\"},{\"id\":\"synthetic-2\",\"text\":\"云剪贴板第二条\",\"updated_at\":\"2026-01-01T00:00:01Z\"}]}\n";
      const bool valid = count > 0 && std::string(request, count).find("cloud_clipboard") != std::string::npos;
      const bool sent = send(client, reply, std::strlen(reply), MSG_NOSIGNAL) == static_cast<ssize_t>(std::strlen(reply));
      close(client); close(cloudServer);
      return valid && sent;
    });
    setenv("MSIME_FCITX5_OPTIONS", path.c_str(), 1);
    char name[] = "fcitx5-native-test";
    char disable[] = "--disable=all";
    char *args[] = {name, disable, nullptr};
    fcitx::Instance instance(2, args);
    instance.initialize();
    FcitxEngine engine(&instance);
    {
      // First-run state: no override, an empty config home and no system file (this target's MSIME_SYSTEM_OPTIONS points into the build tree). The addon must show the setup hint instead of the generic error, pass keys through, open the guide only on activation and not again within its throttle, and log no refresh or event failure. MSIME_BINDIR also points into the build tree, where a stub stands in for the guide script.
      require(!std::filesystem::exists(MSIME_SYSTEM_OPTIONS), "first-run fixture needs an absent system options file");
      const auto *configHome = std::getenv("XDG_CONFIG_HOME");
      const std::optional<std::string> savedConfigHome = configHome ? std::optional<std::string>(configHome) : std::nullopt;
      const auto firstRunConfig = std::string(directory) + "/first-run-config";
      const auto firstRunDiagnostics = std::string(directory) + "/first-run-diagnostics";
      std::filesystem::create_directory(firstRunConfig);
      std::filesystem::create_directory(firstRunDiagnostics);
      unsetenv("MSIME_FCITX5_OPTIONS");
      setenv("XDG_CONFIG_HOME", firstRunConfig.c_str(), 1);
      const auto guideLog = std::string(directory) + "/first-run-guide.log";
      setenv("MSIME_TEST_FIRST_RUN_LOG", guideLog.c_str(), 1);
      // The context and AI runs share the build tree and may run in parallel; a rename replaces the stub without ever exposing a half-written file.
      std::filesystem::create_directories(MSIME_BINDIR);
      const auto guidePath = std::filesystem::path(MSIME_BINDIR) / std::string(msime::linux_host::kFirstRunGuideProgram);
      const auto stagedGuide = guidePath.string() + "." + std::to_string(getpid());
      std::ofstream(stagedGuide) << "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$MSIME_TEST_FIRST_RUN_LOG\"\n";
      require(chmod(stagedGuide.c_str(), 0755) == 0, "first-run guide stub permissions");
      std::filesystem::rename(stagedGuide, guidePath);
      const auto guideCalls = [&guideLog] {
        std::vector<std::string> lines;
        std::ifstream file(guideLog);
        for (std::string line; std::getline(file, line);) lines.push_back(line);
        return lines;
      };
      const auto diagnosticText = [&firstRunDiagnostics] {
        std::ifstream file(firstRunDiagnostics + "/diagnostic.log");
        return std::string(std::istreambuf_iterator<char>(file), std::istreambuf_iterator<char>());
      };
      msime_linux_diagnostic_configure(firstRunDiagnostics, true);
      FcitxEngine::refreshOptions();
      require(diagnosticText().find("dictionary_generation_refresh") == std::string::npos,
              "first-run state is not a refresh failure");
      {
        FixtureContext firstRun(instance.inputContextManager());
        firstRun.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit,
                                                           fcitx::CapabilityFlag::SurroundingText});
        firstRun.focusIn();
        fcitx::InputMethodEntry firstRunEntry("msime", "MSIME", "zh_CN", "msime");
        const auto hint = std::string(msime::linux_host::kFirstRunHint);
        fcitx::KeyEvent typed(&firstRun, fcitx::Key(FcitxKey_n));
        engine.keyEvent(firstRunEntry, typed);
        require(!typed.filtered() && !typed.accepted(), "first-run keys reach the application");
        require(firstRun.inputPanel().auxUp().toString() == hint, "a first-run key shows the setup hint");
        require(firstRun.propertyFor(&engine.factory_)->session_ == 0, "no session before first-run setup");
        std::this_thread::sleep_for(std::chrono::milliseconds(300));
        require(guideCalls().empty(), "a key never opens the first-run guide");
        fcitx::InputContextEvent firstFocus(&firstRun, fcitx::EventType::InputContextFocusIn);
        engine.activate(firstRunEntry, firstFocus);
        require(firstRun.inputPanel().auxUp().toString() == hint, "first-run activation shows the setup hint");
        const auto guideDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
        while (guideCalls().empty() && std::chrono::steady_clock::now() < guideDeadline)
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        require(guideCalls() == std::vector<std::string>{"--host fcitx5"},
                "first-run activation opens the guide once, naming the Fcitx5 host");
        fcitx::InputContextEvent secondFocus(&firstRun, fcitx::EventType::InputContextFocusIn);
        engine.activate(firstRunEntry, secondFocus);
        fcitx::KeyEvent typedAgain(&firstRun, fcitx::Key(FcitxKey_i));
        engine.keyEvent(firstRunEntry, typedAgain);
        require(!typedAgain.filtered(), "first-run keys keep reaching the application");
        std::this_thread::sleep_for(std::chrono::milliseconds(300));
        require(guideCalls().size() == 1, "repeated activation within the throttle does not respawn the guide");
        require(diagnosticText().find("operation_failed") == std::string::npos,
                "first-run state is not reported as an event failure");
      }
      // Positive control: the log above was live, so an actual failure does show up in it.
      setenv("MSIME_FCITX5_OPTIONS", "relative-runtime-options.json", 1);
      FcitxEngine::refreshOptions();
      require(diagnosticText().find("operation_failed operation=dictionary_generation_refresh") != std::string::npos,
              "diagnostic log records a real refresh failure");
      require(diagnosticText().find("reason=dictionary_outdated") == std::string::npos,
              "an invalid options path is not reported as outdated dictionaries");
      // The guide is spawned asynchronously, so a stray spawn would only reach the log later.
      std::this_thread::sleep_for(std::chrono::milliseconds(300));
      require(guideCalls().size() == 1, "an ordinary refresh failure does not open the guide");
      {
        // Downloaded dictionaries an upgrade did not replace: options in the prepared layout whose resource directory holds files the compiled lock does not pin. The refresh reports it by name and hands the notification to the guide script; the options file is not touched.
        const auto outdated = std::filesystem::path(directory) / "outdated";
        std::filesystem::create_directories(outdated / "resources");
        std::filesystem::create_directories(outdated / "state");
        std::ofstream(outdated / "resources/msime-pinyin.db") << "previous generation";
        const auto outdatedOptions = outdated / "state/runtime-options.json";
        const auto document = Json{{"api_version", 1},
                                   {"resources", (outdated / "resources").string()},
                                   {"user_data", (outdated / "state/user").string()},
                                   {"cache", (outdated / "state/cache").string()},
                                   {"dictionaries", (outdated / "state/user/dictionaries/previous").string()},
                                   {"preferences_directory", (outdated / "state").string()},
                                   {"preferences", Json::object()}}.dump(2);
        std::ofstream(outdatedOptions) << document;
        setenv("MSIME_FCITX5_OPTIONS", outdatedOptions.c_str(), 1);
        FcitxEngine::refreshOptions();
        require(diagnosticText().find("operation_failed operation=dictionary_generation_refresh reason=dictionary_outdated") !=
                    std::string::npos,
                "outdated dictionaries are named in the diagnostic log");
        const auto outdatedDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
        while (guideCalls().size() < 2 && std::chrono::steady_clock::now() < outdatedDeadline)
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        require(guideCalls().size() == 2 && guideCalls().back() == "--reason dictionary-outdated",
                "outdated dictionaries hand the notification to the guide script");
        std::this_thread::sleep_for(std::chrono::milliseconds(300));
        require(guideCalls().size() == 2, "outdated dictionaries open the guide exactly once");
        std::ifstream written(outdatedOptions);
        require(std::string(std::istreambuf_iterator<char>(written), std::istreambuf_iterator<char>()) == document,
                "outdated dictionaries leave the runtime options unchanged");
      }
      msime_linux_diagnostic_configure(std::string(), false);
      unsetenv("MSIME_TEST_FIRST_RUN_LOG");
      if (savedConfigHome) setenv("XDG_CONFIG_HOME", savedConfigHome->c_str(), 1);
      else unsetenv("XDG_CONFIG_HOME");
      setenv("MSIME_FCITX5_OPTIONS", path.c_str(), 1);
    }
    {
      // The desktop appearance is probed once for the addon, on a worker, and handed to every context on the loop; a context opened later starts in the last probed value. The fixture never runs the loop, so this drives the timer's step and the hand-off directly.
      FixtureContext before(instance.inputContextManager());
      const auto themeOf = [&engine](fcitx::InputContext &context) {
        return context.propertyFor(&engine.factory_)->system_dark_;
      };
      require(engine.stepSystemTheme() == 250000 && engine.system_theme_job_.valid(),
              "the first step starts the portal probe on a worker and polls it");
      require(engine.stepSystemTheme() == 250000 || !engine.system_theme_job_.valid(),
              "a step while the probe runs only polls it");
      if (engine.system_theme_job_.valid()) {
        engine.system_theme_job_.wait();
        require(engine.stepSystemTheme() == 5000000 && !engine.system_theme_job_.valid(),
                "a finished probe is taken on the loop and the next one waits the full interval");
      }
      require(themeOf(before) == engine.system_dark_, "the probed appearance reaches an open context");
      auto *beforeState = before.propertyFor(&engine.factory_);
      beforeState->preferences_["theme"] = "system";
      for (const bool dark : {!engine.system_dark_, engine.system_dark_}) {
        engine.applySystemTheme(dark);
        require(themeOf(before) == dark, "an appearance change reaches every open context");
        require(beforeState->wave_overlay_.light_theme == !dark,
                "an appearance change redraws a system-following voice overlay");
        FixtureContext after(instance.inputContextManager());
        require(themeOf(after) == dark, "a context opened after the probe starts in its value");
      }
    }
    FixtureContext ic(instance.inputContextManager());
    ic.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit,
                                               fcitx::CapabilityFlag::SurroundingText});
    ic.focusIn();
    fcitx::InputMethodEntry entry("msime", "MSIME", "zh_CN", "msime");
    fcitx::InputContextEvent focus(&ic, fcitx::EventType::InputContextFocusIn);
    engine.activate(entry, focus);
    auto *state = ic.propertyFor(&engine.factory_);
    // 测试里的引擎没有注册到 Fcitx5，失焦时 Fcitx5 随后对输入法调用的 deactivate 要自己补上。
    fcitx::InputContextEvent focusOut(&ic, fcitx::EventType::InputContextFocusOut);
    const auto blur = [&] {
      ic.focusOut();
      engine.deactivate(entry, focusOut);
    };
    const auto heldForTray = [&] { return engine.menu_context_ic_.get() == &ic; };
    if (state->session_ == 0) {
      // ensure() funnels every failure into unavailable(), which swallows the
      // reason. Ask it again here so the message names what went wrong instead
      // of leaving the whole fixture unexplained.
      std::string reason = "ensure() returned without a session";
      try {
        if (state->ensure()) reason = "session opened only on the second attempt";
      } catch (const std::exception &error) {
        reason = error.what();
      } catch (...) {
        reason = "non-standard exception";
      }
      require(state->session_ != 0, ("focus must unpack transition view: " + reason).c_str());
    }
    require(state->voice_hotkey_hold_space_lock_,
            "a stored false no longer turns off the hold-to-lock space lock");
    require(state->view_.contains("candidates"), "focus must unpack transition view");
    if (argc == 3 && std::string(argv[2]) == "--mode-field-hints") {
      const auto modifier = [&](fcitx::KeySym sym, bool release = false) {
        const auto held = (sym == FcitxKey_Shift_L || sym == FcitxKey_Shift_R)
            ? fcitx::KeyState::Shift : fcitx::KeyState::Ctrl;
        fcitx::KeyEvent event(&ic, fcitx::Key(sym, release ? fcitx::KeyStates(held)
                                                        : fcitx::KeyStates()), release);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const auto tap = [&](fcitx::KeySym sym) {
        modifier(sym);
        require(!modifier(sym, true), "模式切换的修饰键松开仍交还应用");
      };
      // 普通不检查拼写字段与地址栏都允许本地切换，学习及联网限制仍保留。
      for (const auto purpose : {fcitx::CapabilityFlag::NoFlag, fcitx::CapabilityFlag::Url}) {
        ic.setCapabilityFlags(fcitx::CapabilityFlags{
            fcitx::CapabilityFlag::Preedit, fcitx::CapabilityFlag::NoSpellCheck, purpose});
        require(state->ensure(), "不检查拼写的字段仍可建立输入会话");
        state->input_enabled_ = true;
        for (const auto shift : {FcitxKey_Shift_L, FcitxKey_Shift_R}) {
          tap(shift);
          require(!state->input_enabled_, "不检查拼写的字段应允许 Shift 切到英文");
          tap(shift);
          require(state->input_enabled_, "不检查拼写的字段应允许 Shift 切回中文");
        }
        state->mode_ctrl_enabled_ = true;
        tap(FcitxKey_Control_L);
        require(!state->input_enabled_, "开启单按 Ctrl 时不检查拼写的字段也可切换");
        tap(FcitxKey_Control_L);
        require(state->input_enabled_, "单按 Ctrl 可以切回中文");
        state->mode_ctrl_enabled_ = false;
        require(!state->preferences_.at("learning").get<bool>() &&
                !state->preferences_.at("cloud_candidates").get<bool>() &&
                !state->preferences_.at("ai_assistant").at("enabled").get<bool>(),
                "切换中英文不放开不检查拼写字段的学习和联网");
      }
      // NoSpellCheck 不得掩盖更严格的字段限制。
      for (const auto restriction : {fcitx::CapabilityFlag::Sensitive, fcitx::CapabilityFlag::Password,
                                     fcitx::CapabilityFlag::Disable, fcitx::CapabilityFlag::Digit,
                                     fcitx::CapabilityFlag::Number, fcitx::CapabilityFlag::Dialable}) {
        ic.setCapabilityFlags(fcitx::CapabilityFlags{
            fcitx::CapabilityFlag::Preedit, fcitx::CapabilityFlag::NoSpellCheck, restriction});
        state->ensure();
        const bool before = state->input_enabled_;
        tap(FcitxKey_Shift_L);
        require(state->input_enabled_ == before, "敏感或受限字段仍禁止 Shift 切换");
        require(!state->toggleInputMode(), "模式切换入口仍遵守敏感及受限字段限制");
      }
      ic.setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
      require(state->ensure(), "返回普通输入框后会话恢复");
      state->input_enabled_ = true;
      tap(FcitxKey_Shift_L);
      require(!state->input_enabled_, "返回普通输入框后 Shift 仍可切换");
      state->close();
      removeFixtureTree(directory);
      std::cout << "Fcitx5 不检查拼写字段的模式切换与隐私边界通过\n";
      return 0;
    }
    if (argc == 3 && std::string(argv[2]) == "--modifier-focus") {
      const auto shiftHeld = fcitx::KeyStates(fcitx::KeyState::Shift);
      const auto ctrlHeld = fcitx::KeyStates(fcitx::KeyState::Ctrl);
      const auto modifier = [&](fcitx::KeySym sym, bool release = false,
                                fcitx::KeyStates modifiers = fcitx::KeyStates()) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym, modifiers), release);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const auto refocus = [&] {
        blur();
        ic.focusIn();
        engine.activate(entry, focus);
      };
      const auto shiftTap = [&] {
        modifier(FcitxKey_Shift_L);
        require(!modifier(FcitxKey_Shift_L, true, shiftHeld), "Shift 松开仍交还应用");
      };
      require(state->input_enabled_, "修饰键焦点测试从中文开始");
      for (const auto ctrl : {FcitxKey_Control_L, FcitxKey_Control_R}) {
        modifier(ctrl);
        modifier(FcitxKey_Alt_L, false, ctrlHeld);
        // 松开留在其他窗口，原上下文只收到失焦和重新聚焦。
        refocus();
        shiftTap();
        require(!state->input_enabled_, "Ctrl 松开丢失后，重新聚焦的 Shift 应切到英文");
        shiftTap();
        require(state->input_enabled_, "恢复后 Shift 可以切回中文");
      }

      // 未完成的旧手势只结束，不把跨焦点的松开认成新的轻按。
      modifier(FcitxKey_Shift_L);
      refocus();
      modifier(FcitxKey_Shift_L, true, shiftHeld);
      require(state->input_enabled_, "失焦前按住的 Shift 松开不切换");
      shiftTap();
      require(!state->input_enabled_, "跨焦点旧松开之后新的 Shift 轻按仍有效");
      shiftTap();

      // 前端可能只报告松开；事件中的 Ctrl 仍按住时不能当作独立 Shift。
      modifier(FcitxKey_Shift_L, true, shiftHeld | ctrlHeld);
      require(state->input_enabled_, "只收到松开且 Ctrl 仍按住时不切换");
      modifier(FcitxKey_Shift_L, true, shiftHeld);
      require(!state->input_enabled_, "没有其他修饰键的仅松开前端仍可切换");
      modifier(FcitxKey_Shift_L, true, shiftHeld);
      require(state->input_enabled_, "仅松开前端可以切回中文");

      modifier(FcitxKey_Control_L);
      fcitx::InputContextEvent resetEvent(&ic, fcitx::EventType::InputContextReset);
      engine.reset(entry, resetEvent);
      shiftTap();
      require(!state->input_enabled_, "reset 结束旧 Ctrl 手势，后续 Shift 正常");
      shiftTap();

      modifier(FcitxKey_Control_L);
      fcitx::InputContextEvent switchEvent(&ic, fcitx::EventType::InputContextSwitchInputMethod);
      engine.deactivate(entry, switchEvent);
      engine.activate(entry, focus);
      shiftTap();
      require(!state->input_enabled_, "输入法停用后旧 Ctrl 状态不影响下一次启用");
      shiftTap();

      // 开启裸 Ctrl 时，跨焦点旧松开也不应成为轻按。
      modifier(FcitxKey_Control_L);
      refocus();
      state->mode_ctrl_enabled_ = true;
      modifier(FcitxKey_Control_L, true, ctrlHeld);
      require(state->input_enabled_, "失焦前按住的 Ctrl 松开不切换");
      modifier(FcitxKey_Control_L);
      modifier(FcitxKey_Control_L, true, ctrlHeld);
      require(!state->input_enabled_, "新的 Ctrl 轻按仍按开关正常切换");
      modifier(FcitxKey_Control_L);
      modifier(FcitxKey_Control_L, true, ctrlHeld);
      require(state->input_enabled_, "新的 Ctrl 轻按可以切回中文");
      modifier(FcitxKey_Shift_L);
      refocus();
      state->mode_ctrl_enabled_ = true;
      modifier(FcitxKey_Control_L);
      modifier(FcitxKey_Control_L, true, ctrlHeld);
      require(!state->input_enabled_, "Shift 松开丢失后，新的 Ctrl 轻按仍正常切换");
      modifier(FcitxKey_Control_L);
      modifier(FcitxKey_Control_L, true, ctrlHeld);
      state->mode_ctrl_enabled_ = false;

      state->close();
      removeFixtureTree(directory);
      std::cout << "Fcitx5 跨焦点修饰键手势与恢复通过\n";
      return 0;
    }
    if (customVoiceProvider) {
      // 原始 X 键码 24 对应物理 Q；布局给出 a 时仍须命中同一录键位置。
      const auto chord = [&](bool release = false, bool shift = false) {
        auto modifiers = fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt};
        if (shift) modifiers |= fcitx::KeyState::Shift;
        if (release) modifiers = fcitx::KeyStates{};
        fcitx::KeyEvent event(&ic, fcitx::Key(FcitxKey_a, modifiers, 24), release);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const auto waitVoice = [&](auto ready) {
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        while (!ready() && std::chrono::steady_clock::now() < deadline) {
          state->refreshVoice();
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        }
        return ready();
      };
      require(!chord(false, true), "自定义语音键接受了多余 Shift");
      require(chord(), "Fcitx5 未接收非美式布局的物理语音键");
      require(waitVoice([&] { return customVoiceProvider->started.load() == 1; }),
              "Fcitx5 自定义语音键未启动合成服务");
      require(chord() && customVoiceProvider->stop_requests.load() == 0,
              "Fcitx5 自动重复错误结束录音");
      require(chord(true), "Fcitx5 未消费修饰键先松开的语音键释放");
      require(chord() && chord(true), "Fcitx5 第二次语音键未被接收");
      require(waitVoice([&] { return customVoiceProvider->stop_requests.load() == 1; }),
              "Fcitx5 自定义语音键未结束合成服务");
      customVoiceProvider->release_final = true;
      require(waitVoice([&] { return ic.committed == "synthetic voice"; }),
              "Fcitx5 自定义语音结果未上屏");
      blur();
      customVoiceProvider.reset();
      close(voiceServer);
      removeFixtureTree(directory);
      std::cout << "Fcitx5 自定义语音物理键、修饰键、重复、松键与合成结果上屏通过\n";
      return 0;
    }
    if (argc == 3 && std::string(argv[2]) == "--helpcode-trigger") {
      const auto press = [&](fcitx::KeySym sym) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym));
        engine.keyEvent(entry, event);
        fcitx::KeyEvent release(&ic, fcitx::Key(sym), true);
        engine.keyEvent(entry, release);
        return event.filtered() && event.accepted();
      };
      for (const bool shuangpin : {false, true}) {
        if (shuangpin) engine.scheme_shuangpin_action_.activate(&ic);
        else engine.scheme_quanpin_action_.activate(&ic);
        const auto trigger = shuangpin ? FcitxKey_semicolon : FcitxKey_slash;
        require(press(FcitxKey_n) && press(FcitxKey_i) && press(trigger), "Fcitx5 未接收辅助码前导键");
        require(state->view_.value("editing_text", std::string{}) == "ni" && ic.committed.empty(),
                "前导键不应上屏或写入拼音");
        require(press(FcitxKey_f) && press(FcitxKey_r), "Fcitx5 未接收辅助码字母");
        const auto rows = state->view_.value("candidates", Json::array());
        require(state->view_.value("editing_text", std::string{}) == (shuangpin ? "nifR" : "niFR") &&
                    !rows.empty() && rows[0].value("text", std::string{}) == "拟",
                ("Fcitx5 前导键未触发双辅助码筛选：" + Json{{"editing_text", state->view_.value("editing_text", std::string{})},
                    {"candidates", rows}, {"committed", ic.committed},
                    {"scheme", state->view_.value("scheme", 99)},
                    {"helpcode", state->preferences_.value(shuangpin ? "shuangpin_helpcode" : "quanpin_helpcode", Json{})}}.dump()).c_str());
        require(press(FcitxKey_space) && ic.committed == "拟", "Fcitx5 辅助码候选未正确上屏");
        ic.committed.clear();
        require(press(FcitxKey_n) && press(FcitxKey_i) &&
                    state->view_.value("editing_text", std::string{}) == "ni", "选词后前导状态未结束");
        require(press(trigger) && press(FcitxKey_BackSpace) &&
                    state->view_.value("editing_text", std::string{}) == "ni", "退格不应删除拼音");
        press(FcitxKey_Escape);
        if (shuangpin) {
          require(press(FcitxKey_n) && press(FcitxKey_semicolon) &&
                      state->view_.value("editing_text", std::string{}) == "n;" && ic.committed.empty(),
                  "微软双拼半音节的分号韵母被前导键抢走");
          press(FcitxKey_Escape);
        }
      }
      state->close();
      removeFixtureTree(directory);
      std::cout << "Fcitx5 全拼、双拼辅助码前导键、筛选、上屏和退格通过\n";
      return 0;
    }
    if (argc == 3 && std::string(argv[2]) == "--punctuation-context") {
      engine.scheme_shuangpin_action_.activate(&ic);
      state->navigation_["comma_period"] = false;
      const auto press = [&](fcitx::KeySym sym) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym));
        engine.keyEvent(entry, event);
        fcitx::KeyEvent release(&ic, fcitx::Key(sym), true);
        engine.keyEvent(entry, release);
        return event.accepted();
      };
      const auto compose = [&] {
        for (const auto sym : {FcitxKey_n, FcitxKey_i, FcitxKey_h, FcitxKey_c})
          require(press(sym), "双拼按键应参与组字");
      };
      ic.surroundingText().setText("A", 1, 1);
      compose();
      require(press(FcitxKey_space) && ic.committed == "你好",
              "空格应上屏合成中文候选");
      require(press(FcitxKey_comma) && ic.committed == "你好，",
              "中文上屏后不能沿用提交前的 ASCII 前文");
      // 客户端随后发布的空文档仍是权威状态，不能被旧的 ASCII 影子盖过。
      state->commitText("A");
      require(!press(FcitxKey_comma), "尚未刷新文档时应保留刚提交的 ASCII 前文");
      ic.surroundingText().setText("", 0, 0);
      require(press(FcitxKey_comma) && ic.committed == "你好，A，",
              "提交后的新空文档不能回退到旧 ASCII 影子");
      // 同步前端也可能在提交回调内发布新文档，返回后不能再次把它作废。
      ic.on_commit = [&] { ic.surroundingText().setText("", 0, 0); };
      state->commitText("A");
      ic.on_commit = {};
      require(press(FcitxKey_comma) && ic.committed == "你好，A，A，",
              "提交回调内的新前文应保持权威");
      ic.committed.clear();
      compose();
      require(press(FcitxKey_comma) && ic.committed == "你好，",
              "关闭逗号翻页时组字中的逗号应直接上屏");
      state->navigation_["comma_period"] = true;
      compose();
      const auto beforePage = ic.committed;
      require(press(FcitxKey_comma) && ic.committed == beforePage && state->composingOrCandidates(),
              "启用逗号翻页时应保留既有导航行为");
      press(FcitxKey_Escape);
      state->close();
      removeFixtureTree(directory);
      std::cout << "Fcitx5 提交前后文时序与逗号导航回归通过\n";
      return 0;
    }
    // 应用例外：焦点从别的程序进入有规则的程序时从规则的模式开始；在这次停留里手动切换后让位，同一程序再次获得焦点仍是手动选的模式；换到别的程序再回来，规则重新生效。程序名是 Fcitx5 前端报告的 program()，夹具上下文自己给出。排在快捷模式那一段之前，免得那里的失败让它跑不到。
    if (argc == 3 && std::string(argv[2]) == "--app-rules") {
      const auto preferenceDirectory = options["preferences_directory"].get<std::string>();
      // 前面的状态栏开关在后台保存偏好；等它落盘再读修订号，免得这次保存撞上它。
      state->waitForPreferenceSave();
      {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["app_input_mode_rules"] = Json{{"Synthetic-Ruled", "english"}};
        snapshot["preferences"]["default_ime_mode"] = "chinese";
        snapshot["preferences"]["ime_mode_scope"] = "app";
        const auto document = snapshot.dump();
        response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
      }
      const auto focusContext = [&](FixtureContext &context) {
        context.focusIn();
        fcitx::InputContextEvent event(&context, fcitx::EventType::InputContextFocusIn);
        engine.activate(entry, event);
      };
      const auto blurContext = [&](FixtureContext &context) {
        context.focusOut();
        fcitx::InputContextEvent event(&context, fcitx::EventType::InputContextFocusOut);
        engine.deactivate(entry, event);
      };
      const bool unruledMode = state->input_enabled_;
      blur();
      FixtureContext ruled(instance.inputContextManager(), "synthetic-ruled");
      ruled.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit});
      auto *ruledState = ruled.propertyFor(&engine.factory_);
      focusContext(ruled);
      require(ruledState->session_ != 0 && !ruledState->input_enabled_,
              "a ruled program did not start in its rule's English mode");
      engine.input_mode_action_.activate(&ruled);
      require(ruledState->input_enabled_, "the mode action did not switch the ruled program to Chinese");
      blurContext(ruled);
      focusContext(ruled);
      require(ruledState->input_enabled_,
              "refocusing the same program put the rule back over the user's own switch");
      {
        FixtureContext sameProgram(instance.inputContextManager(), "synthetic-ruled");
        sameProgram.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit});
        blurContext(ruled);
        focusContext(sameProgram);
        require(sameProgram.propertyFor(&engine.factory_)->input_enabled_,
                "another window of the same program is not the same stay");
        blurContext(sameProgram);
      }
      focusContext(ic);
      require(state->input_enabled_ == unruledMode, "an unruled program took the ruled program's mode");
      blurContext(ic);
      focusContext(ruled);
      require(!ruledState->input_enabled_, "coming back from another program did not apply the rule again");
      // 没有程序名的上下文（不报告程序名的 Wayland text-input、XIM 前端）也是另一个客户端：经过它再回来，手动切换过的程序重新套用规则。
      engine.input_mode_action_.activate(&ruled);
      require(ruledState->input_enabled_, "the mode action did not switch the ruled program to Chinese again");
      blurContext(ruled);
      {
        FixtureContext anonymous(instance.inputContextManager(), "");
        anonymous.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit});
        focusContext(anonymous);
        blurContext(anonymous);
        anonymous.propertyFor(&engine.factory_)->close();
      }
      focusContext(ruled);
      require(!ruledState->input_enabled_, "a context without a program name did not end the stay");
      // 全局作用域：规则定下的模式不写进全局状态，从状态栏切到全局时、离开这个程序时都是；用户在这个程序里手动切换过才写进去。
      fcitx_global_input_mode = true;
      engine.mode_scope_action_.activate(&ruled);
      ruledState->waitForPreferenceSave();
      require(ruledState->preferences_.value("ime_mode_scope", std::string{}) == "global",
              "the mode scope action did not switch to global scope");
      require(fcitx_global_input_mode == std::optional<bool>(true),
              "switching to global scope wrote the rule's English mode into the global state");
      blurContext(ruled);
      require(fcitx_global_input_mode == std::optional<bool>(true),
              "leaving a ruled program wrote the rule's mode into the global state");
      focusContext(ruled);
      require(!ruledState->input_enabled_, "a ruled program did not start in its rule's mode under global scope");
      engine.input_mode_action_.activate(&ruled);
      engine.input_mode_action_.activate(&ruled);
      require(!ruledState->input_enabled_, "switching twice did not come back to English");
      blurContext(ruled);
      require(fcitx_global_input_mode == std::optional<bool>(false),
              "a manual switch in a ruled program did not reach the global state");
      // 不同焦点组之间 Fcitx5 不保证先失焦再聚焦（例如 XWayland 程序和原生 Wayland 程序）：别的程序先获得焦点、把停留换走，有规则的程序随后才失焦，用户在它里面的手动切换照样写进全局状态。
      focusContext(ruled);
      require(!ruledState->input_enabled_, "a ruled program did not start in its rule's mode under global scope again");
      engine.input_mode_action_.activate(&ruled);
      require(ruledState->input_enabled_, "the mode action did not switch the ruled program to Chinese under global scope");
      {
        FixtureContext otherProgram(instance.inputContextManager(), "synthetic-other");
        otherProgram.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit});
        focusContext(otherProgram);
        blurContext(ruled);
        require(fcitx_global_input_mode == std::optional<bool>(true),
                "a manual switch was lost when another program gained focus before the ruled one lost it");
        blurContext(otherProgram);
        otherProgram.propertyFor(&engine.factory_)->close();
      }
      ruledState->close();
      state->close();
      removeFixtureTree(directory);
      std::cout << "Fcitx5 app input mode rules: start mode, manual switch yield, re-entry, anonymous contexts and global scope passed\n";
      return 0;
    }
    // 标准双拼方案的独立验收也在完整上下文套件中运行，避免通用测试的其它失败掩盖新增入口。
    {
      const auto press = [&](fcitx::KeySym sym) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym));
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      engine.scheme_shuangpin_action_.activate(&ic);
      for (int attempt = 0; attempt < 5 &&
           state->view_.value("shuangpin_profile", std::string{}) != "smart_abc"; ++attempt)
        engine.shuangpin_profile_action_.activate(&ic);
      const auto profilePreferencesDirectory = options.at("preferences_directory").get<std::string>();
      const auto stored = response(msime_client_load_preferences(
          reinterpret_cast<const uint8_t *>(profilePreferencesDirectory.data()), profilePreferencesDirectory.size()));
      require(state->view_.value("shuangpin_profile", std::string{}) == "smart_abc" &&
                  stored.at("preferences").at("shuangpin_profile") == "smart_abc" &&
                  engine.shuangpin_profile_action_.shortText(&ic) == "双拼方案：智能 ABC",
              "智能 ABC 状态栏选择、标签或持久化失败");
      const auto committedBeforeAbc = ic.committed;
      for (const auto character : {FcitxKey_n, FcitxKey_i, FcitxKey_h, FcitxKey_k})
        require(press(character), "智能 ABC 字母未进入组合");
      require(press(FcitxKey_space) && ic.committed == committedBeforeAbc + "你好",
              "智能 ABC 状态栏选择后未按键位组词上屏");
      engine.shuangpin_profile_action_.activate(&ic);
      require(state->view_.value("shuangpin_profile", std::string{}) == "xiaohe",
              "智能 ABC 作为末项应轮换回小鹤");
      engine.scheme_quanpin_action_.activate(&ic);
      require(state->view_.value("scheme", 0u) == 0, "智能 ABC 夹具应恢复全拼");
      ic.committed.clear();
      if (argc == 3 && std::string(argv[2]) == "--smart-abc") {
        state->close();
        removeFixtureTree(directory);
        std::cout << "Fcitx5 智能 ABC 状态栏切换、保存与组词上屏通过\n";
        return 0;
      }
    }
    // A pre-preference-store runtime options document may omit newly added keys.
    // The shared preference contract treats candidate translations as enabled when absent;
    // the Fcitx host must preserve that default when the status action toggles it.
    state->preferences_.erase("candidate_translations");
    state->preferences_snapshot_["preferences"].erase("candidate_translations");
    require(engine.candidate_translation_action_.isChecked(&ic),
            "missing candidate translation preference defaults to enabled");
    require(state->toggleCandidateTranslations(),
            "candidate translation toggle accepts a missing preference");
    require(!state->preferences_.value("candidate_translations", true),
            "missing candidate translation preference toggles off");
    require(state->toggleCandidateTranslations(),
            "candidate translation toggle restores a missing preference");
    require(state->preferences_.value("candidate_translations", false),
            "candidate translation toggle restores enabled mode");
    // 真实 KeyEvent 会把 Shift+字母规范化成大写并移除 Shift 位；快捷模式必须仍能进入，
    // 不能把普通大写或 CapsLock 当快捷键，也不能在组合键末尾松开 Shift 时切到英文。
    {
      const auto press = [&](fcitx::KeySym sym, fcitx::KeyStates states = fcitx::KeyStates(), bool release = false) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym, states), release);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const fcitx::KeyStates shiftState{fcitx::KeyState::Shift};
      const auto before = ic.committed;
      for (const auto &[sym, mode] : std::array<std::pair<fcitx::KeySym, const char *>, 8>{{
               {FcitxKey_T, "date_time"}, {FcitxKey_U, "unicode"},
               {FcitxKey_K, "quick_phrase"}, {FcitxKey_E, "emoji"},
               {FcitxKey_M, "kaomoji"}, {FcitxKey_J, "super_jianpin"},
               {FcitxKey_Y, "temporary_english"}, {FcitxKey_R, "temporary_japanese"}}}) {
        require(!press(sym) && !press(sym, fcitx::KeyState::CapsLock),
                "Uppercase without Shift must pass through");
        require(state->view_.value("local_mode", std::string("none")) == "none",
                "Uppercase without Shift must not enter a local mode");
        // #7616 连续大写期间不再进入快捷模式；用小写输入结束这一串，取消合成组字后再测入口。
        require(!press(sym, shiftState), "连续大写中的 Shift 字母应留给编辑器");
        require(press(FcitxKey_n) && press(FcitxKey_Escape), "快捷模式夹具应先结束连续大写并清空合成组字");
        press(FcitxKey_Shift_L, shiftState);
        require(press(sym, shiftState), (std::string("Shift 快捷模式未处理：") + mode).c_str());
        require(state->view_.value("local_mode", std::string("none")) == mode,
                "Shift local-mode shortcut must enter its mode");
        press(sym, shiftState, true);
        press(FcitxKey_Shift_L, fcitx::KeyStates(), true);
        require(state->input_enabled_, "Shift chord release must not switch to English");
        require(press(FcitxKey_Escape) &&
                    state->view_.value("local_mode", std::string("none")) == "none",
                "Escape must leave the local mode");
      }
      const auto disabledEntryComposes = [&](fcitx::KeySym sym, const char *letter) {
        require(press(sym, shiftState) &&
                    state->view_.value("local_mode", std::string("none")) == "none" &&
                    state->view_.value("editing_text", std::string()) == letter,
                "关闭的快捷入口应保留大写普通组字，不进入本地模式");
        press(sym, shiftState, true);
        press(FcitxKey_Shift_L, fcitx::KeyStates(), true);
        require(state->input_enabled_ && press(FcitxKey_Escape) &&
                    state->view_.value("editing_text", std::string()).empty() && ic.committed == before,
                "关闭入口的组字可取消，松开组合键不切英文也不上屏");
      };
      disabledEntryComposes(FcitxKey_V, "V");
      // 拆字反查在桌面默认关：Shift+C 留在普通组字；从菜单打开后进入拆字，撇号作为部件分隔符。
      disabledEntryComposes(FcitxKey_C, "C");
      require(state->toggleLocalMode("chaizi"), "The chaizi switch did not turn on");
      // 菜单开关会在后台把选择写回偏好文档；等它落盘，免得后面的偏好刷新撞上还没完成的保存。
      state->preferences_save_job_.wait();
      state->waitForPreferenceSave();
      press(FcitxKey_Shift_L, shiftState);
      require(press(FcitxKey_C, shiftState) &&
                  state->view_.value("local_mode", std::string("none")) == "chaizi",
              "Shift+C must enter the chaizi mode once it is on");
      press(FcitxKey_C, shiftState, true);
      press(FcitxKey_Shift_L, fcitx::KeyStates(), true);
      for (const auto sym : {FcitxKey_h, FcitxKey_u, FcitxKey_o, FcitxKey_apostrophe, FcitxKey_h, FcitxKey_u,
                             FcitxKey_o, FcitxKey_h, FcitxKey_u, FcitxKey_o})
        require(press(sym), "The chaizi mode must take component letters and the separator");
      require(state->view_.value("editing_text", std::string()) == "Chuo'huohuo" &&
                  !state->view_.value("candidates", Json::array()).empty() &&
                  state->view_["candidates"][0].value("text", std::string()) == "焱",
              "The chaizi mode must find 焱 from huo'huohuo");
      require(press(FcitxKey_Escape) &&
                  state->view_.value("local_mode", std::string("none")) == "none",
              "Escape must leave the chaizi mode");
      require(state->toggleLocalMode("chaizi"), "拆字开关应能再次关闭");
      disabledEntryComposes(FcitxKey_C, "C");
      state->preferences_save_job_.wait();
      state->waitForPreferenceSave();
      require(ic.committed == before, "Mode shortcuts must not commit uppercase letters");
      if (argc == 3 && std::string(argv[2]) == "--local-modes") {
        state->close();
        removeFixtureTree(directory);
        std::cout << "Fcitx5 Shift local modes, uppercase passthrough and modifier release passed\n";
        return 0;
      }
    }
    auto changedPreferences = options["preferences"];
    changedPreferences["number_row_selection"] = false;
    changedPreferences["candidate_layout"] = "horizontal";
    changedPreferences["smart_punctuation"] = true;
    changedPreferences["smart_punctuation_repeat"] = true;
    changedPreferences["learning"] = true;
    changedPreferences["candidate_translations"] = true;
    const auto preferenceDirectory = options["preferences_directory"].get<std::string>();
    if (argc == 3 && std::string(argv[2]) == "--ctrl-space") {
      const auto press = [&](bool release = false, bool ctrl = true) {
        fcitx::KeyEvent event(&ic, fcitx::Key(FcitxKey_space,
            ctrl ? fcitx::KeyStates(fcitx::KeyState::Ctrl) : fcitx::KeyStates()), release);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const auto chord = [&] {
        const bool accepted = press();
        require(press(true) == accepted, "Ctrl+Space press and release have different ownership");
        return accepted;
      };
      require(chord() && !state->input_enabled_ && chord() && state->input_enabled_,
              "Default Ctrl+Space does not switch in both directions");
      const auto set_binding = [&](bool enabled) {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["keybindings"]["switch_language_ctrl_space"] = enabled;
        const auto document = snapshot.dump();
        response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        do {
          state->refreshPreferences();
          if (state->preferences_.at("keybindings").value("switch_language_ctrl_space", true) == enabled)
            return;
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        } while (std::chrono::steady_clock::now() < deadline);
        require(false, "Ctrl+Space preference did not hot-reload");
      };
      set_binding(false);
      for (bool chinese : {true, false}) {
        if (state->input_enabled_ != chinese) engine.input_mode_action_.activate(&ic);
        for (int repeat = 0; repeat < 3; ++repeat)
          require(!press() && state->input_enabled_ == chinese,
                  "Disabled Ctrl+Space press was intercepted or switched mode");
        require(!press(true), "Disabled Ctrl+Space release was intercepted");
      }
      engine.input_mode_action_.activate(&ic);
      fcitx::KeyEvent letter(&ic, fcitx::Key(FcitxKey_n));
      engine.keyEvent(entry, letter);
      const auto preedit = ic.inputPanel().clientPreedit().toString();
      const auto committed = ic.committed;
      require(letter.accepted() && !preedit.empty() && !chord() &&
                  ic.inputPanel().clientPreedit().toString() == preedit && ic.committed == committed,
              "Disabled Ctrl+Space changed the active composition");
      fcitx::KeyEvent cancel(&ic, fcitx::Key(FcitxKey_Escape));
      engine.keyEvent(entry, cancel);
      set_binding(true);
      for (int repeat = 0; repeat < 3; ++repeat)
        require(press() && !state->input_enabled_, "Held Ctrl+Space toggled more than once");
      require(press(true, false) && !state->input_enabled_,
              "Consumed Ctrl+Space release escaped after Ctrl was released");
      require(chord() && state->input_enabled_, "Ctrl+Space did not restore Chinese mode");
      state->close();
      removeFixtureTree(directory);
      std::cout << "Fcitx5 Ctrl+Space defaults, passthrough, hot-reload and repeat passed\n";
      return 0;
    }
    // 单按 Caps Lock 切换中英文，从真实的 keyEvent 驱动。撤回大写锁定换成测试的实现：数撤回了几次，也能让它失败，不碰 X 服务器。按下事件带按下前的锁定状态，松开事件带按下后的。
    if (argc == 3 && std::string(argv[2]) == "--caps-lock") {
      static int releases = 0;
      static bool releaseWorks = true;
      msime::linux_host::replace_caps_lock_release_for_testing(+[] { return true; }, +[] {
        ++releases;
        return releaseWorks;
      });
      const fcitx::KeyStates lock{fcitx::KeyState::CapsLock};
      const auto capsKey = [&](bool release, fcitx::KeyStates states) {
        fcitx::KeyEvent event(&ic, fcitx::Key(FcitxKey_Caps_Lock, states), release);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      // 大写锁定从关到开的一次按下和松开；松开本身永远不吃掉，和裸 Shift 一样。
      const auto tap = [&](fcitx::KeyStates extra = fcitx::KeyStates()) {
        require(!capsKey(false, extra), "Caps Lock press was consumed");
        require(!capsKey(true, extra | lock), "Caps Lock release was consumed");
      };
      require(state->input_enabled_, "Caps Lock test does not start in Chinese");
      // 缺省关：轻点只是大写锁定，不撤回也不切换。
      tap();
      require(state->input_enabled_ && releases == 0, "Caps Lock switched with the preference off");
      const auto setBinding = [&](bool enabled) {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["keybindings"]["switch_language_caps_lock"] = enabled;
        const auto document = snapshot.dump();
        response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        do {
          state->refreshPreferences();
          if (state->mode_caps_lock_enabled_ == enabled) return;
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        } while (std::chrono::steady_clock::now() < deadline);
        require(false, "Caps Lock preference did not hot-reload");
      };
      setBinding(true);
      tap();
      require(!state->input_enabled_ && releases == 1, "a Caps Lock tap did not release the lock and switch to English");
      // 英文透传时同样处理，切回中文。
      tap();
      require(state->input_enabled_ && releases == 2, "a Caps Lock tap did not switch back from English passthrough");
      // 按住超过 300 毫秒是要大写锁定。
      require(!capsKey(false, fcitx::KeyStates()), "held Caps Lock press was consumed");
      std::this_thread::sleep_for(std::chrono::milliseconds(350));
      require(!capsKey(true, lock), "held Caps Lock release was consumed");
      require(state->input_enabled_ && releases == 2, "a long Caps Lock press switched or released the lock");
      // 大写锁定开着时按 Caps Lock 是关掉它。
      require(!capsKey(false, lock) && !capsKey(true, fcitx::KeyStates()), "Caps Lock that turns the lock off was consumed");
      require(state->input_enabled_ && releases == 2, "turning Caps Lock off switched");
      // 按着 Caps Lock 打字是在用大写锁定。
      require(!capsKey(false, fcitx::KeyStates()), "Caps Lock press before a letter was consumed");
      {
        fcitx::KeyEvent letter(&ic, fcitx::Key(FcitxKey_A, lock));
        engine.keyEvent(entry, letter);
        require(!letter.accepted(), "a Caps Lock letter was composed");
      }
      require(!capsKey(true, lock), "Caps Lock release after a letter was consumed");
      require(state->input_enabled_ && releases == 2, "typing while Caps Lock was held switched");
      // 和修饰键一起按是组合键。
      tap(fcitx::KeyStates(fcitx::KeyState::Shift));
      require(state->input_enabled_ && releases == 2, "Shift+Caps Lock switched");
      // 撤不回大写锁定就不切换。
      releaseWorks = false;
      tap();
      require(state->input_enabled_ && releases == 3, "a Caps Lock tap switched although the lock stayed on");
      releaseWorks = true;
      // 组字中先上屏原文再切换，和裸 Shift 一样。
      const auto committed = ic.committed;
      for (const auto sym : {FcitxKey_n, FcitxKey_i}) {
        fcitx::KeyEvent letter(&ic, fcitx::Key(sym));
        engine.keyEvent(entry, letter);
        require(letter.accepted(), "Caps Lock composition test did not compose");
      }
      tap();
      require(!state->input_enabled_ && ic.committed == committed + "ni" && releases == 4,
              "a Caps Lock tap did not commit the reading before switching");
      tap();
      require(state->input_enabled_, "a Caps Lock tap did not switch back after the commit");
      // 组字中长按 Caps Lock 是要大写锁定：fcitx 不把 Caps_Lock 算作修饰键，布防了的按下也不能把首选上屏。
      for (const auto sym : {FcitxKey_n, FcitxKey_i}) {
        fcitx::KeyEvent letter(&ic, fcitx::Key(sym));
        engine.keyEvent(entry, letter);
      }
      require(!capsKey(false, fcitx::KeyStates()), "Caps Lock press during a composition was consumed");
      std::this_thread::sleep_for(std::chrono::milliseconds(350));
      require(!capsKey(true, lock), "Caps Lock release during a composition was consumed");
      require(state->input_enabled_ && ic.committed == committed + "ni" &&
                  ic.inputPanel().clientPreedit().toString() == "ni",
              "a long Caps Lock press during a composition committed or switched");
      {
        fcitx::KeyEvent cancel(&ic, fcitx::Key(FcitxKey_Escape));
        engine.keyEvent(entry, cancel);
      }
      setBinding(false);
      tap();
      require(state->input_enabled_ && releases == 5, "a disabled Caps Lock switched or released the lock");
      // 会话建立时就请求了把快捷短语编码排进普通候选，和 macOS 一样。
      require(state->voice_host_options_.value("quick_phrase_candidates", false),
              "the session was not asked for quick phrase candidates");
      // 「大写锁定时使用英文标点」：宿主把每个键带的锁定状态报告给会话，锁定时没有组字的标点交给应用，即英文标点。智能标点关掉，免得前文决定标点。
      state->waitForPreferenceSave();
      {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["caps_lock_ascii_punctuation"] = true;
        snapshot["preferences"]["smart_punctuation"] = false;
        const auto document = snapshot.dump();
        response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        do {
          state->refreshPreferences();
          if (state->preferences_.value("caps_lock_ascii_punctuation", false) &&
              !state->preferences_.value("smart_punctuation", true))
            break;
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        } while (std::chrono::steady_clock::now() < deadline);
        require(state->preferences_.value("caps_lock_ascii_punctuation", false),
                "Caps Lock punctuation preference did not hot-reload");
      }
      const auto comma = [&](fcitx::KeyStates states) {
        fcitx::KeyEvent event(&ic, fcitx::Key(FcitxKey_comma, states));
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      {
        const auto before = ic.committed;
        require(comma(fcitx::KeyStates()) && ic.committed == before + "，",
                "a comma without Caps Lock did not commit Chinese punctuation");
        require(!comma(lock) && ic.committed == before + "，",
                "a comma under Caps Lock was not left to the application as English punctuation");
        require(comma(fcitx::KeyStates()) && ic.committed == before + "，，",
                "releasing Caps Lock did not bring Chinese punctuation back");
      }
      state->close();
      removeFixtureTree(directory);
      msime::linux_host::replace_caps_lock_release_for_testing(nullptr, nullptr);
      std::cout << "Fcitx5 Caps Lock tap, long press, chords, failed release, hot-reload, Caps Lock punctuation and quick phrase candidates passed\n";
      return 0;
    }
    // Ctrl+Shift+J 在中文方案和日语之间切换，从真实的 keyEvent 驱动：缺省关时交给应用；打开后中文方案去日语并记下这个中文方案，再按回到它；一次按下只切换一次；组字中先上屏；英文模式和 Engine 的英文模式里交给应用。
    if (argc == 3 && std::string(argv[2]) == "--japanese-toggle") {
      const fcitx::KeyStates ctrlShift{fcitx::KeyState::Ctrl, fcitx::KeyState::Shift};
      const auto chordKey = [&](bool release) {
        fcitx::KeyEvent event(&ic, fcitx::Key(release ? FcitxKey_j : FcitxKey_J, ctrlShift), release);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const auto stored = [&] {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        return snapshot.at("preferences");
      };
      const auto storedBecomes = [&](const char *scheme, const char *last) {
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
        while (std::chrono::steady_clock::now() < deadline) {
          const auto preferences = stored();
          if (preferences.value("scheme", std::string("quanpin")) == scheme &&
              preferences.value("last_chinese_scheme", std::string("quanpin")) == last)
            return true;
          std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        return false;
      };
      const auto setPreferences = [&](bool enabled, const char *scheme, const char *last) {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["keybindings"]["toggle_japanese_ctrl_shift_j"] = enabled;
        snapshot["preferences"]["scheme"] = scheme;
        snapshot["preferences"]["last_chinese_scheme"] = last;
        const auto document = snapshot.dump();
        response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        do {
          state->refreshPreferences();
          if (state->japanese_toggle_enabled_ == enabled && state->effectiveScheme() == scheme) return;
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        } while (std::chrono::steady_clock::now() < deadline);
        require(false, "Ctrl+Shift+J preference did not hot-reload");
      };
      require(state->ensure() && state->input_enabled_, "Ctrl+Shift+J test does not start in Chinese");
      // 缺省关：组合键交给应用，方案不变。
      require(!state->japanese_toggle_enabled_ && !chordKey(false) && !chordKey(true) &&
                  state->effectiveScheme() == "quanpin",
              "Ctrl+Shift+J was taken with the preference off");
      // 打开后从双拼出发；存储里记着的上一个中文方案故意是全拼，切走时要换成双拼。
      setPreferences(true, "shuangpin", "quanpin");
      require(chordKey(false) && state->view_.value("scheme", 0u) == 3 && storedBecomes("japanese", "shuangpin"),
              "Ctrl+Shift+J did not switch shuangpin to Japanese and remember shuangpin");
      // 按住时的自动重复和松开都归这次按下，不再切换。
      for (int repeat = 0; repeat < 3; ++repeat)
        require(chordKey(false) && state->view_.value("scheme", 0u) == 3, "a held Ctrl+Shift+J switched more than once");
      require(chordKey(true), "the Ctrl+Shift+J release escaped");
      require(chordKey(false) && chordKey(true) && state->view_.value("scheme", 0u) == 1 &&
                  storedBecomes("shuangpin", "shuangpin"),
              "Ctrl+Shift+J did not return from Japanese to shuangpin");
      // 组字中按下：先上屏，再切换。
      const auto committed = ic.committed;
      for (const auto sym : {FcitxKey_n, FcitxKey_i}) {
        fcitx::KeyEvent letter(&ic, fcitx::Key(sym));
        engine.keyEvent(entry, letter);
        require(letter.accepted(), "Ctrl+Shift+J composition test did not compose");
      }
      require(chordKey(false) && chordKey(true) && ic.committed.size() > committed.size() &&
                  ic.inputPanel().clientPreedit().toString().empty() && state->view_.value("scheme", 0u) == 3,
              "Ctrl+Shift+J did not commit the composition before switching");
      require(chordKey(false) && chordKey(true) && state->view_.value("scheme", 0u) == 1, "Ctrl+Shift+J did not switch back");
      // 英文模式交给应用。
      engine.input_mode_action_.activate(&ic);
      require(!state->input_enabled_ && !chordKey(false) && !chordKey(true) && state->effectiveScheme() == "shuangpin",
              "Ctrl+Shift+J was taken in English mode");
      engine.input_mode_action_.activate(&ic);
      // Engine 自己的英文模式（Ctrl+Shift+E）里 Fcitx5 仍是中文状态，这时同样交给应用。
      require(state->input_enabled_ && state->toggleEnglish() && state->view_.value("dedicated_english", false),
              "dedicated English fixture failed");
      require(!chordKey(false) && !chordKey(true) && state->effectiveScheme() == "shuangpin",
              "Ctrl+Shift+J was taken in the Engine's English mode");
      require(state->toggleEnglish() && !state->view_.value("dedicated_english", true), "leaving dedicated English failed");
      // 关掉后又交给应用。
      setPreferences(false, "shuangpin", "shuangpin");
      require(!chordKey(false) && !chordKey(true) && state->effectiveScheme() == "shuangpin",
              "a disabled Ctrl+Shift+J was taken");
      state->close();
      removeFixtureTree(directory);
      std::cout << "Fcitx5 Ctrl+Shift+J Japanese toggle, repeat, composition, English modes and hot-reload passed\n";
      return 0;
    }
    // The host's statistics gate, not the store, is what keeps an opt-out from reaching the statistics file. First with statistics off after a preference tick, then with them turned on in the store while the host has not ticked since: a commit or passthrough key that slipped past the host would be recorded by that open store and replace the document, so only the host's cached switch can keep it untouched. The preference reload below is the tick that opens the gate, and the check after it proves recording resumes.
    const auto setStatistics = [&](bool enabled) {
      const auto request = Json{{"directory", preferenceDirectory},
                                {"action", Json{{"operation", "set_enabled"}, {"enabled", enabled}}}}.dump();
      const auto result = response(msime_client_typing_statistics(
          reinterpret_cast<const uint8_t *>(request.data()), request.size()));
      require(result.value("enabled", !enabled) == enabled, "typing statistics store switch");
    };
    const auto statisticsTotal = [&] {
      const auto request = Json{{"directory", preferenceDirectory}, {"action", Json{{"operation", "load"}}}}.dump();
      return response(msime_client_typing_statistics(reinterpret_cast<const uint8_t *>(request.data()), request.size()))
          .value("total", uint64_t{});
    };
    const auto statisticsDocument = preferenceDirectory + "/typing-statistics.json";
    const auto statisticsIdentity = [&] {
      struct stat info {};
      require(::stat(statisticsDocument.c_str(), &info) == 0, "typing statistics document exists");
      return std::make_tuple(info.st_ino, info.st_size, info.st_mtim.tv_sec, info.st_mtim.tv_nsec);
    };
    // Three committed characters and one English-mode letter handed back to the application.
    const auto commitAndPassthrough = [&] {
      state->commitText("输入法");
      state->input_enabled_ = false;
      fcitx::KeyEvent letter(&ic, fcitx::Key(FcitxKey_a));
      engine.keyEvent(entry, letter);
      state->input_enabled_ = true;
      require(!letter.accepted(), "an English-mode letter is handed back to the application");
    };
    const auto typeWhileGateShut = [&](const char *message) {
      const auto before = statisticsIdentity();
      commitAndPassthrough();
      // Longer than a detached record thread needs to rewrite the document.
      std::this_thread::sleep_for(std::chrono::milliseconds(300));
      require(statisticsIdentity() == before, message);
    };
    setStatistics(false);
    state->refreshPreferences();
    require(state->preferences_job_.valid(), "a preference tick is queued");
    state->preferences_job_.wait();
    require(!fcitx_typing_statistics.enabled(), "the preference tick reads statistics as off");
    typeWhileGateShut("with statistics off, a commit and a passthrough key leave the statistics document untouched");
    setStatistics(true);
    typeWhileGateShut("before the next preference tick, commits are not recorded even though the store is on");
    require(!fcitx_typing_statistics.enabled(), "no tick ran, so the host switch is still off");
    require(statisticsTotal() == 0, "nothing was recorded while the host switch was off");
    // 聚合打字统计是用户显式开启的本地功能，存储层默认关闭，record 在关闭时如实不计。
    // 下面那条「提交计入统计」的断言此前建立在一个从未开启过的存储上，于是无论宿主做
    // 了什么都必然为 0——它从没被执行到，因为这个测试一直挂在更前面的皮肤断言上。
    {
      const auto enable = Json{{"directory", preferenceDirectory},
                               {"action", Json{{"operation", "set_enabled"},
                                               {"enabled", true}}}}.dump();
      response(msime_client_typing_statistics(
          reinterpret_cast<const uint8_t *>(enable.data()), enable.size()));
    }
    const auto currentSnapshot = response(msime_client_load_preferences(
        reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
    const auto changed = Json{{"format_version", 1},
                              {"revision", currentSnapshot.value("revision", uint64_t{}) + 1},
                              {"preferences", changedPreferences}};
    const auto changedDocument = changed.dump();
    auto saved = response(msime_client_save_preferences(
        reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
        currentSnapshot.value("revision", uint64_t{}),
        reinterpret_cast<const uint8_t *>(changedDocument.data()), changedDocument.size()));
    require(saved.value("revision", uint64_t{}) > currentSnapshot.value("revision", uint64_t{}),
            "preference store update");
    const auto reloadDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
    while (state->preferences_.value("number_row_selection", true) &&
           std::chrono::steady_clock::now() < reloadDeadline) {
      state->refreshPreferences();
      std::this_thread::sleep_for(std::chrono::milliseconds(5));
    }
    require(!state->preferences_.value("number_row_selection", true),
            "runtime preferences reload in active Fcitx session");
    require(fcitx_typing_statistics.enabled(), "the preference reload also opens the statistics gate");
    commitAndPassthrough();
    const auto resumedDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (statisticsTotal() < 4 && std::chrono::steady_clock::now() < resumedDeadline)
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    require(statisticsTotal() == 4, "after that tick a commit and a passthrough key are recorded again");
    // One per entry arrangeStatusArea lists for a focused context with a session (the toolbar and voice entries included). Adding or removing a status action changes this on purpose; the count is here so one going missing is noticed.
    require(ic.statusArea().actions(fcitx::StatusGroup::InputMethod).size() == 32,
            ("native status actions attached: " +
             std::to_string(ic.statusArea().actions(fcitx::StatusGroup::InputMethod).size()))
                .c_str());
    require(engine.learning_action_.isChecked(&ic),
            "learning status action reflects reloaded preference");
    engine.learning_action_.activate(&ic);
    // A toggle that throws is swallowed by the action's catch, which closes the
    // session: report that rather than only the setting that did not move.
    require(!state->preferences_.value("learning", true),
            ("learning status action disables user learning: session=" +
             std::to_string(state->session_) + " learning=" +
             std::to_string(state->preferences_.value("learning", true)))
                .c_str());
    engine.learning_action_.activate(&ic);
    require(state->preferences_.value("learning", false),
            "learning status action restores user learning");
    // Candidate maintenance labels follow the Windows candidate menu (置顶, 第 N 位), shared with the candidate actions and the IBus menu.
    require(engine.pin_action_.shortText(&ic) == "置顶", "pin action uses the Windows wording");
    require(engine.fix1_action_.shortText(&ic) == "固定到第 1 位" &&
                engine.fix5_action_.shortText(&ic) == "固定到第 5 位",
            "fix actions name the target position");
    require(engine.clear_action_.shortText(&ic) == "取消固定" && engine.remove_action_.shortText(&ic) == "删除",
            "clear and remove use the macOS candidate menu wording");
    {
      // 状态区的候选维护菜单与候选右键操作同一顺序：置顶、固定到第 1–5 位、取消固定、删除。
      const auto ordered = engine.maintenance_menu_.actions();
      require(ordered.size() == 8 && ordered.front() == &engine.pin_action_ && ordered[1] == &engine.fix1_action_ &&
                  ordered[5] == &engine.fix5_action_ && ordered[6] == &engine.clear_action_ && ordered.back() == &engine.remove_action_,
              "maintenance menu follows the macOS candidate menu order");
    }
    require(engine.candidate_layout_action_.shortText(&ic) == "候选：横向",
            "candidate layout action reflects reloaded preference");
    engine.candidate_layout_action_.activate(&ic);
    require(state->preferences_.value("candidate_layout", std::string{}) == "vertical",
            "candidate layout action cycles to vertical");
    engine.candidate_layout_action_.activate(&ic);
    require(state->preferences_.value("candidate_layout", std::string{}) == "horizontal",
            "candidate layout action cycles back to horizontal");
    require(engine.candidate_theme_action_.shortText(&ic) == "候选明暗：跟随颜色模式",
            "candidate theme action reads the preference snapshot");
    engine.candidate_theme_action_.activate(&ic);
    require(state->preferences_.value("candidate_theme", std::string{}) == "light",
            "candidate theme action updates the active session");
    // 主题 lists the shared catalogue's themes and then each installed package as its own entry, as IBus does; choosing an entry writes the same preferences the settings page does.
    const auto themeItem = [&](const std::string &title) -> FcitxGlobalThemeItemAction & {
      for (const auto *items : {&engine.global_theme_items_, &engine.global_theme_package_items_})
        for (const auto &item : *items)
          if (item->shortText(&ic) == title) return *item;
      throw std::runtime_error("theme menu lists " + title);
    };
    require(engine.global_theme_items_.size() == 7, "theme menu lists every shared global theme");
    require(engine.global_theme_package_items_.size() == 2 && engine.global_theme_menu_.actions().size() == 9,
            "theme menu lists each installed package after the global themes");
    for (auto *action : engine.global_theme_menu_.actions())
      require(!action->name().empty(), "every theme menu entry is registered");
    require(engine.global_theme_action_.shortText(&ic) == "主题（跟随系统）", "theme action starts at the shared default");
    require(themeItem("跟随系统").isChecked(&ic) && !themeItem("夜青").isChecked(&ic), "theme menu checks the current theme");
    themeItem("夜青").activate(&ic);
    require(state->preferences_.value("global_theme", std::string{}) == "night", "theme item selects its global theme");
    require(themeItem("夜青").isChecked(&ic) && !themeItem("跟随系统").isChecked(&ic), "theme menu follows the choice");
    // Any package is one activation away, not a walk through the ones before it.
    themeItem("纸鸢").activate(&ic);
    require(state->preferences_.value("global_theme", std::string{}) == "custom" &&
                state->preferences_.at("custom_theme").value("candidate_skin", std::string{}) == "kite" &&
                state->preferences_.at("custom_theme").value("base", std::string{}) == "paper",
            "a package entry draws the custom theme over that package and its base");
    require(themeItem("纸鸢").isChecked(&ic) && !themeItem("Solarized").isChecked(&ic) && !themeItem("夜青").isChecked(&ic),
            "theme menu checks the package drawn");
    themeItem("Solarized").activate(&ic);
    require(state->preferences_.at("custom_theme").value("candidate_skin", std::string{}) == "solarized" &&
                state->preferences_.at("custom_theme").value("base", std::string{}) == "system",
            "another package entry switches to that package");
    require(themeItem("Solarized").isChecked(&ic) && !themeItem("纸鸢").isChecked(&ic) && !themeItem("自定义").isChecked(&ic),
            "theme menu moves the check to the package drawn");
    require(engine.global_theme_action_.shortText(&ic) == "主题（Solarized）", "theme action names the package drawn");
    options["candidate_skin_catalog"]["packages"][0]["title"] = "Solarized 更新";
    std::ofstream(path) << options.dump();
    state->refreshProviderSockets();
    require(engine.global_theme_package_items_.size() == 2 && themeItem("Solarized 更新").isChecked(&ic),
            "package entries follow the runtime options");
    // 每一拍的主题同步：这个 Instance 不加载经典界面，一拍之后记下的尝试表明没有经典界面，再来几拍不变，也就不再重做（#5988）。
    {
      const auto remembered = engine.candidate_theme_attempt_;
      require(remembered.find("\"classicui\":false") != std::string::npos,
              "a tick without the classic UI remembers that it has nothing to take over");
      for (int count = 0; count < 3; ++count) state->refreshProviderSockets();
      require(engine.candidate_theme_attempt_ == remembered, "later ticks find nothing changed");
    }
    classicuiThemeTicks(engine);
    // 自定义 selects the custom theme as it stands, as the settings page's 自定义 card does, so the package it is drawn over stays and stays checked.
    themeItem("自定义").activate(&ic);
    require(state->preferences_.value("global_theme", std::string{}) == "custom" &&
                state->preferences_.at("custom_theme").value("candidate_skin", std::string{}) == "solarized" &&
                state->preferences_.at("custom_theme").value("base", std::string{}) == "system",
            "自定义 leaves the stored custom theme unchanged");
    require(!themeItem("自定义").isChecked(&ic) && themeItem("Solarized 更新").isChecked(&ic),
            "theme menu checks the package the custom theme is drawn over");
    themeItem("跟随系统").activate(&ic);
    require(state->preferences_.value("global_theme", std::string{}) == "system", "theme menu returns to the default");
    require(engine.mode_scope_action_.shortText(&ic) == "模式：应用",
            "mode scope action starts at application scope");
    engine.mode_scope_action_.activate(&ic);
    require(state->preferences_.value("ime_mode_scope", std::string{}) == "global",
            "mode scope action switches to global scope");
    engine.mode_scope_action_.activate(&ic);
    require(state->preferences_.value("ime_mode_scope", std::string{}) == "app",
            "mode scope action restores application scope");
    state->input_enabled_ = false;
    state->rememberInputMode();
    state->input_enabled_ = true;
    state->restoreInputMode();
    require(!state->input_enabled_, "application input mode is restored for the client");
    // Put Chinese mode back for this client: everything below types into the Engine, and leaving the remembered English mode in place made the first of those keys (the nine-key digit) pass straight through to the application.
    state->input_enabled_ = true;
    state->rememberInputMode();
    if (state->preferences_.value("cloud_candidates", false)) {
      require(engine.cloud_candidates_action_.isChecked(&ic),
              "cloud candidates status action reflects preference");
      engine.cloud_candidates_action_.activate(&ic);
      require(!state->preferences_.value("cloud_candidates", true),
              "cloud candidates status action disables live mode");
      engine.cloud_candidates_action_.activate(&ic);
      require(state->preferences_.value("cloud_candidates", false),
              "cloud candidates status action restores live mode");
    } else {
      require(!engine.cloud_candidates_action_.isChecked(&ic),
              "cloud candidates status action reflects disabled preference");
    }
    require(ic.statusArea().actions(fcitx::StatusGroup::InputMethod).size() == 32,
            ("status actions unchanged by the option toggles: " +
             std::to_string(ic.statusArea().actions(fcitx::StatusGroup::InputMethod).size()))
                .c_str());
    // 点托盘往往让应用失焦，而托盘菜单列的、点下去作用的都是最近聚焦的上下文：失焦后它的会话留着，勾选照旧，开关点了就生效；只往有焦点的输入框里写的入口先拿掉。别的上下文获得焦点时它才交出，换输入法时照旧清空。
    {
      const auto statusActions = [&] { return ic.statusArea().actions(fcitx::StatusGroup::InputMethod).size(); };
      const bool chinese = engine.input_mode_action_.isChecked(&ic);
      const bool fullwidth = engine.width_action_.isChecked(&ic);
      const auto scheme = engine.scheme_action_.shortText(&ic);
      blur();
      require(state->session_ != 0 && heldForTray() && statusActions() == 23,
              ("focus out keeps the session for the tray, without the focus-only tools: " +
               std::to_string(statusActions()))
                  .c_str());
      require(engine.input_mode_action_.isChecked(&ic) == chinese && engine.width_action_.isChecked(&ic) == fullwidth &&
                  engine.scheme_action_.shortText(&ic) == scheme,
              "the tray reads the same state after focus out");
      engine.input_mode_action_.activate(&ic);
      require(engine.input_mode_action_.isChecked(&ic) != chinese, "the tray switches 中/英 after focus out");
      engine.width_action_.activate(&ic);
      require(engine.width_action_.isChecked(&ic) != fullwidth, "the tray switches the width after focus out");
      ic.focusIn();
      engine.activate(entry, focus);
      require(state->session_ != 0 && !heldForTray() && statusActions() == 32,
              ("refocusing reopens the session with the status actions once each: session=" +
               std::to_string(state->session_) + " actions=" + std::to_string(statusActions()))
                  .c_str());
      require(engine.input_mode_action_.isChecked(&ic) != chinese && engine.width_action_.isChecked(&ic) != fullwidth,
              "what the tray switched holds once the context is focused again");
      engine.input_mode_action_.activate(&ic);
      engine.width_action_.activate(&ic);
      require(engine.input_mode_action_.isChecked(&ic) == chinese && engine.width_action_.isChecked(&ic) == fullwidth,
              "the focused context switches both back");
      blur();
      {
        FixtureContext other(instance.inputContextManager());
        other.focusIn();
        require(state->session_ == 0 && !heldForTray() && statusActions() == 0,
                ("a context gaining focus takes the tray from the held one: session=" +
                 std::to_string(state->session_) + " actions=" + std::to_string(statusActions()))
                    .c_str());
      }
      ic.focusIn();
      engine.activate(entry, focus);
      require(state->session_ != 0 && statusActions() == 32, "focus brings the session and the status actions back");
      fcitx::InputContextEvent switched(&ic, fcitx::EventType::InputContextSwitchInputMethod);
      engine.deactivate(entry, switched);
      require(state->session_ == 0 && statusActions() == 0,
              ("switching input methods removes the status actions: " + std::to_string(statusActions())).c_str());
      engine.activate(entry, focus);
      require(state->session_ != 0 && statusActions() == 32, "switching back restores the status actions");
    }
    require(engine.emoji_category_action_.shortText(&ic) == "表情：Emoji",
            "emoji category starts in the default catalog");
    engine.emoji_category_action_.activate(&ic);
    const auto kaomojiDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while ((state->emoji_category_ != "kaomoji" || state->emoji_items_.empty()) &&
           std::chrono::steady_clock::now() < kaomojiDeadline) {
      state->refreshEmoji();
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(state->emoji_category_ == "kaomoji" && !state->emoji_items_.empty(),
            "emoji category action loads kaomoji catalog");
    engine.emoji_category_action_.activate(&ic);
    const auto symbolsDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (state->emoji_job_.valid() && std::chrono::steady_clock::now() < symbolsDeadline) {
      state->refreshEmoji();
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    engine.emoji_category_action_.activate(&ic);
    require(state->emoji_category_.empty(), "emoji category action cycles back to default catalog");
    const auto defaultEmojiDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (state->emoji_job_.valid() && std::chrono::steady_clock::now() < defaultEmojiDeadline) {
      state->refreshEmoji();
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    engine.emoji_group_action_.activate(&ic);
    const auto groupsDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (state->emoji_groups_job_.valid() && std::chrono::steady_clock::now() < groupsDeadline) {
      state->refreshEmoji();
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(!state->emoji_groups_.empty(), "emoji group action loads catalog groups");
    engine.emoji_group_action_.activate(&ic);
    require(!state->emoji_group_.empty(), "emoji group action selects a group");
    const bool aiEnabled = state->preferences_.value("ai_assistant", Json::object())
                               .value("enabled", false);
    require(engine.ai_candidates_action_.isChecked(&ic) == aiEnabled,
            "AI status action reflects preference");
    engine.ai_candidates_action_.activate(&ic);
    require(state->preferences_.value("ai_assistant", Json::object()).value("enabled", false) != aiEnabled,
            "AI status action toggles live mode");
    engine.ai_candidates_action_.activate(&ic);
    require(state->preferences_.value("ai_assistant", Json::object()).value("enabled", false) == aiEnabled,
            "AI status action restores live mode");
    require(engine.translation_language_action_.shortText(&ic) == "翻译：英语",
            "translation language action starts in English");
    engine.translation_language_action_.activate(&ic);
    require(state->preferences_.value("translation_target_language", std::string{}) == "fr" &&
            engine.translation_language_action_.shortText(&ic) == "翻译：法语",
            "translation language action cycles to French");
    for (int index = 0; index < 6; ++index) engine.translation_language_action_.activate(&ic);
    require(state->preferences_.value("translation_target_language", std::string{}) == "en",
            "translation language action cycles back to English");
    require(engine.punctuation_lock_action_.shortText(&ic) == "标点：跟随",
            "punctuation lock status action starts in follow mode");
    engine.punctuation_lock_action_.activate(&ic);
    require(state->punctuation_lock_ == 1 && engine.punctuation_lock_action_.shortText(&ic) == "标点：中文",
            "punctuation lock cycles to Chinese mode");
    engine.punctuation_lock_action_.activate(&ic);
    require(state->punctuation_lock_ == 2 && engine.punctuation_lock_action_.shortText(&ic) == "标点：英文",
            "punctuation lock cycles to English mode");
    engine.punctuation_lock_action_.activate(&ic);
    require(state->punctuation_lock_ == 0 && engine.punctuation_lock_action_.shortText(&ic) == "标点：跟随",
            "punctuation lock cycles back to follow mode");
    require(engine.smart_punctuation_action_.isChecked(&ic),
            "smart punctuation status action reflects preference");
    engine.smart_punctuation_action_.activate(&ic);
    require(!state->preferences_.value("smart_punctuation", true),
            "smart punctuation status action toggles live mode");
    // 与设置页相同，总开关连同数字后、字母后直出一起写；否则设置页关过一次之后，这里打开的只是一个不起作用的总开关。
    require(!state->preferences_.value("smart_punctuation_direct_digit", true) &&
                !state->preferences_.value("smart_punctuation_direct_letter", true),
            "turning smart punctuation off from the status bar turns both halves off");
    engine.smart_punctuation_action_.activate(&ic);
    require(state->preferences_.value("smart_punctuation", false),
            "smart punctuation status action restores live mode");
    require(state->preferences_.value("smart_punctuation_direct_digit", false) &&
                state->preferences_.value("smart_punctuation_direct_letter", false),
            "turning smart punctuation on from the status bar turns both halves on");
    {
      // 三个键在同一次保存里落盘。
      const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
      Json saved;
      auto halvesOn = [&] {
        const auto stored = saved.value("preferences", Json::object());
        return stored.value("smart_punctuation", false) &&
               stored.value("smart_punctuation_direct_digit", false) &&
               stored.value("smart_punctuation_direct_letter", false);
      };
      while (std::chrono::steady_clock::now() < deadline) {
        saved = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        if (halvesOn()) break;
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
      }
      require(halvesOn(), "the status bar persists smart punctuation with both halves");
    }
    require(engine.smart_punctuation_repeat_action_.isChecked(&ic),
            "smart punctuation repeat status action reflects preference");
    engine.smart_punctuation_repeat_action_.activate(&ic);
    require(!state->preferences_.value("smart_punctuation_repeat", true),
            "smart punctuation repeat action toggles live mode");
    engine.smart_punctuation_repeat_action_.activate(&ic);
    require(state->preferences_.value("smart_punctuation_repeat", false),
            "smart punctuation repeat status action restores live mode");
    require(engine.chinese_punctuation_action_.isChecked(&ic),
            "Chinese punctuation status action reflects preference");
    engine.chinese_punctuation_action_.activate(&ic);
    require(!state->chinese_punctuation_, "Chinese punctuation status action toggles live mode");
    engine.chinese_punctuation_action_.activate(&ic);
    require(state->chinese_punctuation_, "Chinese punctuation status action restores live mode");
    require(engine.paired_punctuation_action_.isChecked(&ic),
            "paired punctuation status action reflects preference");
    engine.paired_punctuation_action_.activate(&ic);
    require(!state->paired_punctuation_, "paired punctuation status action toggles live mode");
    engine.paired_punctuation_action_.activate(&ic);
    require(state->paired_punctuation_, "paired punctuation status action restores live mode");
    require(engine.candidate_translation_action_.isChecked(&ic),
            "candidate translation status action reflects preference");
    engine.candidate_translation_action_.activate(&ic);
    require(!state->preferences_.value("candidate_translations", true),
            "candidate translation status action disables live mode");
    engine.candidate_translation_action_.activate(&ic);
    require(state->preferences_.value("candidate_translations", false),
            "candidate translation status action restores live mode");
    require(engine.maintenance_menu_.actions().size() == 8,
            "candidate maintenance menu attached");
    require(engine.clipboard_menu_.actions().size() == 11,
            "clipboard history management menu attached");
    require(engine.cloud_clipboard_menu_.actions().size() == 5,
            "cloud clipboard menu attached");
    require(engine.desktop_tools_menu_.actions().size() == 7,
            "desktop tools menu attached");
    // 状态区按 macOS 输入法菜单的顺序：中文输入、英文候选模式；繁体、全角、标点、译文；输入方案、主题；显示工具栏与工具栏子菜单、表情面板、云剪贴板、云词库、屏幕键盘、手写、语音；设置、关于；之后才是 Linux 独有的词库…。
    {
      const auto status = ic.statusArea().actions(fcitx::StatusGroup::InputMethod);
      const std::vector<fcitx::Action *> design{
          &engine.input_mode_action_, &engine.english_action_, &engine.traditional_action_, &engine.width_action_,
          &engine.chinese_punctuation_action_, &engine.candidate_translation_action_, &engine.scheme_action_,
          &engine.global_theme_action_, &engine.toolbar_enabled_action_, &engine.toolbar_action_,
          &engine.desktop_emoji_action_, &engine.desktop_cloud_clipboard_action_, &engine.cloud_dictionary_action_,
          &engine.keyboard_action_, &engine.handwriting_action_, &engine.voice_action_, &engine.voice_cancel_action_,
          &engine.settings_action_, &engine.about_action_, &engine.dictionary_action_};
      require(status.size() > design.size() && std::equal(design.begin(), design.end(), status.begin()),
              "status area follows the macOS input menu order");
      require(engine.input_mode_action_.shortText(&ic) == "中文输入" &&
                  engine.english_action_.shortText(&ic) == "英文候选模式（Ctrl+Shift+E）" &&
                  engine.traditional_action_.shortText(&ic) == "繁体输出" &&
                  engine.width_action_.shortText(&ic) == "全角字符（Ctrl+Shift+Space）" &&
                  engine.chinese_punctuation_action_.shortText(&ic) == "中文标点（Ctrl+.）" &&
                  engine.candidate_translation_action_.shortText(&ic) == "显示译文" &&
                  engine.desktop_emoji_action_.shortText(&ic) == "水杉表情面板…" &&
                  engine.handwriting_action_.shortText(&ic) == "手写输入…" &&
                  engine.dictionary_action_.shortText(&ic) == "词库…" &&
                  engine.settings_action_.shortText(&ic) == "水杉输入法设置…" &&
                  engine.about_action_.shortText(&ic) == "关于水杉输入法…",
              "status entries use the macOS menu labels");
      // 英文候选模式挂在顶层，不再收在「输入选项」里；那里的「中英混输」是另一项功能。
      const auto input = engine.input_group_menu_.actions();
      require(std::find(input.begin(), input.end(), &engine.english_action_) == input.end() &&
                  std::find(input.begin(), input.end(), &engine.traditional_action_) == input.end() &&
                  engine.mixed_english_action_.shortText(&ic) == "中英混输",
              "英文候选模式 and 繁体输出 move to the top level");
      // 输入方案这一行带上正在用的方案，子菜单按 Engine 的方案顺序排列。
      require(engine.scheme_action_.shortText(&ic).rfind("输入方案（", 0) == 0, "the scheme row names the scheme in use");
      const auto schemes = engine.scheme_menu_.actions();
      require(!schemes.empty() && schemes.front() == &engine.scheme_quanpin_action_ &&
                  engine.scheme_japanese_action_.shortText(&ic) == "日语" &&
                  engine.scheme_vietnamese_action_.shortText(&ic) == "越南语",
              "the scheme menu uses the macOS order and names");
    }
    // Nothing the status area listed before is lost: each moved action sits in one of the option groups, and every entry of those menus is registered so the D-Bus menus can reach it.
    {
      std::size_t grouped = 0;
      for (auto *menu : {&engine.input_group_menu_, &engine.punctuation_group_menu_, &engine.candidate_group_menu_})
        for (auto *action : menu->actions()) {
          require(!action->name().empty(), "every option group entry is registered");
          if (!action->isSeparator()) ++grouped;
        }
      require(grouped == 42, ("option groups hold the moved status actions: " + std::to_string(grouped)).c_str());
      for (auto *menu : {&engine.scheme_menu_, &engine.desktop_tools_menu_, &engine.helpcode_schema_menu_})
        for (auto *action : menu->actions())
          require(!action->name().empty(), "scheme and desktop tools entries are registered");
    }
    require(engine.candidate_page_size_menu_.actions().size() == 10,
            "candidate page-size menu attached, one to ten");
    engine.candidate_page_size3_.activate(&ic);
    require(state->preferences_.value("candidate_page_size", 0u) == 3,
            "candidate page-size action persists a larger page");
    require(state->view_.value("page_size", 0u) == 3,
            "candidate page-size action applies a larger page");
    engine.candidate_page_size2_.activate(&ic);
    require(state->view_.value("page_size", 0u) == 2,
            "candidate page-size action restores the configured page");
    require(engine.nine_key_menu_.actions().size() == 9,
            "nine-key spelling menu attached");
    engine.nine_key_action_.activate(&ic);
    require(state->view_.value("nine_key", false), "nine-key action enables nine-key mode");
    fcitx::KeyEvent nineKeyDigit(&ic, fcitx::Key(FcitxKey_6));
    engine.keyEvent(entry, nineKeyDigit);
    require(nineKeyDigit.accepted(), "nine-key digit starts a spelling composition");
    const auto spellings = state->view_.value("nine_key_spellings", Json::array());
    require(spellings.is_array() && !spellings.empty(),
            "nine-key mode exposes spelling choices");
    engine.nine_key_spelling1_.activate(&ic);
    require(state->view_.value("nine_key", false),
            "nine-key spelling action preserves nine-key mode");
    fcitx::KeyEvent nineKeyEscape(&ic, fcitx::Key(FcitxKey_Escape));
    engine.keyEvent(entry, nineKeyEscape);
    require(nineKeyEscape.accepted(), "nine-key spelling composition cleanup");
    engine.nine_key_action_.activate(&ic);
    require(!state->view_.value("nine_key", true), "nine-key action restores alphabetic mode");
    require(engine.emoji_menu_.actions().size() == 7,
            "emoji paging menu attached");
    const auto routeScript = std::string(directory) + "/route-helper.sh";
    const auto routeOutput = std::string(directory) + "/route-output";
    // The helper renames a finished file into place so the poll below never reads a half-written one.
    std::ofstream(routeScript) << "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$MSIME_TEST_ROUTE_OUTPUT.tmp\" && mv \"$MSIME_TEST_ROUTE_OUTPUT.tmp\" \"$MSIME_TEST_ROUTE_OUTPUT\"\n";
    require(chmod(routeScript.c_str(), 0700) == 0, "desktop route helper permissions");
    setenv("MSIME_CLIENT_SETTINGS_COMMAND", routeScript.c_str(), 1);
    setenv("MSIME_TEST_ROUTE_OUTPUT", routeOutput.c_str(), 1);
    const auto launchedRoute = [&](FcitxDesktopPanelAction &action, const std::string &label) {
      std::filesystem::remove(routeOutput);
      action.activate(&ic);
      const auto routeDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
      while (!std::filesystem::exists(routeOutput) && std::chrono::steady_clock::now() < routeDeadline)
        std::this_thread::sleep_for(std::chrono::milliseconds(5));
      require(std::filesystem::exists(routeOutput), (label + " desktop route helper launched").c_str());
      std::ifstream routeFile(routeOutput);
      std::string arguments;
      std::getline(routeFile, arguments);
      return arguments;
    };
    require(launchedRoute(engine.handwriting_action_, "handwriting") == "--route=handwriting",
            "desktop route argument propagated");
    // About, help, feedback and the local dictionary are settings sections: each opens its own page, as the IBus host and Windows do, rather than the settings home page.
    for (auto *action : {&engine.about_action_, &engine.help_action_, &engine.feedback_action_, &engine.dictionary_action_}) {
      const auto page = action == &engine.about_action_      ? std::string("about")
                        : action == &engine.help_action_     ? std::string("help")
                        : action == &engine.feedback_action_ ? std::string("feedback")
                                                             : std::string("dictionary");
      require(launchedRoute(*action, page) == "--route=settings:" + page,
              (page + " menu opens its settings section").c_str());
    }
    engine.emoji_action_.activate(&ic);
    const auto emojiDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (state->emoji_items_.empty() && std::chrono::steady_clock::now() < emojiDeadline) {
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
      state->refreshEmoji();
    }
    require(!state->emoji_items_.empty() && state->emoji_items_.size() <= 5,
            "emoji first page loaded asynchronously");
    const auto firstEmoji = state->emoji_items_.front().value("text", std::string{});
    require(!firstEmoji.empty() && !state->emoji_complete_, "emoji page exposes continuation");
    engine.emoji_next_action_.activate(&ic);
    const auto nextEmojiDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (state->emoji_offset_.offset == 0 && std::chrono::steady_clock::now() < nextEmojiDeadline) {
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
      state->refreshEmoji();
    }
    require(state->emoji_offset_.offset > 0 && !state->emoji_items_.empty(),
            "emoji next page loaded");
    require(state->emoji_items_.front().value("text", std::string{}).size() > 0,
            "emoji pagination returns catalog entries");
    const auto beforeEmoji = ic.committed;
    engine.emoji_item1_.activate(&ic);
    require(ic.committed != beforeEmoji, "emoji menu item commits selected text");
    engine.emoji_search_action_.activate(&ic);
    require(state->emoji_search_mode_, "emoji search action enters native search mode");
    auto searchKey = [&](fcitx::KeySym sym) {
      fcitx::KeyEvent event(&ic, fcitx::Key(sym));
      engine.keyEvent(entry, event);
      return event.accepted();
    };
    require(searchKey(FcitxKey_g), "emoji search accepts ASCII query input");
    const auto searchDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (state->emoji_items_.empty() && std::chrono::steady_clock::now() < searchDeadline) {
      state->refreshEmoji();
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(!state->emoji_items_.empty() && state->emoji_search_ == "g",
            "emoji search returns filtered catalog entries");
    const auto beforeSearchCommit = ic.committed;
    require(searchKey(FcitxKey_Return), "emoji search accepts selection key");
    require(ic.committed != beforeSearchCommit && !state->emoji_search_mode_,
            "emoji search commits the first result and exits");
    require(!engine.english_action_.isChecked(&ic), "English candidates initially disabled");
    require(msime_linux_simplified_to_traditional("汉语") == "漢語", "traditional conversion available");
    require(msime_linux_simplified_to_traditional("头发") == "頭髮", "traditional conversion is phrase-level OpenCC, not character by character");
    engine.traditional_action_.activate(&ic);
    require(state->traditional_, "traditional status action enables conversion");
    require(state->preferences_.value("traditional_chinese_output", false),
            "traditional action updates the live preference snapshot");
    require(state->preferences_snapshot_.value("preferences", Json::object())
                .value("traditional_chinese_output", false),
            "traditional action updates the revision snapshot");
    const auto traditionalSaveDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    Json savedTraditional;
    while (std::chrono::steady_clock::now() < traditionalSaveDeadline) {
      state->refreshPreferences();
      savedTraditional = response(msime_client_load_preferences(
          reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
      if (savedTraditional.value("preferences", Json::object())
              .value("traditional_chinese_output", false)) break;
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(savedTraditional.value("preferences", Json::object())
                .value("traditional_chinese_output", false),
            "traditional status action persists preference");
    engine.traditional_action_.activate(&ic);
    require(!state->traditional_, "traditional status action disables conversion");
    require(!state->preferences_.value("traditional_chinese_output", true),
            "traditional action clears the live preference snapshot");
    engine.english_action_.activate(&ic);
    require(engine.english_action_.isChecked(&ic), "status action enables English candidates");
    engine.english_action_.activate(&ic);
    require(!engine.english_action_.isChecked(&ic), "status action disables English candidates");
    engine.width_action_.activate(&ic);
    require(engine.width_action_.isChecked(&ic), "status action enables fullwidth");
    require(state->preferences_.value("character_width", std::string{}) == "fullwidth",
            "status action updates the live width preference snapshot");
    const auto widthSaveDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    Json savedWidth;
    while (std::chrono::steady_clock::now() < widthSaveDeadline) {
      savedWidth = response(msime_client_load_preferences(
          reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
      if (savedWidth.value("preferences", Json::object()).value("character_width", std::string{}) ==
          "fullwidth") break;
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(savedWidth.value("preferences", Json::object()).value("character_width", std::string{}) ==
                "fullwidth",
            "status action persists fullwidth preference");
    engine.width_action_.activate(&ic);
    require(!engine.width_action_.isChecked(&ic), "status action restores halfwidth");
    require(state->preferences_.value("character_width", std::string{}) == "halfwidth",
            "status action updates the live snapshot back to halfwidth");
    // 与上面那次切换同样是异步保存，此处原先立即回读，于是读到的往往还是上一个值。
    // 实测切回半角后文件在数十毫秒内更新并保持，所以缺的是等待而不是保存——两次检查
    // 用同一个轮询，免得这条断言的成败取决于机器快慢。
    const auto finalWidthDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    Json finalWidth;
    while (std::chrono::steady_clock::now() < finalWidthDeadline) {
      finalWidth = response(msime_client_load_preferences(
          reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
      if (finalWidth.value("preferences", Json::object()).value("character_width", std::string{}) ==
          "halfwidth") break;
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(finalWidth.value("preferences", Json::object()).value("character_width", std::string{}) ==
                "halfwidth",
            "serialized preference saves retain the latest width toggle");
    engine.input_mode_action_.activate(&ic);
    require(!state->input_enabled_, "input mode action disables Chinese input");
    fcitx::KeyEvent passthrough(&ic, fcitx::Key(FcitxKey_n));
    engine.keyEvent(entry, passthrough);
    require(!passthrough.accepted(), "disabled input mode passes keys through");
    // A toggle chord owns its stroke until the release, so every press here is followed by one.
    const auto ctrlSpace = [&] {
      fcitx::KeyEvent event(
          &ic, fcitx::Key(FcitxKey_space, fcitx::KeyStates(fcitx::KeyState::Ctrl)));
      engine.keyEvent(entry, event);
      const bool accepted = event.accepted();
      fcitx::KeyEvent release(
          &ic, fcitx::Key(FcitxKey_space, fcitx::KeyStates(fcitx::KeyState::Ctrl)), true);
      engine.keyEvent(entry, release);
      return accepted;
    };
    require(ctrlSpace(), "Ctrl+Space is handled during English passthrough");
    require(state->input_enabled_, "Ctrl+Space restores Chinese input");
    require(ctrlSpace(), "Ctrl+Space is handled in Chinese input");
    require(!state->input_enabled_, "Ctrl+Space switches to English input");
    fcitx::KeyEvent ctrlAltSpace(
        &ic, fcitx::Key(FcitxKey_space,
                        fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt}));
    engine.keyEvent(entry, ctrlAltSpace);
    fcitx::KeyEvent ctrlAltSpaceRelease(
        &ic, fcitx::Key(FcitxKey_space,
                        fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt}),
        true);
    engine.keyEvent(entry, ctrlAltSpaceRelease);
    require(ctrlAltSpace.accepted(),
            "enabled Ctrl+Alt+Space is handled during English passthrough");
    require(state->input_enabled_, "Ctrl+Alt+Space restores Chinese input");
    const auto key = [&](fcitx::KeySym sym) {
      fcitx::KeyEvent event(&ic, fcitx::Key(sym));
      engine.keyEvent(entry, event);
      return event.accepted();
    };
    // The same press with modifiers held; Fcitx normalises the raw key exactly as it does for a real keyboard.
    const auto keyWith = [&](fcitx::KeySym sym, fcitx::KeyStates states) {
      fcitx::KeyEvent event(&ic, fcitx::Key(sym, states));
      engine.keyEvent(entry, event);
      return event.accepted();
    };
    // Ctrl+Shift+F flips the character set once per press and leaves the composition in place, as on Windows: auto-repeat while it is held is swallowed, and the release (a lowercase f once Shift is let go first) ends the hold.
    {
      require(key(FcitxKey_h) && key(FcitxKey_a) && key(FcitxKey_n) && key(FcitxKey_y) && key(FcitxKey_u),
              "Ctrl+Shift+F test composes hanyu");
      const auto preedit = ic.inputPanel().clientPreedit().toString();
      const auto committed = ic.committed;
      require(ic.inputPanel().candidateList() && ic.inputPanel().candidateList()->size() > 0,
              "Ctrl+Shift+F test has candidates");
      const auto first = ic.inputPanel().candidateList()->candidate(0).text().toString();
      require(!state->traditional_, "Ctrl+Shift+F test starts simplified");
      const fcitx::KeyStates ctrlShift{fcitx::KeyState::Ctrl, fcitx::KeyState::Shift};
      for (int press = 0; press < 3; ++press) {
        fcitx::KeyEvent event(&ic, fcitx::Key(FcitxKey_F, ctrlShift));
        engine.keyEvent(entry, event);
        require(event.accepted(), "a held Ctrl+Shift+F press is consumed");
        require(state->traditional_, "a held Ctrl+Shift+F flips the character set only once");
      }
      fcitx::KeyEvent release(&ic, fcitx::Key(FcitxKey_f, fcitx::KeyStates(fcitx::KeyState::Ctrl)), true);
      engine.keyEvent(entry, release);
      require(release.accepted(), "the Ctrl+Shift+F release is consumed after Shift is let go");
      require(ic.inputPanel().clientPreedit().toString() == preedit && ic.committed == committed,
              "Ctrl+Shift+F keeps the composition instead of committing it");
      require(ic.inputPanel().candidateList() && ic.inputPanel().candidateList()->size() > 0 &&
                  ic.inputPanel().candidateList()->candidate(0).text().toString().rfind(
                      msime_linux_simplified_to_traditional(first), 0) == 0,
              "Ctrl+Shift+F rewrites the open candidates in traditional characters");
      fcitx::KeyEvent back(&ic, fcitx::Key(FcitxKey_F, ctrlShift));
      engine.keyEvent(entry, back);
      fcitx::KeyEvent backRelease(&ic, fcitx::Key(FcitxKey_F, ctrlShift), true);
      engine.keyEvent(entry, backRelease);
      require(back.accepted() && backRelease.accepted() && !state->traditional_,
              "the next Ctrl+Shift+F press switches back to simplified");
      require(key(FcitxKey_Escape) && ic.inputPanel().clientPreedit().toString().empty(),
              "Escape clears the Ctrl+Shift+F test composition");
    }
    // Resetting MSIME is what Windows gets by restarting its Server: the controller's ReloadAddonConfig (the settings page's restart button), Ctrl+Shift+Alt+R and the status-menu action all end the composition, destroy the Engine session and give the focused context a new one at once. None of them may depend on a helper program, so a fcitx5-remote that fails and records being run stands first on PATH.
    {
      const auto *pathVariable = std::getenv("PATH");
      const std::string savedPath = pathVariable ? pathVariable : "";
      const auto stubDirectory = std::string(directory) + "/reset-bin";
      const auto stubLog = std::string(directory) + "/fcitx5-remote.log";
      std::filesystem::create_directory(stubDirectory);
      const auto stub = stubDirectory + "/fcitx5-remote";
      std::ofstream(stub) << "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '" << stubLog << "'\nexit 1\n";
      require(chmod(stub.c_str(), 0755) == 0, "fcitx5-remote stub permissions");
      setenv("PATH", (stubDirectory + ":" + savedPath).c_str(), 1);
      const auto sessionGone = [](uint64_t handle) {
        try {
          response(msime_client_view(handle));
          return false;
        } catch (...) {
          return true;
        }
      };
      const auto compose = [&](const char *message) {
        require(key(FcitxKey_n) && key(FcitxKey_i) && !state->view_.at("candidates").empty() &&
                    !ic.inputPanel().clientPreedit().toString().empty(),
                message);
      };
      const auto requireReset = [&](uint64_t before, const std::string &committed, const char *message) {
        require(before != 0 && sessionGone(before), message);
        require(state->session_ != 0 && state->session_ != before, "the focused context has a new session at once");
        require(state->view_.value("editing_text", std::string()).empty() && state->view_.at("candidates").empty() &&
                    ic.inputPanel().clientPreedit().toString().empty() && !ic.inputPanel().candidateList(),
                "the reset ends the composition and clears the panel");
        require(ic.committed == committed, "the reset drops the composition as focus-out does, committing nothing");
      };
      compose("composition before ReloadAddonConfig");
      auto before = state->session_;
      auto committedBefore = ic.committed;
      engine.reloadConfig();
      requireReset(before, committedBefore, "ReloadAddonConfig destroys the old Engine session");
      compose("the new session composes after ReloadAddonConfig");
      require(key(FcitxKey_Escape), "cancel the composition after ReloadAddonConfig");
      compose("composition before Ctrl+Shift+Alt+R");
      before = state->session_;
      committedBefore = ic.committed;
      const fcitx::KeyStates chord{fcitx::KeyState::Ctrl, fcitx::KeyState::Shift, fcitx::KeyState::Alt};
      fcitx::KeyEvent press(&ic, fcitx::Key(FcitxKey_R, chord));
      engine.keyEvent(entry, press);
      require(press.accepted(), "Ctrl+Shift+Alt+R is consumed even though fcitx5-remote fails");
      requireReset(before, committedBefore, "Ctrl+Shift+Alt+R destroys the old Engine session");
      const auto reopened = state->session_;
      fcitx::KeyEvent repeat(&ic, fcitx::Key(FcitxKey_R, chord));
      engine.keyEvent(entry, repeat);
      require(repeat.accepted() && state->session_ == reopened, "auto-repeat of the held chord is swallowed without another reset");
      fcitx::KeyEvent release(&ic, fcitx::Key(FcitxKey_r, fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Alt}), true);
      engine.keyEvent(entry, release);
      require(release.accepted() && !state->maintenance_reload_held_, "the chord's release is consumed and ends the hold");
      compose("the new session composes after Ctrl+Shift+Alt+R");
      before = state->session_;
      committedBefore = ic.committed;
      engine.reload_service_action_.activate(&ic);
      requireReset(before, committedBefore, "the status-menu action destroys the old Engine session");
      require(!std::filesystem::exists(stubLog), "no reset path runs fcitx5-remote");
      setenv("PATH", savedPath.c_str(), 1);
    }
    // English mode keeps the "always Chinese punctuation" lock and fullwidth output, as Windows does with the IME closed; without either, keys pass through. The lock is set on the host field directly so no preference save races the checks.
    {
      require(ctrlSpace() && !state->input_enabled_, "English output test starts in English");
      const auto committedBy = [&](fcitx::KeySym sym, fcitx::KeyStates states = fcitx::KeyStates()) {
        const auto before = ic.committed;
        fcitx::KeyEvent event(&ic, fcitx::Key(sym, states));
        engine.keyEvent(entry, event);
        return std::make_pair(event.accepted(), ic.committed.substr(before.size()));
      };
      require(!state->fullwidthOutput(), "English output test starts halfwidth");
      require(committedBy(FcitxKey_comma) == std::make_pair(false, std::string{}),
              "English mode passes a comma through under the follow lock");
      state->punctuation_lock_ = 1;
      require(committedBy(FcitxKey_comma) == std::make_pair(true, std::string("，")),
              "the Chinese punctuation lock converts a comma in English mode");
      require(committedBy(FcitxKey_less) == std::make_pair(true, std::string("《")) &&
                  committedBy(FcitxKey_greater) == std::make_pair(true, std::string("》")),
              "the Chinese punctuation lock converts book title marks in English mode");
      require(committedBy(FcitxKey_a) == std::make_pair(false, std::string{}),
              "a halfwidth letter passes through under the Chinese punctuation lock");
      require(committedBy(FcitxKey_comma, fcitx::KeyStates(fcitx::KeyState::Ctrl)).first == false,
              "a Ctrl chord passes through under the Chinese punctuation lock");
      state->punctuation_lock_ = 0;
      fcitx::KeyEvent widen(&ic, fcitx::Key(FcitxKey_space,
                                            fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Shift}));
      engine.keyEvent(entry, widen);
      fcitx::KeyEvent widenRelease(&ic, fcitx::Key(FcitxKey_space,
                                                   fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Shift}), true);
      engine.keyEvent(entry, widenRelease);
      require(widen.accepted() && state->fullwidthOutput(), "Ctrl+Shift+Space turns on fullwidth in English mode");
      require(committedBy(FcitxKey_a) == std::make_pair(true, std::string("ａ")),
              "fullwidth English mode widens a letter");
      require(committedBy(FcitxKey_space) == std::make_pair(true, std::string("\u3000")),
              "fullwidth English mode widens Space");
      require(committedBy(FcitxKey_period) == std::make_pair(true, std::string("．")),
              "fullwidth English mode widens an ASCII period under the follow lock");
      fcitx::KeyEvent narrow(&ic, fcitx::Key(FcitxKey_space,
                                             fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Shift}));
      engine.keyEvent(entry, narrow);
      fcitx::KeyEvent narrowRelease(&ic, fcitx::Key(FcitxKey_space,
                                                    fcitx::KeyStates{fcitx::KeyState::Ctrl, fcitx::KeyState::Shift}), true);
      engine.keyEvent(entry, narrowRelease);
      require(narrow.accepted() && !state->fullwidthOutput(), "Ctrl+Shift+Space restores halfwidth");
      // surrounding text 无效时，输入法自己刚上屏的 ASCII 数字仍应作为智能标点前文。
      // Unicode 模式生成固定的数字，避免把测试依赖到编辑器原有内容。
      require(ctrlSpace() && state->input_enabled_,
              "smart punctuation shadow test could not return to Chinese mode");
      const auto shadowDigit = committedBy(FcitxKey_U, fcitx::KeyStates(fcitx::KeyState::Shift));
      require(shadowDigit.first, "Unicode mode could not start for smart punctuation shadow");
      for (const auto sym : {FcitxKey_0, FcitxKey_0, FcitxKey_3, FcitxKey_0})
        require(committedBy(sym).first, "Unicode digit input was not consumed");
      const auto unicodeSpace = committedBy(FcitxKey_space);
      require(unicodeSpace == std::make_pair(true, std::string("0")),
              "Unicode mode did not commit the synthetic ASCII digit");
      fcitx::KeyEvent unicodeSpaceRelease(&ic, fcitx::Key(FcitxKey_space), true);
      engine.keyEvent(entry, unicodeSpaceRelease);
      require(!unicodeSpaceRelease.accepted(),
              "Unicode Space release was unexpectedly consumed");
      // 提交后由客户端发布的新空文档才代表当前光标，不回退旧的数字影子。
      ic.surroundingText().setText("", 0, 0);
      require(committedBy(FcitxKey_period) == std::make_pair(true, std::string("。")),
              "Fcitx5 fell back to the shadow after receiving empty surrounding text");
      // 重建数字影子，核对文档确实不可用时，真实松键事件仍保留它。
      ic.surroundingText().invalidate();
      require(committedBy(FcitxKey_U, fcitx::KeyStates(fcitx::KeyState::Shift)).first,
              "Unicode mode could not restart for the released-key shadow check");
      for (const auto sym : {FcitxKey_0, FcitxKey_0, FcitxKey_3, FcitxKey_0})
        require(committedBy(sym).first, "Unicode digit input was not consumed after restart");
      const auto unicodeSpaceAgain = committedBy(FcitxKey_space);
      require(unicodeSpaceAgain == std::make_pair(true, std::string("0")),
              "Unicode mode did not commit the second synthetic ASCII digit");
      fcitx::KeyEvent unicodeSpaceAgainRelease(&ic, fcitx::Key(FcitxKey_space), true);
      engine.keyEvent(entry, unicodeSpaceAgainRelease);
      require(!unicodeSpaceAgainRelease.accepted(),
              "second Unicode Space release was unexpectedly consumed");
      require(committedBy(FcitxKey_period) == std::make_pair(false, std::string{}),
              "Fcitx5 did not keep an input-method ASCII digit as smart-punctuation context");
      require(ctrlSpace() && !state->input_enabled_,
              "smart punctuation shadow test could not restore English mode");
      // Ctrl+. works in English mode too, as Windows does with the IME closed: the chord is eaten, commas become Chinese until the next mode switch, and nothing is saved. A pinned English lock keeps ASCII whatever Ctrl+. says.
      const auto ctrlPeriod = [&] {
        fcitx::KeyEvent event(&ic, fcitx::Key(FcitxKey_period, fcitx::KeyStates(fcitx::KeyState::Ctrl)));
        engine.keyEvent(entry, event);
        const bool accepted = event.accepted();
        fcitx::KeyEvent release(&ic, fcitx::Key(FcitxKey_period, fcitx::KeyStates(fcitx::KeyState::Ctrl)), true);
        engine.keyEvent(entry, release);
        return accepted && !release.accepted();
      };
      const auto savedPunctuation = state->preferences_.value("chinese_punctuation", Json());
      require(ctrlPeriod(), "Ctrl+. is handled in English mode and its release passes through");
      require(committedBy(FcitxKey_comma) == std::make_pair(true, std::string("，")),
              "English mode converts a comma after Ctrl+.");
      require(state->preferences_.value("chinese_punctuation", Json()) == savedPunctuation,
              "English-mode Ctrl+. leaves the saved punctuation preference alone");
      // The status item shows what English mode types, and clicking it there is the same session-only toggle.
      require(engine.chinese_punctuation_action_.isChecked(&ic),
              "the punctuation status item shows the English-mode Ctrl+. choice");
      engine.chinese_punctuation_action_.activate(&ic);
      require(!engine.chinese_punctuation_action_.isChecked(&ic) &&
                  committedBy(FcitxKey_comma) == std::make_pair(false, std::string{}) &&
                  state->preferences_.value("chinese_punctuation", Json()) == savedPunctuation,
              "the punctuation status item toggles English-mode punctuation without saving");
      engine.chinese_punctuation_action_.activate(&ic);
      require(engine.chinese_punctuation_action_.isChecked(&ic),
              "the punctuation status item turns English-mode Chinese punctuation back on");
      require(ctrlSpace() && state->input_enabled_ && ctrlSpace() && !state->input_enabled_,
              "the English-mode Ctrl+. test switches modes twice");
      require(committedBy(FcitxKey_comma) == std::make_pair(false, std::string{}),
              "a mode round trip drops the English-mode Ctrl+. choice");
      state->punctuation_lock_ = 2;
      require(ctrlPeriod(), "Ctrl+. is handled in English mode under the English punctuation lock");
      require(committedBy(FcitxKey_comma) == std::make_pair(false, std::string{}),
              "the English punctuation lock keeps a comma ASCII after Ctrl+. in English mode");
      state->punctuation_lock_ = 0;
      require(committedBy(FcitxKey_comma) == std::make_pair(false, std::string{}),
              "Ctrl+. under the English punctuation lock left no choice behind");
      require(ctrlSpace() && state->input_enabled_, "English output test returns to Chinese");
      // Under the follow lock a mode switch re-resolves punctuation: English mode takes ASCII marks, and coming back restores Chinese ones even after a Ctrl+. choice.
      state->chinese_punctuation_ = false;
      state->syncSessionChinesePunctuation();
      require(ctrlSpace() && !state->input_enabled_ && !state->chinese_punctuation_,
              "switching to English under the follow lock selects ASCII punctuation");
      require(ctrlSpace() && state->input_enabled_ && state->chinese_punctuation_ &&
                  state->session_chinese_punctuation_,
              "switching back to Chinese under the follow lock restores Chinese punctuation");
      state->preferences_["chinese_punctuation"] = false;
      state->chinese_punctuation_ = false;
      state->syncSessionChinesePunctuation();
      require(ctrlSpace() && !state->input_enabled_ && !state->chinese_punctuation_ &&
                  ctrlSpace() && state->input_enabled_ && !state->chinese_punctuation_ &&
                  !state->session_chinese_punctuation_,
              "switching back to Chinese ignored saved English punctuation");
      state->preferences_["chinese_punctuation"] = true;
      state->chinese_punctuation_ = true;
      state->syncSessionChinesePunctuation();
    }
    // In Chinese mode a pinned lock holds as well, as Windows resolves Ctrl+. and the toolbar switch through ResolvePunctuationOpen: the chord is eaten, the punctuation state stays, and no preference save starts. The lock is set on the host field directly so no lock save races the checks.
    {
      require(state->input_enabled_ && !state->composingOrCandidates(),
              "Chinese-mode lock test starts idle in Chinese");
      require(!state->options_path_.empty(), "Chinese-mode lock test has a preference store a toggle would save to");
      state->waitForPreferenceSave();
      state->punctuation_lock_ = 2;
      state->chinese_punctuation_ = false;
      state->syncSessionChinesePunctuation();
      const auto savedPunctuation = state->preferences_.value("chinese_punctuation", Json());
      fcitx::KeyEvent chord(&ic, fcitx::Key(FcitxKey_period, fcitx::KeyStates(fcitx::KeyState::Ctrl)));
      engine.keyEvent(entry, chord);
      fcitx::KeyEvent chordRelease(&ic, fcitx::Key(FcitxKey_period, fcitx::KeyStates(fcitx::KeyState::Ctrl)), true);
      engine.keyEvent(entry, chordRelease);
      require(chord.accepted(), "Chinese-mode Ctrl+. is consumed under the English punctuation lock");
      require(!state->chinese_punctuation_ && !state->session_chinese_punctuation_,
              "Chinese-mode Ctrl+. does not override the English punctuation lock");
      require(!state->preferences_save_job_.valid() &&
                  state->preferences_.value("chinese_punctuation", Json()) == savedPunctuation,
              "Chinese-mode Ctrl+. under a lock saves nothing");
      engine.chinese_punctuation_action_.activate(&ic);
      require(!state->chinese_punctuation_ && !state->session_chinese_punctuation_ &&
                  !engine.chinese_punctuation_action_.isChecked(&ic),
              "the Chinese punctuation status item does not override the English punctuation lock");
      require(!state->preferences_save_job_.valid() &&
                  state->preferences_.value("chinese_punctuation", Json()) == savedPunctuation,
              "the Chinese punctuation status item under a lock saves nothing");
      state->punctuation_lock_ = 0;
      state->chinese_punctuation_ = true;
      state->syncSessionChinesePunctuation();
    }
    // 重复标点转中文 depends only on smart punctuation and its repeat switch, as Windows _CanInterceptSmartPunctuationRevert does; paired completion is a separate feature. A comma after an ASCII letter goes to the editor as ASCII, and the same key again inside the window replaces it with the Chinese mark.
    {
      require(state->input_enabled_ && !state->composingOrCandidates(),
              "repeat test starts idle in Chinese");
      state->paired_punctuation_ = false;
      state->smart_punctuation_ = true;
      state->smart_punctuation_repeat_ = true;
      state->forgetSmartPunctuationRepeat();
      const auto before = ic.committed;
      ic.surroundingText().setText("a", 1, 1);
      require(!key(FcitxKey_comma) && ic.committed == before,
              "smart punctuation hands a comma after a letter back to the editor");
      ic.surroundingText().setText("a,", 2, 2);
      require(key(FcitxKey_comma) && ic.committed == before + "，",
              "a repeated comma turns Chinese with paired completion off");
      ic.surroundingText().invalidate();
      state->paired_punctuation_ = true;
    }
    // 四个模式快捷键里的裸修饰键：按下只是布防，松开才切换，期间打了别的键或按住太久都
    // 不算。这一段此前没有任何覆盖，而实现被一条「松开或修饰键一律不处理」的返回挡在后
    // 面，于是裸 Shift 在这个宿主上一次都没生效过。
    {
      const auto modifier = [&](fcitx::KeySym sym, bool release, fcitx::KeyStates states = fcitx::KeyStates()) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym, states), release);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const auto shiftHeld = fcitx::KeyStates(fcitx::KeyState::Shift);
      require(state->input_enabled_, "bare modifier test starts in Chinese");
      modifier(FcitxKey_Shift_L, false);
      require(state->pure_shift_candidate_, "a bare Shift press arms the gesture");
      // Windows toggles on the bare release but still lets the application see it.
      require(!modifier(FcitxKey_Shift_L, true, shiftHeld), "the toggling release still reaches the application");
      require(!state->input_enabled_, "a bare Shift switches to English");
      // 切到英文之后还要能切回来：宿主在英文透传时依然处理模式快捷键，与 IBus 一致。
      modifier(FcitxKey_Shift_L, false);
      modifier(FcitxKey_Shift_L, true, shiftHeld);
      require(state->input_enabled_, "a bare Shift switches back from English passthrough");
      // 期间打了别的键，这个 Shift 就是组合键的一半。
      modifier(FcitxKey_Shift_L, false);
      require(key(FcitxKey_a), "a key typed while Shift is held still reaches the session");
      modifier(FcitxKey_Shift_L, true, shiftHeld);
      require(state->input_enabled_, "Shift used as part of a combination does not switch");
      require(key(FcitxKey_Escape), "cancel what the combination test composed");
      // 按住超过 500ms 是在用修饰键，不是手势。
      modifier(FcitxKey_Shift_L, false);
      std::this_thread::sleep_for(std::chrono::milliseconds(600));
      modifier(FcitxKey_Shift_L, true, shiftHeld);
      require(state->input_enabled_, "a Shift held past the window does not switch");
      // 只有松开、而且 keysym 不是 Shift_L 的那条路径。xkb 的
      // shift:both_capslock_cancel（两个 Shift 一起按切大写锁定，Omarchy 默认带着）
      // 把 Shift 键的符号改成了 Caps_Lock，同一套布局下 Wayland 前端还只派发松开事件：
      // 按 sym 比较永远不中，布防也从未发生，四个模式快捷键在这种机器上整个是死的。
      // 改为按键码识别（X11 50/62 是左右 Shift），并在没有按下事件时用「松开前 500ms
      // 内没有普通按键」代替按住时长那条判据。
      const auto capsLockShift = [&] {
        fcitx::KeyEvent event(&ic, fcitx::Key(FcitxKey_Caps_Lock, shiftHeld, 50), true);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      std::this_thread::sleep_for(std::chrono::milliseconds(600));
      require(state->input_enabled_, "release-only gesture starts in Chinese");
      require(!capsLockShift(), "a release-only Shift toggles without being consumed, even as Caps_Lock");
      require(!state->input_enabled_, "a release-only Shift identified by keycode switches");
      std::this_thread::sleep_for(std::chrono::milliseconds(600));
      capsLockShift();
      require(state->input_enabled_, "and switches back");
      // 判据是按键自己的修饰位，不是时间：按住 Shift 打出的字母带 Shift 位，那次松开是
      // 组合键的尾巴；而敲完拼音再点一下 Shift 不带，那是手势。后者正是「组字途中切英
      // 文」，最常用的一个操作，按时间窗口判会把它误杀。
      const auto shiftedKey = [&](fcitx::KeySym sym) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym, shiftHeld));
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      shiftedKey(FcitxKey_A);
      capsLockShift();
      require(state->input_enabled_, "a release after a Shift-modified key is not a gesture");
      key(FcitxKey_Escape);

      // 组字途中切英文，上屏的必须是读入串。敲 ni 再按 Shift 要得到 ni，不是「你」——
      // Windows 是这个语义，IBus 宿主也照它写着，而这个宿主此前调的是结束组合。
      std::this_thread::sleep_for(std::chrono::milliseconds(600));
      require(state->input_enabled_, "raw-commit check starts in Chinese");
      require(key(FcitxKey_n) && key(FcitxKey_i), "compose before switching to English");
      {
        const auto before = ic.committed;
        capsLockShift();
        require(!state->input_enabled_, "the gesture switched to English");
        const auto added = ic.committed.substr(before.size());
        require(added == "ni", ("switching to English commits the reading string, got: " + added).c_str());
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(600));
      capsLockShift();
      require(state->input_enabled_, "back to Chinese for the rest of the suite");

      // 裸 Ctrl 跟随自己的开关，默认关闭时不动。
      require(!state->mode_ctrl_enabled_, "bare Ctrl is off by default");
      modifier(FcitxKey_Control_L, false);
      modifier(FcitxKey_Control_L, true, fcitx::KeyStates(fcitx::KeyState::Ctrl));
      require(state->input_enabled_, "a bare Ctrl does not switch while its binding is off");
    }
    // 组字时 Shift+左右和 Ctrl+左右在全拼里也按音节移动拼音光标：发的是分段命令，运行时不会把它换成移动候选，以前全拼的 Ctrl+左右发字母光标命令，高亮不在首选时就成了移动候选。按住 Shift 再按方向键是组合键，松开 Shift 不切换中英文。候选窗显示时 Home 先回首选，已在首选时才把拼音光标移到开头。没有组字时这些组合键仍是应用的按键。
    {
      const auto shiftHeld = fcitx::KeyStates(fcitx::KeyState::Shift);
      const auto ctrlHeld = fcitx::KeyStates(fcitx::KeyState::Ctrl);
      const auto shiftEdge = [&](bool release) {
        fcitx::KeyEvent event(&ic, fcitx::Key(FcitxKey_Shift_L, release ? shiftHeld : fcitx::KeyStates()), release);
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const auto caret = [&] { return state->view_.value("caret_position", size_t{99}); };
      const auto highlighted = [&](size_t index) {
        const auto candidates = state->view_.value("candidates", Json::array());
        return index < candidates.size() && candidates[index].value("highlighted", false);
      };
      require(state->input_enabled_ && !state->composingOrCandidates(), "segment caret test starts idle in Chinese");
      require(!keyWith(FcitxKey_Left, shiftHeld) && !keyWith(FcitxKey_Right, shiftHeld) &&
                  !keyWith(FcitxKey_Left, ctrlHeld) && !keyWith(FcitxKey_Right, ctrlHeld),
              "with nothing composing Shift and Ctrl arrows stay the application's keys");
      require(key(FcitxKey_n) && key(FcitxKey_i) && key(FcitxKey_h) && key(FcitxKey_a) && key(FcitxKey_o) && caret() == 5,
              "segment caret test composes nihao");
      require(!shiftEdge(false) && keyWith(FcitxKey_Left, shiftHeld) && caret() == 2,
              "Shift+Left moves the quanpin caret to the preceding syllable");
      require(!shiftEdge(true) && state->input_enabled_ &&
                  state->view_.value("editing_text", std::string{}) == "nihao",
              "releasing Shift after Shift+Left neither switches to English nor touches the composition");
      require(!shiftEdge(false) && keyWith(FcitxKey_Right, shiftHeld) && caret() == 5,
              "Shift+Right moves the quanpin caret to the following syllable");
      require(!shiftEdge(true) && state->input_enabled_, "releasing Shift after Shift+Right does not switch");
      require(keyWith(FcitxKey_Left, ctrlHeld) && caret() == 2,
              "Ctrl+Left moves the quanpin caret by a syllable, not by a letter");
      require(keyWith(FcitxKey_Right, ctrlHeld) && caret() == 5,
              "Ctrl+Right moves the quanpin caret by a syllable, not by a letter");
      require(key(FcitxKey_Down) && highlighted(1), "the Home test moves the highlight off the first candidate");
      require(key(FcitxKey_Home) && highlighted(0) && caret() == 5,
              "Home off the first candidate returns the highlight to it and leaves the caret");
      require(key(FcitxKey_Home) && caret() == 0 && state->view_.value("editing_text", std::string{}) == "nihao",
              "Home on the first candidate moves the pinyin caret to the start");
      require(key(FcitxKey_Escape) && !state->composingOrCandidates(), "the segment caret test cancels its composition");
    }
    require(key(FcitxKey_n) && key(FcitxKey_i), "composition keys");
    require(ic.inputPanel().clientPreedit().toString() == "ni", "native preedit");
    auto horizontalPage = ic.inputPanel().candidateList();
    require(horizontalPage && horizontalPage->layoutHint() == fcitx::CandidateLayoutHint::Horizontal,
            "hot-loaded horizontal layout reaches native candidate list");
    blur();
    // 会话留给托盘菜单，但组字丢掉：之后的菜单动作不能把它当成正在输入的文字提交出去。
    const auto heldSession = state->session_;
    require(heldSession != 0 && state->view_.value("editing_text", std::string{}).empty(),
            "focus out keeps the session for the tray and drops the composition");
    require(ic.inputPanel().clientPreedit().empty(), "focus out clears preedit");
    ic.focusIn();
    engine.activate(entry, focus);
    require(state->session_ != 0 && state->session_ != heldSession, "focus in creates a fresh host session");
    // 新会话以存储为准：热加载存进去的横向布局不回到 runtime options 里的纵向（#6540）。再从托盘切回纵向，下面验证旧候选页保留自己的布局快照。
    require(state->preferences_.value("candidate_layout", std::string{}) == "horizontal",
            "a fresh session keeps the stored horizontal layout");
    engine.candidate_layout_action_.activate(&ic);
    require(state->preferences_.value("candidate_layout", std::string{}) == "vertical",
            "the tray switches the fresh session back to vertical");
    for (const auto sym : {FcitxKey_n, FcitxKey_i, FcitxKey_h, FcitxKey_a, FcitxKey_o,
                           FcitxKey_j, FcitxKey_i, FcitxKey_e})
      require(key(sym), "composition after refocus");
    const auto onlineQuery = response(msime_client_online_query(state->session_));
    state->refreshClipboard();
    const auto clipboardDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (state->clipboard_items_.empty() && std::chrono::steady_clock::now() < clipboardDeadline) {
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
      state->refreshClipboard();
    }
    require(!state->clipboard_items_.empty(), "clipboard history loaded asynchronously");
    const auto beforeClipboard = ic.committed;
    engine.clipboard_action_.activate(&ic);
    require(ic.committed == beforeClipboard + "剪贴板合成测试", "clipboard action commits newest history");
    engine.clipboard_remove1_.activate(&ic);
    const auto removeClipboardDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (std::any_of(state->clipboard_items_.begin(), state->clipboard_items_.end(),
                       [](const Json &item) { return (item.is_string() ? item.get<std::string>() : item.value("text", std::string{})) == "剪贴板合成测试"; }) &&
           std::chrono::steady_clock::now() < removeClipboardDeadline) {
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
      state->refreshClipboard();
    }
    require(std::none_of(state->clipboard_items_.begin(), state->clipboard_items_.end(),
                         [](const Json &item) { return (item.is_string() ? item.get<std::string>() : item.value("text", std::string{})) == "剪贴板合成测试"; }),
            "clipboard remove action updates history");
    engine.clipboard_clear_action_.activate(&ic);
    const auto clearClipboardDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (!state->clipboard_items_.empty() && std::chrono::steady_clock::now() < clearClipboardDeadline) {
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
      state->refreshClipboard();
    }
    require(state->clipboard_items_.empty(), "clipboard clear action updates history");
    engine.cloud_clipboard_action_.activate(&ic);
    const auto cloudDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (ic.committed.find("云剪贴板测试") == std::string::npos &&
           std::chrono::steady_clock::now() < cloudDeadline) {
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
      engine.cloud_clipboard_action_.activate(&ic);
    }
    require(ic.committed.find("云剪贴板测试") != std::string::npos, "cloud clipboard action commits provider entry");
    require(cloudProvider.get(), "cloud clipboard socket protocol");
    const auto beforeCloudSecond = ic.committed;
    engine.cloud_clipboard_item2_.activate(&ic);
    require(ic.committed == beforeCloudSecond + "云剪贴板第二条",
            "cloud clipboard menu commits selected provider entry");
    {
      // Moving the preferences store must release both the read and save
      // futures that still target the previous directory; otherwise the next
      // refresh remains blocked behind an old store operation.
      auto switched = options;
      const auto switchedPreferencesDirectory = std::filesystem::path(directory) / "preferences-switch";
      switched["preferences_directory"] = switchedPreferencesDirectory.string();
      std::ofstream(path) << switched.dump();
      std::promise<void> releaseRead;
      std::promise<void> releaseSave;
      auto readGate = releaseRead.get_future().share();
      auto saveGate = releaseSave.get_future().share();
      state->preferences_job_ = detachedJob([readGate] {
        readGate.wait();
        return Json::object();
      });
      state->preferences_save_job_ = detachedJob([saveGate] {
        saveGate.wait();
        return Json::object();
      });
      state->preferences_save_retry_ = PendingPreferenceSave{
          state->options_path_, {}, "synthetic", true};
      state->refreshProviderSockets();
      require(state->options_path_ == switchedPreferencesDirectory.string() &&
                  !state->preferences_job_.valid() && !state->preferences_save_job_.valid() &&
                  !state->preferences_save_retry_,
              "switching preference stores releases stale futures");
      releaseRead.set_value();
      releaseSave.set_value();
      std::ofstream(path) << options.dump();
      state->refreshProviderSockets();
      // 恢复目录也会清空快照；模拟下一次偏好计时器重读，后续动作才能使用有效修订号。
      state->refreshPreferences();
      require(state->preferences_job_.valid(), "恢复偏好目录后重新读取快照");
      state->preferences_job_.wait();
      state->refreshPreferences();
      require(state->preferences_snapshot_.contains("revision"), "恢复的偏好快照包含修订号");
    }
    {
      // Moving the local history store must release reads and mutations that
      // still belong to the previous directory; otherwise refreshClipboard()
      // waits on an old future forever and never starts the new read.
      auto switched = options;
      const auto switchedClipboardPath = std::filesystem::path(directory) / "clipboard-switch";
      switched["clipboard_history_path"] =
          (switchedClipboardPath / "clipboard_history.json").string();
      std::ofstream(path) << switched.dump();
      std::promise<void> releaseRead;
      std::promise<void> releaseMutation;
      auto readGate = releaseRead.get_future().share();
      auto mutationGate = releaseMutation.get_future().share();
      state->clipboard_job_ = detachedJob([readGate] {
        readGate.wait();
        return Json::object();
      });
      state->clipboard_mutation_job_ = detachedJob([mutationGate] {
        mutationGate.wait();
        return Json::object();
      });
      const auto generationBefore = state->clipboard_generation_;
      state->refreshProviderSockets();
      require(state->clipboard_generation_ > generationBefore &&
                  state->clipboard_path_ == switchedClipboardPath.string() &&
                  !state->clipboard_job_.valid() && !state->clipboard_mutation_job_.valid(),
              "switching clipboard stores releases stale futures");
      releaseRead.set_value();
      releaseMutation.set_value();
      std::ofstream(path) << options.dump();
      state->refreshProviderSockets();
    }
    {
      // A cloud clipboard socket switch has the same lifetime rule: the old
      // provider request must not block requests sent to the new socket.
      auto switched = options;
      const auto switchedCloudSocket = std::filesystem::path(directory) / "cloud-clipboard-switch.sock";
      switched["cloud_clipboard_provider_socket"] = switchedCloudSocket.string();
      std::ofstream(path) << switched.dump();
      std::promise<void> release;
      auto gate = release.get_future().share();
      state->cloud_clipboard_job_ = detachedJob([gate] {
        gate.wait();
        return Json::object();
      });
      const auto generationBefore = state->cloud_clipboard_generation_;
      state->refreshProviderSockets();
      require(state->cloud_clipboard_generation_ > generationBefore &&
                  state->cloud_clipboard_socket_ == switchedCloudSocket.string() &&
                  !state->cloud_clipboard_job_.valid(),
              "switching cloud clipboard sockets releases stale futures");
      release.set_value();
      std::ofstream(path) << options.dump();
      state->refreshProviderSockets();
    }
    if (ai) {
      require(onlineQuery.value("ai_eligible", false), "AI query eligible");
      require(onlineQuery.at("ai_assistant").value("enabled", false), "AI provider enabled");
      require(!onlineQuery.at("cloud_candidates").get<bool>(), "cloud disabled for AI-only test");
    }
    require(!state->online_socket_.empty(), "AI provider socket configured");
    const auto onlineDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
    while (response(msime_client_all_candidates(state->session_)).dump().find(suggestion) == std::string::npos &&
           std::chrono::steady_clock::now() < onlineDeadline) {
      state->refreshOnline();
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(response(msime_client_all_candidates(state->session_)).dump().find(suggestion) != std::string::npos,
            "provider candidate applied to full candidate list");
    const auto verifyPreferenceInvalidation = [&] {
      if (!ai) return;
      const auto epochBeforeSettings = state->online_epoch_;
      auto changedPreferences = state->preferences_snapshot_;
      changedPreferences["preferences"]["ai_assistant"]["prompt_custom_1"] =
          "synthetic changed prompt";
      require(state->applyPreferenceSnapshot(std::move(changedPreferences)),
              "AI preference update succeeds");
      require(state->online_epoch_ > epochBeforeSettings,
              "AI preference changes invalidate online completions");
      require(state->online_query_.empty() && state->online_slots_[1].query.empty(),
              "AI preference changes discard the old online query");
      auto cloudPreferences = state->preferences_snapshot_;
      cloudPreferences["preferences"]["cloud_candidates"] = true;
      state->online_query_ = "synthetic-cloud-query";
      state->online_slots_[0].query = state->online_query_;
      const auto onlineEpochBeforeCloud = state->online_epoch_;
      require(state->applyPreferenceSnapshot(std::move(cloudPreferences)),
              "cloud preference update succeeds");
      require(state->online_epoch_ > onlineEpochBeforeCloud && state->online_query_.empty(),
              "cloud preference changes invalidate online completions");
      auto translationPreferences = state->preferences_snapshot_;
      translationPreferences["preferences"]["translation_secondary_language"] = "ja";
      state->translation_query_ = "synthetic-translation-query";
      state->translation_pending_ = "synthetic-translation-pending";
      const auto translationEpochBeforeSettings = state->translation_epoch_;
      require(state->applyPreferenceSnapshot(std::move(translationPreferences)),
              "translation preference update succeeds");
      require(state->translation_epoch_ > translationEpochBeforeSettings &&
                  state->translation_query_.empty() && state->translation_pending_.empty(),
              "translation preference changes invalidate translation completions");
      auto pronunciationPreferences = state->preferences_snapshot_;
      pronunciationPreferences["preferences"]["candidate_pronunciation"] = true;
      state->translation_query_ = "synthetic-pronunciation-query";
      state->translation_pending_ = "synthetic-pronunciation-pending";
      const auto translationEpochBeforePronunciation = state->translation_epoch_;
      require(state->applyPreferenceSnapshot(std::move(pronunciationPreferences)),
              "pronunciation preference update succeeds");
      require(state->translation_epoch_ > translationEpochBeforePronunciation &&
                  state->translation_query_.empty() && state->translation_pending_.empty(),
              "pronunciation preference changes restart the translation request");
      // 打开「显示读音」后，候选的读音取自翻译工作线程记下的那份，按主目标语言那一行读；换焦点、换偏好时随翻译请求一起作废，用户刚下载读音表时记下的空串要重新问。
      const Json glossed{{"text", "爱"}, {"translation", "love; affection"}};
      state->pronunciations_.store(Json{{"love; affection", "/l\xCA\x8Cv/"}});
      require(state->candidatePronunciation(glossed) == "/l\xCA\x8Cv/", "pronunciation follows the stored reading");
      state->preferences_["translation_target_language"] = "fr";
      require(state->candidatePronunciation(glossed).empty(), "a French target is not read as English");
      state->preferences_["translation_target_language"] = "en";
      state->preferences_["candidate_pronunciation"] = false;
      require(state->candidatePronunciation(glossed).empty(), "the switch turned off hides the reading");
      state->preferences_["candidate_pronunciation"] = true;
      state->invalidateTranslationRequests();
      require(state->pronunciations_.size() == 0 && state->candidatePronunciation(glossed).empty(),
              "invalidated translation requests drop the stored readings");
    };
    provider.join();
    auto page = ic.inputPanel().candidateList();
    require(page && page->layoutHint() == fcitx::CandidateLayoutHint::Vertical,
            "the tray's vertical layout reaches native candidate list");
    require(horizontalPage->layoutHint() == fcitx::CandidateLayoutHint::Horizontal,
            "previous candidate page retains its layout snapshot");
    require(page && page->size() == 2 && page->toPageable()->hasNext(), "runtime candidate page");
    require(key(FcitxKey_KP_End), "keypad end candidate navigation");
    require(key(FcitxKey_KP_Home), "keypad home candidate navigation");
    require(key(FcitxKey_Page_Down), "page down");
    const auto oldCommit = ic.committed;
    page->candidate(0).select(&ic);
    require(ic.committed == oldCommit, "stale page must not commit");
    require(key(FcitxKey_Page_Up), "return to first page");
    bool selected = false;
    for (int pages = 0; pages < 100 && !selected; ++pages) {
      page = ic.inputPanel().candidateList();
      for (int i = 0; i < page->size(); ++i) {
        const auto displayed = page->candidate(i).text().toString();
        if (displayed.rfind(suggestion, 0) == 0) {
          page->candidate(i).select(&ic);
          selected = true;
          break;
        }
      }
      if (!selected) {
        require(page->toPageable()->hasNext(), "provider candidate reachable by paging");
        require(key(FcitxKey_Page_Down), "page to provider candidate");
      }
    }
    require(selected && ic.committed == oldCommit + suggestion, "exact provider candidate commit");
    verifyPreferenceInvalidation();
    {
      // A provider request can outlive focus. Its detached worker is fenced by
      // the epoch, but retaining the future would block the next session from
      // starting its own request until the old provider times out.
      std::promise<void> release;
      auto gate = release.get_future().share();
      state->online_slots_[0].job = detachedJob([gate] {
        gate.wait();
        return Json::object();
      });
      state->online_slots_[0].query = "synthetic-stale-query";
      state->invalidateOnlineRequests();
      require(!state->online_slots_[0].job.valid(),
              "invalidating online requests releases stale provider futures");
      release.set_value();
    }
    {
      // Translation has the same detached-worker lifetime as online
      // candidates; invalidating it must release a stale provider future.
      std::promise<void> release;
      auto gate = release.get_future().share();
      state->translation_job_ = detachedJob([gate] {
        gate.wait();
        return Json::object();
      });
      state->translation_query_ = "synthetic-stale-translation";
      state->invalidateTranslationRequests();
      require(!state->translation_job_.valid(),
              "invalidating translation requests releases stale provider futures");
      release.set_value();
    }
    require(key(FcitxKey_n) && key(FcitxKey_i), "second composition keys");
    // Recreate a displayed answer in the same synthetic composition so disabling the provider checks a real candidate removal after the selection above committed the first answer.
    const auto secondQuery = response(msime_client_online_query(state->session_)).dump();
    const auto secondCandidates = Json::array({suggestion}).dump();
    const uint8_t activeSource = ai ? 1 : 0;
    state->view_ = response(msime_client_apply_online_candidates(
        state->session_, reinterpret_cast<const uint8_t *>(secondQuery.data()), secondQuery.size(),
        reinterpret_cast<const uint8_t *>(secondCandidates.data()), secondCandidates.size(), activeSource)).at("view");
    state->render();
    require(response(msime_client_all_candidates(state->session_)).dump().find(suggestion) != std::string::npos,
            "synthetic provider answer is displayed before disabling it");
    // 已显示答案后禁用服务必须立即移除该答案；不能先更新偏好再清除，否则 Host API 会把回调视为过期。
    {
      auto disabled = state->preferences_snapshot_;
      if (ai)
        disabled["preferences"]["ai_assistant"]["enabled"] = false;
      else
        disabled["preferences"]["cloud_candidates"] = false;
      require(state->applyPreferenceSnapshot(std::move(disabled)),
              "disabling the active provider succeeds");
      require(response(msime_client_all_candidates(state->session_)).dump().find(suggestion) ==
                  std::string::npos,
              "disabling the active provider clears displayed candidates");
    }
    const auto beforeWordCharacter = ic.committed;
    require(key(FcitxKey_bracketleft), "configured word-to-character binding");
    require(ic.committed != beforeWordCharacter, "word-to-character commits selected edge");
    require(key(FcitxKey_n) && key(FcitxKey_i), "third composition keys");
    require(key(FcitxKey_KP_1), "keypad candidate selection");
    require(!ic.committed.empty(), "keypad selection commits candidate");
    require(key(FcitxKey_n) && key(FcitxKey_i), "fourth composition keys");
    require(key(FcitxKey_minus), "configured minus previous-page binding");
    require(key(FcitxKey_equal), "configured equal next-page binding");
    require(key(FcitxKey_Escape), "cancel after navigation");
    // Tab and Shift+Tab page the candidates by default (navigation.tab), as on macOS, Windows and IBus. A real keyboard sends Shift+Tab as Shift+ISO_Left_Tab; Fcitx can drop Shift from the normalized key, so the raw event decides direction.
    {
      const auto candidatePage = [&] { return state->view_.value("page", size_t{0}); };
      const auto preedit = [&] { return ic.inputPanel().clientPreedit().toString(); };
      const fcitx::KeyStates shiftState(fcitx::KeyState::Shift);
      require(state->navigation_.value("tab", true), "Tab paging is on by default");
      require(!key(FcitxKey_Tab), "an idle Tab belongs to the application");
      const auto beforeTab = ic.committed;
      require(key(FcitxKey_n) && key(FcitxKey_i) && candidatePage() == 0 &&
                  state->view_.value("page_count", size_t{0}) >= 3,
              "Tab paging test composes a multi-page ni");
      require(key(FcitxKey_Tab) && candidatePage() == 1, "Tab moves to the next candidate page");
      require(key(FcitxKey_Tab) && candidatePage() == 2, "a second Tab moves on again");
      const bool shiftTabAccepted = keyWith(FcitxKey_Tab, shiftState);
      require(shiftTabAccepted && candidatePage() == 1,
              ("Shift+Tab moves to the previous page: accepted=" + std::to_string(shiftTabAccepted) +
               " page=" + std::to_string(candidatePage())).c_str());
      require(keyWith(FcitxKey_ISO_Left_Tab, shiftState) && candidatePage() == 0,
              "Shift+ISO_Left_Tab moves to the previous page");
      require(key(FcitxKey_Tab) && candidatePage() == 1 && key(FcitxKey_ISO_Left_Tab) && candidatePage() == 0,
              "a back-tab without Shift still moves to the previous page");
      require(ic.committed == beforeTab && preedit() == "ni", "Tab paging commits nothing and keeps the spelling");
      // The temporary page of a candidate's translation senses (Ctrl+Enter) pages with Tab as well, instead of leaving the senses and paging the hidden Engine list.
      require(state->enterTranslationCandidates("一; 二; 三; 四; 五") && state->translationCandidatesActive() &&
                  candidatePage() == 0 && state->view_.value("page_count", size_t{0}) == 3,
              "translation senses open on a three-page overlay");
      require(key(FcitxKey_Tab) && state->translationCandidatesActive() && state->translation_page_ == 1 && candidatePage() == 1,
              "Tab pages the translation senses forward");
      require(state->translation_saved_view_.value("page", size_t{1}) == 0,
              "Tab leaves the Engine's own candidate page alone");
      require(keyWith(FcitxKey_Tab, shiftState) && state->translationCandidatesActive() && state->translation_page_ == 0,
              "Shift+Tab pages the translation senses back");
      require(key(FcitxKey_Tab) && keyWith(FcitxKey_ISO_Left_Tab, shiftState) &&
                  state->translationCandidatesActive() && state->translation_page_ == 0,
              "Shift+ISO_Left_Tab pages the translation senses back");
      require(key(FcitxKey_Tab) && key(FcitxKey_ISO_Left_Tab) && state->translationCandidatesActive() &&
                  state->translation_page_ == 0,
              "a back-tab without Shift pages the translation senses back");
      require(key(FcitxKey_Page_Down) && state->translationCandidatesActive() && state->translation_page_ == 1 &&
                  key(FcitxKey_Page_Up) && state->translation_page_ == 0,
              "Page Down and Page Up still page the translation senses");
      require(ic.committed == beforeTab, "paging the translation senses commits nothing");
      require(key(FcitxKey_Escape) && !state->translationCandidatesActive() && preedit().empty(),
              "Escape leaves the translation senses and the composition");
      // Turning navigation.tab off through the store reaches the open context on the next preference tick, the same reload the settings page triggers.
      const auto loadStore = [&] {
        return response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
      };
      const auto setTabPaging = [&](bool enabled) {
        auto snapshot = loadStore();
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["navigation"]["tab"] = enabled;
        snapshot["revision"] = revision + 1;
        const auto document = snapshot.dump();
        const auto saved = response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        require(saved.value("revision", uint64_t{}) > revision, "navigation.tab saved");
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        while (state->navigation_.value("tab", true) != enabled && std::chrono::steady_clock::now() < deadline) {
          state->refreshPreferences();
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        }
        return state->navigation_.value("tab", true) == enabled;
      };
      require(setTabPaging(false), "navigation.tab off reloads into the open context");
      require(!key(FcitxKey_Tab), "an idle Tab belongs to the application with Tab paging off");
      // With candidates showing, a disabled Tab finishes the composition with the highlighted candidate and then reaches the application, which is the Linux behaviour this pins; it does not page.
      require(key(FcitxKey_n) && key(FcitxKey_i) && candidatePage() == 0, "disabled Tab test composes ni");
      const auto beforeDisabledTab = ic.committed;
      require(!key(FcitxKey_Tab), "a disabled Tab is handed to the application");
      require(ic.committed.size() > beforeDisabledTab.size() && preedit().empty() &&
                  state->view_.value("editing_text", std::string()).empty(),
              "a disabled Tab commits the composition instead of paging");
      // Ctrl+Tab is an application shortcut whatever the preference: it drops the spelling and passes through.
      require(key(FcitxKey_n) && key(FcitxKey_i), "Ctrl+Tab test composes ni");
      const auto beforeCtrlTab = ic.committed;
      require(!keyWith(FcitxKey_Tab, fcitx::KeyStates(fcitx::KeyState::Ctrl)), "Ctrl+Tab is handed to the application");
      require(ic.committed == beforeCtrlTab && preedit().empty() &&
                  state->view_.value("editing_text", std::string()).empty(),
              "Ctrl+Tab cancels the composition without committing it");
      require(setTabPaging(true), "navigation.tab restored");
      require(key(FcitxKey_n) && key(FcitxKey_i), "Ctrl+Tab test with Tab paging on composes ni");
      const auto beforeEnabledCtrlTab = ic.committed;
      require(!keyWith(FcitxKey_Tab, fcitx::KeyStates(fcitx::KeyState::Ctrl)) && candidatePage() == 0 &&
                  ic.committed == beforeEnabledCtrlTab && preedit().empty(),
              "Ctrl+Tab cancels and passes through with Tab paging on as well");
    }
    const auto beforePunctuation = ic.committed;
    ic.surroundingText().setText("😀A", 2, 2);
    require(!key(FcitxKey_comma), "ASCII punctuation remains with editor");
    require(ic.committed == beforePunctuation, "ASCII pass-through must not also commit");
    ic.surroundingText().setText("A😀", 2, 2);
    require(key(FcitxKey_comma), "non-ASCII surrounding context");
    require(ic.committed == beforePunctuation + "，", "Unicode scalar cursor context");
    ic.surroundingText().setText("A", 1, 1);
    require(key(FcitxKey_apostrophe), "ASCII apostrophe enters punctuation routing");
    // Paired completion is on by default (#579), so the opening quote arrives with its closing mark, as on Windows.
    require(ic.committed == beforePunctuation + "，‘’", "apostrophe follows shared punctuation policy");
    {
      // 引号改成直角、方头括号改成空心：存进偏好、宿主读回后 View 报告新样式，' 补出「」，[ 补出〖〗。
      const auto saveStyles = [&](const Json &quote, const Json &bracket) {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["quote_style"] = quote;
        snapshot["preferences"]["bracket_style"] = bracket;
        snapshot["revision"] = revision + 1;
        const auto document = snapshot.dump();
        const auto saved = response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        require(saved.value("revision", uint64_t{}) > revision, "punctuation styles saved");
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        const auto marks = [&] { return state->view_.value("punctuation_marks", Json::object()); };
        while (marks().value("quotes", std::string()) != quote.get<std::string>() &&
               std::chrono::steady_clock::now() < deadline) {
          state->refreshPreferences();
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        }
        return marks() == Json{{"quotes", quote}, {"brackets", bracket}};
      };
      require(saveStyles("corner", "hollow"), "the view reports corner quotes and hollow brackets");
      const auto beforeCorner = ic.committed;
      ic.surroundingText().setText("A", 1, 1);
      require(key(FcitxKey_apostrophe) && ic.committed == beforeCorner + "「」",
              "apostrophe pairs corner quotes");
      ic.surroundingText().setText("A", 1, 1);
      require(key(FcitxKey_bracketleft) && ic.committed == beforeCorner + "「」〖〗",
              "left bracket pairs hollow brackets");
      require(saveStyles("curly", "solid"), "punctuation styles restored");
    }
    // Voice input replaces an open composition, as on Windows and in the IBus host, so this runs after the checks that need the refocus composition and brings a composition of its own.
    require(key(FcitxKey_n) && key(FcitxKey_i) &&
                ic.inputPanel().clientPreedit().toString() == "ni",
            "composition before voice input");
    const auto committedBeforeVoice = ic.committed;
    auto voiceProvider = std::async(std::launch::async, [voiceServer] {
      pollfd ready{voiceServer, POLLIN, 0};
      if (poll(&ready, 1, kProviderAcceptMs) <= 0) { close(voiceServer); return false; }
      const int client = accept(voiceServer, nullptr, nullptr);
      if (client < 0) { close(voiceServer); return false; }
      char request[4096]{};
      const auto count = read(client, request, sizeof(request) - 1);
      uint64_t generation = 1;
      try {
        generation = Json::parse(request, request + std::max<ssize_t>(count, 0))
                         .at("query").at("generation").get<uint64_t>();
      } catch (...) {}
      const auto partial = Json{{"ok", true}, {"type", "partial"}, {"generation", generation},
                                {"text", "语音中"}}.dump() + "\n";
      const auto status = Json{{"ok", true}, {"type", "status"}, {"generation", generation},
                               {"phase", "recognizing"}}.dump() + "\n";
      const auto level = Json{{"ok", true}, {"type", "level"}, {"generation", generation},
                              {"level", 0.7}}.dump() + "\n";
      const auto final = Json{{"ok", true}, {"type", "final"}, {"generation", generation},
                              {"text", "语音测试"}}.dump() + "\n";
      const bool valid = count > 0 && std::string(request, count).find("voice") != std::string::npos;
      const bool partialSent = send(client, partial.data(), partial.size(), MSG_NOSIGNAL) ==
                               static_cast<ssize_t>(partial.size());
      const bool statusSent = send(client, status.data(), status.size(), MSG_NOSIGNAL) ==
                              static_cast<ssize_t>(status.size());
      const bool levelSent = send(client, level.data(), level.size(), MSG_NOSIGNAL) ==
                             static_cast<ssize_t>(level.size());
      std::this_thread::sleep_for(std::chrono::milliseconds(100));
      const bool finalSent = send(client, final.data(), final.size(), MSG_NOSIGNAL) ==
                             static_cast<ssize_t>(final.size());
      close(client); close(voiceServer);
      return valid && partialSent && statusSent && levelSent && finalSent;
    });
    fcitx::KeyEvent voiceHotkey(&ic,
        fcitx::Key(FcitxKey_F9, fcitx::KeyStates{fcitx::KeyState::Ctrl}));
    engine.keyEvent(entry, voiceHotkey);
    require(voiceHotkey.accepted(), "Ctrl+F9 starts voice input");
    require(!state->wave_overlay_.actions_visible() && state->wave_overlay_.listening,
            "voice start shows the bare wave; the round actions wait for a Space lock or recognition");
    bool observedVoicePartial = false;
    bool observedVoicePreedit = false;
    const auto voiceDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (ic.committed.find("语音测试") == std::string::npos &&
           std::chrono::steady_clock::now() < voiceDeadline) {
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
      if (state->voice_mailbox_) {
        std::lock_guard lock(state->voice_mailbox_->mutex);
        observedVoicePartial = observedVoicePartial || state->voice_mailbox_->partial == "语音中";
      }
      state->refreshVoice();
      observedVoicePreedit = observedVoicePreedit ||
                             ic.inputPanel().clientPreedit().toString() == "语音中";
    }
    const bool voiceProtocolValid = voiceProvider.get();
    require(observedVoicePartial || state->voice_partial_seen_,
            ("voice action receives provider partial text: provider_valid=" +
             std::to_string(voiceProtocolValid) +
             " job_valid=" + std::to_string(state->voice_job_.valid()) +
             " failure_visible=" + std::to_string(state->voice_failure_visible_) +
             " committed=" + std::to_string(ic.committed.find("语音测试") != std::string::npos) +
             " aux=" + ic.inputPanel().auxUp().toString()).c_str());
    require(observedVoicePreedit,
            "streaming Doubao text reaches preedit even with a stored ctrl_v commit mode");
    require(state->voice_phase_seen_ && state->voice_level_seen_,
            "voice action receives provider status and level");
    require(ic.committed == committedBeforeVoice + "语音测试",
            "voice action commits provider text in place of the composition");
    // render() always writes one segment, empty when nothing is composing, and fcitx::Text::empty() counts segments rather than text, so the check is on the text the client shows.
    require(ic.inputPanel().clientPreedit().toString().empty(),
            "final voice result clears streaming preedit");
    require(!state->wave_overlay_visible_, "voice completion hides the native wave overlay");
    require(voiceProtocolValid, "voice socket protocol");
    Json statistics;
    const auto statisticsDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (std::chrono::steady_clock::now() < statisticsDeadline) {
      const auto request = Json{{"directory", preferenceDirectory},
                                {"action", Json{{"operation", "load"}}}}.dump();
      statistics = response(msime_client_typing_statistics(
          reinterpret_cast<const uint8_t *>(request.data()), request.size()));
      if (statistics.value("total", uint64_t{}) > 0) break;
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(statistics.value("total", uint64_t{}) > 0,
            "committed Fcitx text is recorded in aggregate typing statistics");
    state->voice_loading_ = true;
    state->voice_ralt_held_ = true;
    state->voice_hotkey_hold_space_lock_ = true;
    fcitx::KeyEvent voiceLockDown(&ic, fcitx::Key(FcitxKey_space));
    engine.keyEvent(entry, voiceLockDown);
    require(voiceLockDown.accepted() && state->voice_space_locked_ &&
                state->voice_space_consumed_,
            "Space locks an active hold-to-record voice shortcut");
    state->voice_loading_ = false;
    state->voice_ralt_held_ = false;
    state->voice_space_consumed_ = false;
    state->voice_space_locked_ = false;
    const auto committedBeforeCancel = ic.committed;
    // 使用产品相同的后台任务；std::async 的最后一份 future 析构会等待，无法验证非阻塞取消。
    state->voice_job_ = detachedJob([] {
      std::this_thread::sleep_for(std::chrono::milliseconds(250));
      return Json{{"text", "已取消语音"}};
    });
    state->voice_loading_ = true;
    state->voice_socket_.clear();
    state->voice_generation_ = 0;
    const auto cancelStarted = std::chrono::steady_clock::now();
    require(state->cancelVoice(), "voice cancellation accepts an active delayed provider");
    require(std::chrono::steady_clock::now() - cancelStarted < std::chrono::milliseconds(100),
            "voice cancellation does not wait for the provider future");
    require(!state->voice_job_.valid(),
            "voice cancellation releases the stale provider future");
    const auto cancelDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
    while (state->voice_job_.valid() && std::chrono::steady_clock::now() < cancelDeadline) {
      state->refreshVoice();
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(!state->voice_job_.valid(), "cancelled voice future is reclaimed asynchronously");
    require(ic.committed == committedBeforeCancel, "cancelled voice result is not committed");
    // A native surface that cannot show (GNOME Wayland has no layer-shell) hands the recording's status to the auxiliary text, as the IBus FallbackSurface does, instead of leaving the recording invisible.
    {
      struct UnavailableSurface final : msime::linux_host::WaveOverlaySurface {
        explicit UnavailableSurface(int &count) : shows(count) {}
        bool show(const msime::linux_host::WaveOverlayModel &) override {
          ++shows;
          return false;
        }
        void update(const msime::linux_host::WaveOverlayModel &) override {}
        void hide() override {}
        int &shows;
      };
      int shows = 0;
      auto nativeSurface = std::move(state->wave_overlay_surface_);
      state->wave_overlay_surface_ = std::make_unique<UnavailableSurface>(shows);
      state->wave_overlay_failed_ = false;
      state->wave_overlay_visible_ = false;
      state->voice_loading_ = true;
      state->voice_phase_ = msime::linux_host::kVoiceStatusRecording;
      // 一次新录音的模型；Xlib 的 `None` 宏使这里无法写出 `CompactStatus::None`。
      state->wave_overlay_.reset();
      const auto voiceAux = [&] { return ic.inputPanel().auxUp().toString(); };
      state->updateVoiceOverlay();
      require(shows == 1 && state->wave_overlay_failed_ && !state->wave_overlay_visible_,
              "a surface that cannot show is marked failed");
      require(voiceAux().rfind("正在录音…  麦克风 [", 0) == 0,
              "a failed surface falls back to the auxiliary text the IBus host shows");
      state->voice_transcript_ = "你好";
      state->updateVoiceOverlay();
      require(shows == 1 && voiceAux() == msime::linux_host::wave_overlay_feedback_text(state->wave_overlay_) &&
                  voiceAux().find("\n你好") != std::string::npos,
              "later updates stay on the auxiliary text without retrying the surface");
      state->voice_space_locked_ = true;
      state->updateVoiceOverlay();
      require(voiceAux().rfind(msime::linux_host::kVoiceStatusLockedHint, 0) == 0,
              "a Space-locked recording says so in the auxiliary text");
      state->voice_space_locked_ = false;
      state->updateVoiceOverlay();
      const auto statusBeforeRedraw = voiceAux();
      state->render();
      require(voiceAux() == statusBeforeRedraw, "a panel redraw keeps the voice status");
      state->voice_loading_ = false;
      state->voice_transcript_.clear();
      state->wave_overlay_failed_ = false;
      state->wave_overlay_surface_ = std::move(nativeSurface);
      ic.inputPanel().setAuxUp(fcitx::Text());
    }
    // 画出来的浮层在自己的 `update()` 里报告点击，所以取消会在 `update()` 返回之前拆掉帧定时器和显示连接监听（帧定时器自己的回调之后不再经捕获读取任何东西，由 `fcitx5_contract.py` 固定）；识别进行中点确认只收起浮条，不再向提供商发送第二次停止。这里不运行事件循环：sd-event 循环一旦退出就不再接受新的事件源。
    {
      using Action = msime::linux_host::WaveOverlayModel::Action;
      struct ActingSurface final : msime::linux_host::WaveOverlaySurface {
        explicit ActingSurface(FcitxState &owner) : state(owner) {}
        bool show(const msime::linux_host::WaveOverlayModel &) override { return true; }
        void update(const msime::linux_host::WaveOverlayModel &) override {
          ++updates;
          if (!pending) return;
          const auto action = *pending;
          pending.reset();
          state.handleVoiceOverlayAction(action);
        }
        void hide() override { ++hides; }
        bool animates() const override { return true; }
        FcitxState &state;
        std::optional<Action> pending;
        int updates = 0;
        int hides = 0;
      };
      auto nativeSurface = std::move(state->wave_overlay_surface_);
      auto acting = std::make_unique<ActingSurface>(*state);
      auto *surface = acting.get();
      state->wave_overlay_surface_ = std::move(acting);
      state->wave_overlay_failed_ = false;
      state->wave_overlay_visible_ = false;
      state->wave_overlay_dismissed_ = false;
      state->voice_job_ = {};
      state->voice_mailbox_.reset();
      state->voice_loading_ = true;
      state->voice_socket_.clear();
      state->voice_generation_ = 0;
      state->voice_phase_ = msime::linux_host::kVoiceStatusRecording;
      state->wave_overlay_.reset();
      state->updateVoiceOverlay();
      require(state->wave_overlay_visible_ && state->wave_overlay_frame_timer_,
              "a drawn recording runs the wave's frame timer");
      surface->pending = Action::Cancel;
      state->updateVoiceOverlay();
      require(!surface->pending && surface->updates >= 1, "the click was delivered from inside update()");
      require(!state->voice_loading_ && !state->wave_overlay_visible_ && surface->hides >= 1 &&
                  !state->wave_overlay_frame_timer_ && !state->wave_overlay_io_,
              "Cancel from inside update() ends the recording and stops the overlay's sources");
      // 识别进行中：浮条显示文字标签，没有帧定时器，点确认只收起它。
      state->voice_loading_ = true;
      state->voice_socket_ = "/nonexistent/msime-voice-provider.sock";
      state->voice_generation_ = 7;
      state->voice_phase_ = msime::linux_host::kVoiceStatusRecognizing;
      state->wave_overlay_.reset();
      state->wave_overlay_.compact_status = msime::linux_host::WaveOverlayModel::CompactStatus::Recognizing;
      state->updateVoiceOverlay();
      require(state->wave_overlay_visible_ && !state->wave_overlay_frame_timer_ &&
                  state->wave_overlay_.actions_visible(),
              "pending recognition shows its actions and needs no frame timer");
      require(state->stopVoice() && state->voice_loading_ && !state->wave_overlay_.failure,
              "a second stop while recognition is pending sends the provider nothing and cancels nothing");
      state->handleVoiceOverlayAction(Action::Confirm);
      require(state->wave_overlay_dismissed_ && !state->wave_overlay_visible_ && state->voice_loading_,
              "Confirm while recognition is pending only takes the bar down");
      state->voice_socket_.clear();
      state->voice_generation_ = 0;
      require(state->cancelVoice(), "the pending recognition is cancelled for the next case");
      state->wave_overlay_dismissed_ = false;
      state->wave_overlay_.reset();
      state->wave_overlay_surface_ = std::move(nativeSurface);
      ic.inputPanel().setAuxUp(fcitx::Text());
    }
    // 提供商没给出结果算提供商失败，与 IBus 宿主一致；缺了具名依赖时说明要装什么。两者都不能显示成「未识别到文字」，因为重新录音解决不了。这里没有浮层，提示走辅助行；测试环境有没有显示连接不影响结果。
    auto failureSurface = std::move(state->wave_overlay_surface_);
    state->wave_overlay_visible_ = false;
    for (const auto &[providerError, notice] : std::vector<std::pair<std::string, std::string>>{
             {"voice_dependency_missing:websockets", "豆包语音需要 websockets 15 或更高版本，请安装 python3-websockets"},
             {"voice_dependency_missing:recorder", "未找到录音工具，请安装 pulseaudio-utils、pipewire-bin 或 alsa-utils"},
             {"voice_dependency_missing:local_asr", "本地语音识别组件无法加载，请重新安装输入法"},
             {"", "语音输入失败，请检查语音服务、麦克风及提供商配置后重试"}}) {
      const auto committedBeforeFailure = ic.committed;
      const auto error = providerError;
      state->voice_job_ = std::async(std::launch::async, [error] {
        return Json{{"provider_error", error}};
      }).share();
      state->voice_job_.wait();
      state->voice_mailbox_ = std::make_shared<FcitxVoiceMailbox>();
      state->voice_loading_ = true;
      state->voice_cancelled_ = false;
      state->refreshVoice();
      require(!state->voice_job_.valid() && !state->voice_loading_, "a failed voice result is reclaimed");
      require(ic.committed == committedBeforeFailure, "a failed voice result commits nothing");
      require(ic.inputPanel().auxUp().toString() == notice,
              "a provider failure shows its fixed notice");
      require(state->wave_overlay_.failure && !state->wave_overlay_.actions_visible(),
              "a failure notice shows its message alone, without actions");
      // Esc 在六秒到期之前收起提示。
      fcitx::KeyEvent dismiss(&ic, fcitx::Key(FcitxKey_Escape));
      engine.keyEvent(entry, dismiss);
      require(!state->voice_failure_visible_ && !state->wave_overlay_.failure &&
                  ic.inputPanel().auxUp().toString() != notice,
              "Esc dismisses a voice failure notice");
      state->hideVoiceOverlay();
      ic.inputPanel().setAuxUp(fcitx::Text());
    }
    // 画出来的浮层承载失败提示时不再往辅助行写一份副本，与 macOS 一样；Esc 收起浮层后辅助行原样不动。
    {
      struct DrawnSurface final : msime::linux_host::WaveOverlaySurface {
        bool show(const msime::linux_host::WaveOverlayModel &) override { return true; }
        void update(const msime::linux_host::WaveOverlayModel &) override {}
        void hide() override { ++hides; }
        bool animates() const override { return true; }
        int hides = 0;
      };
      auto drawn = std::make_unique<DrawnSurface>();
      auto *surface = drawn.get();
      state->wave_overlay_surface_ = std::move(drawn);
      state->wave_overlay_visible_ = false;
      state->wave_overlay_dismissed_ = false;
      ic.inputPanel().setAuxUp(fcitx::Text());
      state->showVoiceFailure("未识别到文字，请重新录音");
      require(state->voice_failure_visible_ && state->wave_overlay_visible_ && state->wave_overlay_.failure,
              "a drawn surface shows the voice failure notice");
      require(ic.inputPanel().auxUp().toString().empty(),
              "a drawn failure notice leaves no copy on the auxiliary line");
      fcitx::KeyEvent dismiss(&ic, fcitx::Key(FcitxKey_Escape));
      engine.keyEvent(entry, dismiss);
      require(!state->voice_failure_visible_ && !state->wave_overlay_visible_ && surface->hides >= 1 &&
                  ic.inputPanel().auxUp().toString().empty(),
              "Esc takes a drawn failure notice down and leaves the auxiliary line empty");
    }
    state->wave_overlay_surface_ = std::move(failureSurface);
    state->wave_overlay_visible_ = false;
    require(key(FcitxKey_n), "restart composition");
    ic.setCapabilityFlags(fcitx::CapabilityFlag::Password);
    require(state->session_ == 0, "password capability immediately closes session");
    state->cloud_clipboard_items_ = Json::array({Json{{"id", "synthetic-3"}, {"text", "云剪贴板受限"}}});
    require(engine.cloud_clipboard_item1_.shortText(&ic) == "云剪贴板 1",
            "password context does not preview cloud clipboard text");
    const auto beforeRestrictedCloud = ic.committed;
    engine.cloud_clipboard_item1_.activate(&ic);
    require(ic.committed == beforeRestrictedCloud, "password context does not commit cloud clipboard text");
    state->cloud_clipboard_items_ = Json::array();
    engine.english_action_.activate(&ic);
    require(state->session_ == 0, "status action cannot reopen password context");
    require(ic.inputPanel().clientPreedit().empty(), "password immediately clears preedit");
    require(!key(FcitxKey_i) && state->session_ == 0, "password context closes session");
    require(ic.inputPanel().clientPreedit().empty(), "password clears preedit");
    ic.setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
    require(key(FcitxKey_n), "normal input resumes after restricted context");
    ic.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit,
                                               fcitx::CapabilityFlag::Sensitive});
    require(state->session_ == 0, "privacy transition closes old session immediately");
    require(ic.inputPanel().clientPreedit().empty(), "privacy transition clears previous composition");
    require(key(FcitxKey_n), "private context can compose");
    require(!state->preferences_.at("learning").get<bool>() &&
            !state->preferences_.at("cloud_candidates").get<bool>() &&
            !state->preferences_.at("ai_assistant").at("enabled").get<bool>(),
            "private context disables learning and remote candidates");
    ic.setCapabilityFlags(fcitx::CapabilityFlag::NoFlag);
    require(state->session_ == 0, "leaving private context invalidates session");
    require(key(FcitxKey_n), "panel preedit composition");
    require(ic.inputPanel().preedit().toString() == "n", "server preedit without client support");
    ic.setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
    require(ic.inputPanel().preedit().empty() &&
            ic.inputPanel().clientPreedit().toString() == "n", "capability moves active preedit to client");
    state->close();
    state->clearPanel();
    auto capsEvent = fcitx::KeyEvent(&ic,
        fcitx::Key(FcitxKey_A, fcitx::KeyStates{fcitx::KeyState::CapsLock}));
    engine.keyEvent(entry, capsEvent);
    require(!capsEvent.accepted(), "CapsLock uppercase passes through idle editor");
    require(state->view_.value("editing_text", std::string{}).empty(),
            "CapsLock does not begin composition");
    state->close();
    state->clearPanel();
    // Screen keyboard keys go through the context's input method before the editor; the daemon test covers the MSIME composition this leads to. This fixture's instance has no input method of its own (the engine above is driven directly), so here nothing consumes the key: it has to reach the editor as one whole stroke, with the evdev code turned into an X keycode. Panel text still commits as it is.
    {
      using msime::linux_host::PanelInputDelivery;
      using msime::linux_host::PanelInputRequest;
      PanelInputRequest panelKey;
      panelKey.kind = PanelInputRequest::Kind::Key;
      panelKey.key = "BackSpace";
      panelKey.keycode = 14;
      ic.forwarded.clear();
      require(engine.deliverPanelInput(panelKey) == PanelInputDelivery::Delivered, "panel key delivered");
      require(ic.forwarded.size() == 2 && !ic.forwarded[0].second && ic.forwarded[1].second &&
                  ic.forwarded[0].first.sym() == FcitxKey_BackSpace &&
                  ic.forwarded[0].first.code() == 22 && ic.forwarded[1].first == ic.forwarded[0].first,
              "unconsumed panel key reaches the editor as one stroke");
      PanelInputRequest panelText;
      panelText.kind = PanelInputRequest::Kind::Text;
      panelText.text = "好";
      const auto beforePanelText = ic.committed;
      require(engine.deliverPanelInput(panelText) == PanelInputDelivery::Delivered &&
                  ic.committed == beforePanelText + "好" && ic.forwarded.size() == 2,
              "panel text commits as it is");
      panelText.text = "0";
      const auto beforePanelDigit = ic.committed;
      require(engine.deliverPanelInput(panelText) == PanelInputDelivery::Delivered &&
                  ic.committed == beforePanelDigit + "0",
              "panel ASCII text commits as it is");
      ic.surroundingText().invalidate();
      fcitx::KeyEvent panelPeriod(&ic, fcitx::Key(FcitxKey_period));
      engine.keyEvent(entry, panelPeriod);
      require(!panelPeriod.accepted() && ic.committed == beforePanelDigit + "0",
              "panel ASCII text did not provide smart-punctuation shadow context");
    }
    // Real translation socket: no HTTP, credentials, or user input in this fixture.
    const auto translationPath = std::string(directory) + "/translation.sock";
    const int translationServer = socket(AF_UNIX, SOCK_STREAM, 0);
    require(translationServer >= 0, "translation socket");
    sockaddr_un translationAddress{};
    translationAddress.sun_family = AF_UNIX;
    std::strncpy(translationAddress.sun_path, translationPath.c_str(), sizeof(translationAddress.sun_path) - 1);
    require(bind(translationServer, reinterpret_cast<sockaddr *>(&translationAddress), sizeof(translationAddress)) == 0 &&
            listen(translationServer, 1) == 0, "translation listener");
    auto translationProvider = std::async(std::launch::async, [translationServer] {
      const auto failed = [](const char *stage) {
        std::cerr << "合成翻译服务失败阶段：" << stage << '\n';
        return false;
      };
      struct Descriptor { int fd; ~Descriptor() { if (fd >= 0) close(fd); } } server{translationServer};
      pollfd ready{server.fd, POLLIN, 0};
      if (poll(&ready, 1, 5000) <= 0) return failed("等待连接");
      Descriptor client{accept(server.fd, nullptr, nullptr)};
      if (client.fd < 0) return failed("接受连接");
      std::string request;
      const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
      while (request.size() < 16384 && request.find('\n') == std::string::npos &&
             std::chrono::steady_clock::now() < deadline) {
        pollfd readable{client.fd, POLLIN, 0};
        if (poll(&readable, 1, 100) <= 0) continue;
        char buffer[1024];
        const auto count = read(client.fd, buffer, sizeof(buffer));
        if (count <= 0) return failed("读取请求");
        request.append(buffer, count);
      }
      const auto document = Json::parse(request);
      if (document.at("kind") != "translation" || document.at("query").at("target_language") != "en") return failed("请求类型或目标语言");
      const auto &texts = document.at("query").at("candidates");
      if (texts.empty() || !texts.at(0).is_string()) return failed("候选格式");
      const auto reply = Json{{"translations", Json::array({
          Json{{"text", texts.at(0)}, {"translation", "synthetic-gloss"}}})}}.dump() + "\n";
      return send(client.fd, reply.data(), reply.size(), MSG_NOSIGNAL) == static_cast<ssize_t>(reply.size());
    });
    options["translation_provider_socket"] = translationPath;
    settingsPageSaves([](Json &preferences) {
      preferences["candidate_translations"] = true;
      preferences["candidate_english_gloss"] = false;
      preferences["translation_target_language"] = "en";
    });
    // 前面的 CapsLock 透传已重建会话；新套接字写入配置后重新加载，不能沿用旧地址。
    state->close();
    state->clearPanel();
    require(key(FcitxKey_n) && key(FcitxKey_i), "translation composition");
    require(state->translation_socket_ == translationPath, "翻译夹具必须加载新配置的套接字");
    // 模拟定时器重复收到同一份云候选或 AI 候选，已显示的译文不能被清掉。
    const uint8_t repeatedSource = ai ? 1 : 0;
    const auto deliverRepeatedOnline = [&] {
      const auto query = response(msime_client_online_query(state->session_)).dump();
      state->online_query_ = query;
      state->online_due_ = state->ai_due_ = std::chrono::steady_clock::now();
      state->refreshOnline();
      require(state->online_dispatched_queries_[repeatedSource] == query,
              "合成回包必须对应已分发的网络请求");
      state->online_due_ = state->ai_due_ =
          std::chrono::steady_clock::now() + std::chrono::hours(1);
      state->online_job_session_ = state->session_;
      auto &slot = state->online_slots_[repeatedSource];
      slot.query = query;
      slot.epoch = state->online_epoch_;
      std::promise<Json> reply;
      slot.job = reply.get_future().share();
      reply.set_value(Json{{"query", query}, {"candidates", Json::array({
          Json{{"text", "合成候选"}, {"source", repeatedSource}}})}});
      state->refreshOnline();
    };
    state->refreshTranslations();
    require(!state->translation_job_.valid(), "translation waits for idle debounce");
    const auto translationDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
    while (state->view_.at("candidates").dump().find("synthetic-gloss") == std::string::npos &&
           std::chrono::steady_clock::now() < translationDeadline) {
      state->refreshTranslations();
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    require(translationProvider.get(), "translation socket protocol");
    require(ic.inputPanel().candidateList()->candidate(0).text().toString().find("synthetic-gloss") != std::string::npos,
            "translation visible in native Fcitx candidate");
    const auto localTranslationGeneration = state->view_.at("generation").get<uint64_t>();
    deliverRepeatedOnline();
    require(ic.inputPanel().candidateList()->candidate(0).text().toString().find("synthetic-gloss") != std::string::npos,
            "新在线候选到达使未变化候选的译文消失");
    require(state->view_.at("generation").get<uint64_t>() > localTranslationGeneration,
            "保留译文不能保留旧候选代次");
    const auto staleGloss = Json::array({Json{{"text", state->view_.at("candidates").at(0).at("text")},
                                            {"translation", "stale-gloss"}}}).dump();
    require(!response(msime_client_apply_translations(
                state->session_, localTranslationGeneration,
                reinterpret_cast<const uint8_t *>(staleGloss.data()), staleGloss.size())).at("applied").get<bool>(),
            "新在线候选到达后仍拒绝旧代次的翻译回包");
    const auto glossPath = std::filesystem::path(options.at("user_data").get<std::string>()) /
                           "translation-glosses.db";
    require(std::filesystem::exists(glossPath), "English translation gloss database exists");
    std::ifstream glossFile(glossPath);
    const std::string glossContent((std::istreambuf_iterator<char>(glossFile)),
                                   std::istreambuf_iterator<char>());
    require(glossContent.find("synthetic-gloss") != std::string::npos,
            "English translation gloss is persisted");
    // 离线探测失败后转在线，空回包也不能清掉当前页保留下来的释义。
    const auto glossResources = state->resources_;
    const std::filesystem::path backupPath = glossPath.string() + ".seed-test-backup";
    std::error_code restorationError;
    const auto checkMissingOfflineGloss = [&](bool interrupt) {
      auto resources = state->resources_;
      std::filesystem::rename(glossPath, backupPath);
      struct RestoreOfflineGloss {
        FcitxState &state;
        std::string resources;
        const std::filesystem::path &backup;
        const std::filesystem::path &original;
        std::error_code &error;
        ~RestoreOfflineGloss() {
          state.resources_ = std::move(resources);
          std::filesystem::rename(backup, original, error);
        }
      } restore{*state, std::move(resources), backupPath, glossPath, restorationError};
      state->resources_ = std::string(directory) + "/missing-gloss-resources";
      if (interrupt) throw std::logic_error("合成夹具中途失败");
      state->startTranslation(response(msime_client_translation_query(state->session_)), true);
      while (state->translation_job_.valid()) {
        require(state->translation_job_.wait_for(std::chrono::seconds(5)) == std::future_status::ready,
                "离线与在线译文任务应结束");
        state->refreshTranslations();
        require(ic.inputPanel().candidateList()->candidate(0).text().toString().find("synthetic-gloss") != std::string::npos,
                "离线探测失败或在线空回包使保留的译文消失");
      }
    };
    bool interrupted = false;
    try {
      checkMissingOfflineGloss(true);
    } catch (const std::logic_error &) {
      interrupted = true;
    }
    require(interrupted, "夹具应触发合成中途失败");
    require(!restorationError && state->resources_ == glossResources && std::filesystem::exists(glossPath),
            "中途失败后必须恢复资源目录和释义数据库");
    checkMissingOfflineGloss(false);
    require(!restorationError && state->resources_ == glossResources && std::filesystem::exists(glossPath),
            "正常退出也必须恢复资源目录和释义数据库");
    const auto retainedQuery = response(msime_client_translation_query(state->session_));
    state->startTranslation(retainedQuery, true);
    std::promise<Json> emptyTranslation;
    state->translation_job_ = emptyTranslation.get_future().share();
    emptyTranslation.set_value(Json{{"query", retainedQuery.dump()},
                                   {"translations", Json::array()},
                                   {"continue_online", false},
                                   {"_socket", state->translation_socket_}});
    state->refreshTranslations();
    require(ic.inputPanel().candidateList()->candidate(0).text().toString().find("synthetic-gloss") != std::string::npos,
            "异步空译文回包不能触发主动清理");
    // 「显示读音」：工作线程回答里的 "pronunciations" 经 refreshTranslations 记下，候选按它取读音；开关关着时回答里的读音不记。
    {
      const Json glossed{{"text", "爱"}, {"translation", "love"}};
      const auto savedPreferences = state->preferences_;
      const auto deliverPronunciations = [&](bool pronounce) {
        const auto query = response(msime_client_translation_query(state->session_));
        state->startTranslation(query, true);
        state->preferences_["candidate_pronunciation"] = pronounce;
        std::promise<Json> reply;
        state->translation_job_ = reply.get_future().share();
        reply.set_value(Json{{"query", query.dump()},
                             {"translations", Json::array()},
                             {"continue_online", false},
                             {"pronunciations", Json{{"love", "/l\xCA\x8Cv/"}}},
                             {"_socket", state->translation_socket_}});
        state->refreshTranslations();
      };
      state->pronunciations_.clear();
      deliverPronunciations(false);
      state->preferences_["candidate_pronunciation"] = true;
      require(state->candidatePronunciation(glossed).empty(), "pronunciations in a reply are not kept while the switch is off");
      deliverPronunciations(true);
      require(state->candidatePronunciation(glossed) == "/l\xCA\x8Cv/", "the worker reply's pronunciations reach the candidate");
      state->preferences_ = savedPreferences;
      state->pronunciations_.clear();
    }
    const auto translatedGeneration = state->view_.at("generation");
    for (int repeat = 0; repeat < 3; ++repeat) {
      state->online_due_ = state->ai_due_ = std::chrono::steady_clock::now();
      state->refreshOnline();
      // 已完成的查询若被再次发起，回放同一份成功回包以捕获译文消失。
      if (state->online_slots_[repeatedSource].job.valid()) deliverRepeatedOnline();
      require(ic.inputPanel().candidateList()->candidate(0).text().toString().find("synthetic-gloss") != std::string::npos,
              "重复在线候选刷新使已显示的译文消失");
      require(state->view_.at("generation") == translatedGeneration,
              "相同在线候选不应推进候选代次");
      require(!state->online_slots_[repeatedSource].job.valid(),
              "已显示的在线候选不应重复请求");
    }
    const auto translatedText = state->view_.at("candidates").at(0).at("text").get<std::string>();
    const auto beforeTranslatedCommit = ic.committed;
    ic.inputPanel().candidateList()->candidate(0).select(&ic);
    require(ic.committed == beforeTranslatedCommit + translatedText, "gloss excluded from committed text");
    require(key(FcitxKey_n) && key(FcitxKey_i), "新的组合仍能发起在线请求");
    state->refreshOnline();
    if (ai) {
      require(state->online_slots_[1].job.wait_for(std::chrono::seconds(5)) == std::future_status::ready,
              "新组合的 AI 缓存探测应结束");
      state->refreshOnline();
    }
    state->online_due_ = state->ai_due_ = std::chrono::steady_clock::now();
    state->refreshOnline();
    require(state->online_dispatched_queries_[repeatedSource] ==
                response(msime_client_online_query(state->session_)).dump(),
            "消费缓存探测后，新组合必须实际分发网络请求");
    state->close();
    state->clearPanel();
    // The helpcode annotation on a candidate row follows the scheme's show_in_candidate_window preference, the way the IBus host renders it. This host used to append it whatever the setting said. In / and @ the annotation is the command title or the place's province and city, so neither that preference nor the wubi code hint hides it; Wubi opens / and @ too, and with 五笔剩余编码 off its command rows used to lose their titles. The other local modes still carry helpcodes and stay behind the preference.
    {
      require(state->ensure(), "session for the annotation check");
      auto withHelpcode = options;
      withHelpcode["preferences"]["quanpin_helpcode"]["enabled"] = true;
      withHelpcode["preferences"]["quanpin_helpcode"]["show_in_candidate_window"] = true;
      state->preferences_ = withHelpcode.at("preferences");
      require(state->showCandidateAnnotations(),
              "quanpin annotation shown when the preference asks for it");
      state->preferences_["quanpin_helpcode"]["show_in_candidate_window"] = false;
      require(!state->showCandidateAnnotations(),
              "quanpin annotation hidden when the preference turns it off");
      const auto savedView = state->view_;
      state->view_["local_mode"] = "mention";
      require(state->showCandidateAnnotations(),
              "a mention row keeps its place annotation with the helpcode switch off");
      state->view_["local_mode"] = "super_jianpin";
      require(!state->showCandidateAnnotations(),
              "super jianpin helpcodes stay hidden with the helpcode switch off");
      state->view_["local_mode"] = "quick_phrase";
      require(!state->showCandidateAnnotations(),
              "quick phrase helpcodes stay hidden with the helpcode switch off");
      state->view_["scheme"] = 2;
      state->view_["local_mode"] = "none";
      state->preferences_["wubi_code_hint"] = false;
      require(!state->showCandidateAnnotations(),
              "wubi code hint off hides the remaining-code annotation");
      state->view_["local_mode"] = "command";
      require(state->showCandidateAnnotations(),
              "a wubi command row keeps its title with the wubi code hint off");
      state->view_ = savedView;
      state->close();
      state->clearPanel();
    }
    // 正在组词时内嵌预编辑分成两个子句，对应 macOS 的细、粗两种下划线：已选的一段只加下划线，还在输入的读音加下划线并高亮（Fcitx5 没有粗下划线，`Bold` 是字形加粗）；没有已选段时整段只是一条下划线。
    {
      require(state->ensure(), "session for the phrase preedit check");
      const auto savedView = state->view_;
      const auto savedPreferences = state->preferences_;
      state->preferences_["tsf_preedit_style"] = "raw";
      state->view_["phrase_prefix"] = "你好";
      state->view_["editing_text"] = "shi";
      state->view_["preedit"] = "shi";
      state->view_["reading"] = "";
      state->view_["caret_position"] = 3;
      state->view_["local_mode"] = "none";
      state->view_["candidates"] = Json::array();
      state->render();
      const fcitx::TextFormatFlags reading{fcitx::TextFormatFlag::Underline, fcitx::TextFormatFlag::HighLight};
      const auto segmented = ic.inputPanel().clientPreedit();
      require(segmented.toString() == "你好shi" && segmented.size() == 2 && segmented.stringAt(0) == "你好" &&
                  segmented.formatAt(0) == fcitx::TextFormatFlags(fcitx::TextFormatFlag::Underline) &&
                  segmented.stringAt(1) == "shi" && segmented.formatAt(1) == reading &&
                  segmented.cursor() == static_cast<int>(std::string("你好shi").size()),
              "a phrase in progress draws the chosen piece and the reading as two clauses");
      state->view_["phrase_prefix"] = "";
      state->render();
      const auto plain = ic.inputPanel().clientPreedit();
      require(plain.toString() == "shi" && plain.size() == 1 &&
                  plain.formatAt(0) == fcitx::TextFormatFlags(fcitx::TextFormatFlag::Underline),
              "without a chosen piece the composition keeps one underline");
      state->view_ = savedView;
      state->preferences_ = savedPreferences;
      state->close();
      state->clearPanel();
    }
    // Cycling through the schemes has to leave a way back to Chinese: the shared settings page and the IBus host both offer "中文", and it returns to last_chinese_scheme. Leaving for Japanese or Korean must not overwrite it.
    {
      const auto savedScheme = [&](const char *key) {
        const auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()),
            preferenceDirectory.size()));
        return snapshot.at("preferences").value(key, std::string("quanpin"));
      };
      // 方案切换的持久化是异步的，立即回读拿到的往往还是上一个值。断言的是最终状态，
      // 所以这里等它落盘，而不是赌一次读取的时机。
      const auto savedSchemeBecomes = [&](const char *key, const char *expected) {
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
        while (std::chrono::steady_clock::now() < deadline) {
          if (savedScheme(key) == expected) return true;
          std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        return false;
      };
      require(state->ensure(), "session for the scheme cycle");
      while (state->view_.value("scheme", 0u) != 0) require(state->cycleScheme(), "reach quanpin");
      require(state->cycleScheme() && savedSchemeBecomes("scheme", "shuangpin") &&
                  savedSchemeBecomes("last_chinese_scheme", "shuangpin"),
              "shuangpin recorded as the last Chinese scheme");
      require(state->cycleScheme() && savedSchemeBecomes("scheme", "wubi") &&
                  savedSchemeBecomes("last_chinese_scheme", "wubi"),
              "wubi recorded as the last Chinese scheme");
      require(state->cycleScheme() && savedSchemeBecomes("scheme", "japanese") &&
                  savedSchemeBecomes("last_chinese_scheme", "wubi"),
              "Japanese leaves the last Chinese scheme alone");
      require(state->cycleScheme() && state->view_.value("scheme", 0u) == 4 &&
                  savedSchemeBecomes("scheme", "korean") &&
                  savedSchemeBecomes("last_chinese_scheme", "wubi"),
              "Korean leaves the last Chinese scheme alone");
      require(state->modeIndicatorLabel() == "한", "the status area labels Korean input");
      // The 输入方案 menu picks a scheme directly and marks the one in use.
      // Cantonese, Zhuyin and Stroke are listed only with their dictionaries, which this fixture does not install.
      require(engine.scheme_menu_.actions().size() == 7, "scheme menu lists the seven schemes that need no dictionary");
      require(engine.scheme_korean_action_.isChecked(&ic) && !engine.scheme_japanese_action_.isChecked(&ic) &&
                  !engine.scheme_quanpin_action_.isChecked(&ic),
              "scheme menu marks the scheme in use");
      engine.scheme_japanese_action_.activate(&ic);
      require(state->view_.value("scheme", 0u) == 3 && savedSchemeBecomes("scheme", "japanese") &&
                  engine.scheme_japanese_action_.isChecked(&ic) && !engine.scheme_korean_action_.isChecked(&ic),
              "scheme menu selects Japanese directly");
      engine.scheme_shuangpin_action_.activate(&ic);
      require(state->view_.value("scheme", 0u) == 1 && savedSchemeBecomes("scheme", "shuangpin") &&
                  savedSchemeBecomes("last_chinese_scheme", "shuangpin"),
              "scheme menu selects shuangpin directly");
      require(engine.scheme_shuangpin_action_.isChecked(&ic) && !engine.scheme_japanese_action_.isChecked(&ic) &&
                  !engine.scheme_korean_action_.isChecked(&ic),
              "scheme menu follows the choice");
      engine.scheme_quanpin_action_.activate(&ic);
      require(state->view_.value("scheme", 0u) == 0 && savedSchemeBecomes("scheme", "quanpin"),
              "scheme menu returns to quanpin");
      state->close();
      state->clearPanel();
    }
    // A status-bar choice outranked the store for the rest of the context's life, so a scheme picked later in the settings page never reached that window, and a helpcode schema picked under quanpin followed the user into shuangpin.
    {
      const auto loadStore = [&] {
        return response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
      };
      // What the settings page does: write the store, then mirror it into the runtime options file. Another window's status bar writes the store alone.
      const auto settingsPageSetsScheme = [&](const char *scheme, bool mirror = true) {
        auto snapshot = loadStore();
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["scheme"] = scheme;
        snapshot["revision"] = revision + 1;
        const auto document = snapshot.dump();
        const auto saved = response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        require(saved.value("revision", uint64_t{}) > revision, "settings page scheme saved");
        if (!mirror) return;
        options["preferences"]["scheme"] = scheme;
        std::ofstream(path) << options.dump();
      };
      require(state->ensure(), "session for the override expiry");
      while (state->view_.value("scheme", 0u) != 1) require(state->cycleScheme(), "reach shuangpin");
      require(state->scheme_override_ == std::optional<std::string>("shuangpin"),
              "status bar keeps its scheme choice for this context");
      settingsPageSetsScheme("wubi");
      const auto reloadDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
      while (state->view_.value("scheme", 0u) != 2 && std::chrono::steady_clock::now() < reloadDeadline) {
        state->refreshPreferences();
        std::this_thread::sleep_for(std::chrono::milliseconds(5));
      }
      require(state->view_.value("scheme", 0u) == 2 && !state->scheme_override_,
              "a later settings page scheme replaces the status bar choice on reload");
      // The same change made while the context had no session, as when the settings window has the focus.
      while (state->view_.value("scheme", 0u) != 1) require(state->cycleScheme(), "status bar back to shuangpin");
      state->close();
      state->clearPanel();
      settingsPageSetsScheme("wubi");
      require(state->ensure() && state->view_.value("scheme", 0u) == 2 && !state->scheme_override_,
              "a settings page scheme chosen while unfocused wins on the next session");
      // Another window's status bar reaches only the store, so the runtime options file still says wubi; a new session takes the store's quanpin rather than a value no window has chosen since.
      state->close();
      state->clearPanel();
      settingsPageSetsScheme("quanpin", false);
      require(state->ensure() && state->view_.value("scheme", 0u) == 0,
              "a store scheme the runtime options file never saw wins on the next session");
      // A status-bar save that fails must not roll the menu back: not in the rebuild the cycle does itself, not on the next focus, and not once a later save takes over the single retry slot. At its last revision the store still loads but refuses every save, even for root in the gate container.
      const auto storeFile = std::filesystem::path(preferenceDirectory) / "preferences.json";
      std::string storeBytes;
      {
        std::ifstream in(storeFile, std::ios::binary);
        storeBytes.assign(std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>());
      }
      require(!storeBytes.empty(), "store file to freeze");
      auto frozen = loadStore();
      frozen["revision"] = std::numeric_limits<uint64_t>::max();
      std::ofstream(storeFile, std::ios::binary | std::ios::trunc) << frozen.dump();
      require(state->cycleScheme(), "status bar to shuangpin with a failing save");
      require(state->preferences_save_retry_ && state->preferences_save_retry_->key == "scheme" &&
                  loadStore().at("preferences").value("scheme", std::string()) == "quanpin",
              "the status bar scheme save failed and awaits a retry");
      require(state->view_.value("scheme", 0u) == 1 &&
                  state->scheme_override_ == std::optional<std::string>("shuangpin"),
              "a failed status bar save survives the cycle's own rebuild");
      state->close();
      state->clearPanel();
      require(state->ensure() && state->view_.value("scheme", 0u) == 1 &&
                  state->scheme_override_ == std::optional<std::string>("shuangpin") &&
                  state->preferences_save_retry_ && state->preferences_save_retry_->key == "scheme",
              "a failed status bar save and its retry survive a focus change");
      std::ofstream(storeFile, std::ios::binary | std::ios::trunc) << storeBytes;
      require(state->cycleShuangpinProfile() && !state->preferences_save_retry_ &&
                  loadStore().at("preferences").value("scheme", std::string()) == "quanpin",
              "the shuangpin profile save lands and takes over the retry slot");
      require(state->view_.value("scheme", 0u) == 1 &&
                  state->scheme_override_ == std::optional<std::string>("shuangpin"),
              "the unsaved scheme choice survives a later status bar save");
      state->scheme_override_.reset();
      state->shuangpin_profile_override_.reset();
      state->scheme_unsaved_ = false;
      state->shuangpin_profile_unsaved_ = false;
      state->close();
      state->clearPanel();
      require(state->ensure(), "session for the helpcode leak check");
      while (state->view_.value("scheme", 0u) != 0) require(state->cycleScheme(), "reach quanpin");
      for (int step = 0; step < 6 && state->preferences_.value("quanpin_helpcode", Json::object())
                                             .value("schema", std::string()) != "xiaohe"; ++step)
        require(state->cycleHelpcodeSchema(), "cycle the quanpin helpcode schema");
      require(state->preferences_.value("quanpin_helpcode", Json::object()).value("schema", std::string()) == "xiaohe",
              "quanpin helpcode schema set to xiaohe from the status bar");
      // 辅助码方案 opens a menu of the built-in schemas; the one in use is the only one checked, and choosing another saves it.
      const auto helpcodeSchemaItem = [&](std::string_view schema) -> fcitx::Action & {
        for (auto &item : engine.helpcode_schema_items_)
          if (item->shortText(&ic) == msime::linux_host::helpcode_schema_label(schema)) return *item;
        throw std::runtime_error("no helpcode schema menu entry");
      };
      require(engine.helpcode_schema_action_.menu() == &engine.helpcode_schema_menu_ &&
                  engine.helpcode_schema_menu_.actions().size() == msime::linux_host::kHelpcodeSchemaNames.size(),
              "the helpcode schema menu lists every built-in schema");
      require(std::count_if(engine.helpcode_schema_items_.begin(), engine.helpcode_schema_items_.end(),
                            [&](const auto &item) { return item->isChecked(&ic); }) == 1 &&
                  helpcodeSchemaItem("xiaohe").isChecked(&ic),
              "the helpcode schema menu checks the schema in use");
      helpcodeSchemaItem("jiajia").activate(&ic);
      require(state->preferences_.value("quanpin_helpcode", Json::object()).value("schema", std::string()) == "jiajia" &&
                  loadStore().at("preferences").value("quanpin_helpcode", Json::object()).value("schema", std::string()) == "jiajia" &&
                  helpcodeSchemaItem("jiajia").isChecked(&ic) && !helpcodeSchemaItem("xiaohe").isChecked(&ic) &&
                  engine.helpcode_schema_action_.shortText(&ic) == "辅助码：加加",
              "choosing a helpcode schema from the menu saves it");
      helpcodeSchemaItem("xiaohe").activate(&ic);
      require(state->preferences_.value("quanpin_helpcode", Json::object()).value("schema", std::string()) == "xiaohe",
              "the helpcode schema menu returns to xiaohe");
      require(state->cycleScheme() && state->view_.value("scheme", 0u) == 1, "status bar to shuangpin");
      const auto storedShuangpin = loadStore().at("preferences").value("shuangpin_helpcode", Json::object())
                                       .value("schema", std::string("lantian"));
      require(storedShuangpin == "lantian" && !state->helpcode_schema_override_ &&
                  state->preferences_.value("shuangpin_helpcode", Json::object())
                          .value("schema", std::string("lantian")) == storedShuangpin,
              "shuangpin keeps the store's helpcode schema, not quanpin's status bar choice");
      // A helpcode pack picked on the settings page outranks the schema, so the status bar names the pack, and its first click returns to the stored built-in schema by clearing the pack in the store, as the settings page does. The store stays the authority for the pack in new sessions, a pack picked later replaces the status bar schema, and a failed save of that first click keeps the choice until its retry lands.
      const auto settingsPageSetsPack = [&](const std::string &pack, bool mirror) {
        auto snapshot = loadStore();
        const auto revision = snapshot.at("revision").get<uint64_t>();
        msime::linux_host::set_helpcode_pack(snapshot["preferences"], "quanpin", pack);
        snapshot["revision"] = revision + 1;
        const auto document = snapshot.dump();
        const auto saved = response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        require(saved.value("revision", uint64_t{}) > revision, "settings page helpcode pack saved");
        if (!mirror) return;
        msime::linux_host::set_helpcode_pack(options["preferences"], "quanpin", pack);
        std::ofstream(path) << options.dump();
      };
      const auto storedPack = [&] { return msime::linux_host::helpcode_pack(loadStore().at("preferences"), "quanpin"); };
      const auto storedHelpcodeSchema = [&] {
        return loadStore().at("preferences").value("quanpin_helpcode", Json::object()).value("schema", std::string());
      };
      const auto sessionPack = [&] { return msime::linux_host::helpcode_pack(state->preferences_, "quanpin"); };
      const auto sessionHelpcodeSchema = [&] {
        return state->preferences_.value("quanpin_helpcode", Json::object()).value("schema", std::string());
      };
      const auto schemaLabel = [](const std::string &schema) {
        return std::string("辅助码：") + std::string(msime::linux_host::helpcode_schema_label(schema));
      };
      state->scheme_override_.reset();
      state->scheme_unsaved_ = false;
      state->close();
      state->clearPanel();
      settingsPageSetsScheme("quanpin");
      settingsPageSetsPack("radicals", true);
      require(state->ensure() && state->view_.value("scheme", 0u) == 0 && sessionPack() == "radicals" &&
                  engine.helpcode_schema_action_.shortText(&ic) == "辅助码：插件 radicals",
              "the status bar names the helpcode pack in use");
      const auto packedSchema = storedHelpcodeSchema();
      require(!packedSchema.empty(), "the store keeps a helpcode schema under the pack");
      require(std::none_of(engine.helpcode_schema_items_.begin(), engine.helpcode_schema_items_.end(),
                           [&](const auto &item) { return item->isChecked(&ic); }),
              "the helpcode schema menu checks no built-in schema while a pack is in use");
      // Choosing the schema the store already holds is not a no-op while the pack is active: it turns the pack off.
      helpcodeSchemaItem(packedSchema).activate(&ic);
      require(!state->preferences_save_retry_ && storedPack().empty() && storedHelpcodeSchema() == packedSchema,
              "choosing the stored schema from the menu clears the pack in the store and keeps its schema");
      require(sessionPack().empty() && sessionHelpcodeSchema() == packedSchema &&
                  engine.helpcode_schema_action_.shortText(&ic) == schemaLabel(packedSchema),
              "the session returns to the stored built-in schema");
      // The runtime options file still names the pack, which only the settings page rewrites.
      state->helpcode_schema_override_.reset();
      state->close();
      state->clearPanel();
      require(state->ensure() && sessionPack().empty() && sessionHelpcodeSchema() == packedSchema,
              "a new session takes the pack from the store, not the runtime options file");
      require(state->cycleHelpcodeSchema() && !state->preferences_save_retry_ && state->helpcode_schema_override_ &&
                  *state->helpcode_schema_override_ != packedSchema && storedHelpcodeSchema() == *state->helpcode_schema_override_,
              "the next status bar click moves on to the next built-in schema");
      settingsPageSetsPack("radicals", false);
      const auto packDeadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
      while (state->helpcode_schema_override_ && std::chrono::steady_clock::now() < packDeadline) {
        state->refreshPreferences();
        std::this_thread::sleep_for(std::chrono::milliseconds(5));
      }
      require(!state->helpcode_schema_override_ && sessionPack() == "radicals" &&
                  engine.helpcode_schema_action_.shortText(&ic) == "辅助码：插件 radicals",
              "a pack picked later on the settings page replaces the status bar schema on reload");
      // The first click while a pack is active saves the schema the store already holds; a failed save must not read as landed because of that.
      const auto pendingSchema = storedHelpcodeSchema();
      {
        std::ifstream in(storeFile, std::ios::binary);
        storeBytes.assign(std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>());
      }
      require(!storeBytes.empty(), "store file to freeze under the pack");
      auto frozenPack = loadStore();
      frozenPack["revision"] = std::numeric_limits<uint64_t>::max();
      std::ofstream(storeFile, std::ios::binary | std::ios::trunc) << frozenPack.dump();
      require(state->cycleHelpcodeSchema(), "status bar helpcode click with a failing save");
      require(state->preferences_save_retry_ && state->preferences_save_retry_->key == "schema" &&
                  state->preferences_save_retry_->helpcode_schema_choice && storedPack() == "radicals",
              "the status bar helpcode save failed and awaits a retry");
      require(state->helpcode_schema_override_ == std::optional<std::string>(pendingSchema) && sessionPack().empty() &&
                  engine.helpcode_schema_action_.shortText(&ic) == schemaLabel(pendingSchema),
              "a failed helpcode save survives the cycle's own rebuild");
      state->close();
      state->clearPanel();
      require(state->ensure() && state->helpcode_schema_override_ == std::optional<std::string>(pendingSchema) &&
                  sessionPack().empty() && state->preferences_save_retry_ &&
                  state->preferences_save_retry_->key == "schema",
              "a failed helpcode save and its retry survive a focus change");
      std::ofstream(storeFile, std::ios::binary | std::ios::trunc) << storeBytes;
      require(state->retryPreferenceSave(), "retry the helpcode save");
      state->waitForPreferenceSave();
      require(!state->preferences_save_retry_ && storedPack().empty() && storedHelpcodeSchema() == pendingSchema,
              "the retried helpcode save clears the pack in the store");
      state->close();
      state->clearPanel();
      require(state->ensure() && state->helpcode_schema_override_ == std::optional<std::string>(pendingSchema) &&
                  !state->helpcode_schema_unsaved_ && sessionPack().empty(),
              "once the store holds the choice it is no longer marked unsaved");
      state->helpcode_schema_override_.reset();
      // The store has no pack left; only the runtime options file still names it.
      msime::linux_host::set_helpcode_pack(options["preferences"], "quanpin", "");
      std::ofstream(path) << options.dump();
      settingsPageSetsScheme("japanese");
      state->scheme_override_.reset();
      state->shuangpin_profile_override_.reset();
      state->close();
      state->clearPanel();
    }
    // 全角 is a saved preference like any other: a session opens at the saved width, a focus change keeps it, and a reload moves the open session without one. The host's English-mode width is read back from the session's view, so the letter below proves the host and the runtime agree.
    {
      const auto loadStore = [&] {
        return response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
      };
      const auto saveStore = [&](const auto &edit) {
        auto snapshot = loadStore();
        const auto revision = snapshot.at("revision").get<uint64_t>();
        edit(snapshot["preferences"]);
        snapshot["revision"] = revision + 1;
        const auto document = snapshot.dump();
        const auto saved = response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(),
            revision, reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        require(saved.value("revision", uint64_t{}) > revision, "store saved");
      };
      const auto setWidth = [&](const char *width, bool mirror) {
        saveStore([&](Json &preferences) { preferences["character_width"] = width; });
        if (!mirror) return;
        options["preferences"]["character_width"] = width;
        std::ofstream(path) << options.dump();
      };
      const auto sessionWidth = [&] { return state->view_.value("character_width", std::string()); };
      // What an English-mode letter puts in the document; a letter handed back to the application reads as itself.
      const auto englishLetter = [&] {
        const auto before = ic.committed;
        state->input_enabled_ = false;
        fcitx::KeyEvent letter(&ic, fcitx::Key(FcitxKey_a));
        engine.keyEvent(entry, letter);
        state->input_enabled_ = true;
        return letter.accepted() ? ic.committed.substr(before.size()) : std::string("a");
      };
      const auto reloadUntil = [&](const auto &ready) {
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        while (!ready() && std::chrono::steady_clock::now() < deadline) {
          state->refreshPreferences();
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
        }
        return ready();
      };
      setWidth("fullwidth", true);
      require(state->ensure() && sessionWidth() == "Fullwidth" && engine.width_action_.isChecked(&ic),
              "a session opens at the saved fullwidth");
      require(englishLetter() == "ａ", "the saved fullwidth widens an English-mode letter");
      blur();
      require(state->session_ != 0 && engine.width_action_.isChecked(&ic),
              "focus out keeps the fullwidth session for the tray");
      ic.focusIn();
      engine.activate(entry, focus);
      require(state->session_ != 0 && sessionWidth() == "Fullwidth", "fullwidth survives a focus change");
      require(englishLetter() == "ａ", "the refocused session still widens a letter");
      const auto reloadedSession = state->session_;
      setWidth("halfwidth", false);
      require(reloadUntil([&] { return sessionWidth() == "Halfwidth"; }) && state->session_ == reloadedSession,
              "a halfwidth store reaches the open session without a focus change");
      require(!engine.width_action_.isChecked(&ic) && englishLetter() == "a",
              "the reloaded halfwidth hands the letter back");
      setWidth("fullwidth", false);
      require(reloadUntil([&] { return sessionWidth() == "Fullwidth"; }), "fullwidth reloads into the open session");
      // Another window's status bar reaches only the store; the runtime options file still says fullwidth from the first step, so write halfwidth there to prove the store wins.
      options["preferences"]["character_width"] = "halfwidth";
      std::ofstream(path) << options.dump();
      state->close();
      state->clearPanel();
      require(state->ensure() && sessionWidth() == "Fullwidth",
              "a store width the runtime options file never saw wins on the next session");
      setWidth("halfwidth", true);
      require(reloadUntil([&] { return sessionWidth() == "Halfwidth"; }), "width fixture restored to halfwidth");
      // The reload tick nearly always has a store read in flight when the status bar toggles the width. That read predates the toggle; applied after the save, it would put the session back to the old width until the next tick read the saved store.
      const auto toggleHolds = [&](const char *expected, const char *stored) {
        state->refreshPreferences();
        require(state->preferences_job_.valid(), "a store read is in flight before the toggle");
        state->preferences_job_.wait();
        require(state->toggleWidth() && sessionWidth() == expected, "the status bar toggles the width");
        for (int tick = 0; tick < 4; ++tick) {
          if (state->preferences_save_job_.valid()) state->preferences_save_job_.wait();
          if (state->preferences_job_.valid()) state->preferences_job_.wait();
          state->refreshPreferences();
          require(sessionWidth() == expected, "a read that predates the toggle does not undo it");
        }
        require(!state->preferences_save_retry_ &&
                    loadStore().at("preferences").value("character_width", std::string()) == stored,
                "the toggled width is saved");
      };
      toggleHolds("Fullwidth", "fullwidth");
      toggleHolds("Halfwidth", "halfwidth");
      // The diagnostic switch applies on the reload too, with the same session and no focus change.
      const auto diagnosticLog = std::filesystem::path(preferenceDirectory) / "diagnostic.log";
      std::filesystem::remove(diagnosticLog);
      const auto diagnosticSession = state->session_;
      saveStore([](Json &preferences) { preferences["diagnostic_log"]["server"] = true; });
      require(reloadUntil([&] {
                msime_linux_diagnostic_write("native_probe");
                return std::filesystem::exists(diagnosticLog);
              }) && state->session_ == diagnosticSession,
              "turning the diagnostic log on reaches the open session");
      saveStore([](Json &preferences) { preferences["diagnostic_log"]["server"] = false; });
      require(reloadUntil([&] {
                const auto before = std::filesystem::file_size(diagnosticLog);
                msime_linux_diagnostic_write("native_probe");
                return std::filesystem::file_size(diagnosticLog) == before;
              }) && state->session_ == diagnosticSession,
              "turning the diagnostic log off stops it without a focus change");
      state->close();
      state->clearPanel();
    }
    options["preferences"]["scheme"] = "japanese";
    std::ofstream(path) << options.dump();
    require(key(FcitxKey_k) && key(FcitxKey_o), "Japanese romaji composition");
    require(state->view_.at("scheme") == 3, "Engine Japanese scheme active");
    require(key(FcitxKey_minus), "Japanese long vowel key");
    require(state->view_.at("editing_text") == "ko-", "minus extends romaji instead of paging");
    require(state->view_.at("reading") == "こー", "Engine resolves Japanese long vowel");
    require(key(FcitxKey_Escape), "cancel Japanese composition");
    state->close();
    state->clearPanel();
    // Korean composes Dubeolsik jamo into a Hangul syllable drawn inline. Starting a new syllable commits the previous one, the keys that end a syllable commit it and still do their own work in the application, and punctuation stays ASCII.
    {
      options["preferences"]["scheme"] = "korean";
      std::ofstream(path) << options.dump();
      // Status-bar choices live in the store and take precedence over runtime options on a new session.
      auto snapshot = response(msime_client_load_preferences(
          reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
      const auto revision = snapshot.at("revision").get<uint64_t>();
      snapshot["preferences"]["scheme"] = "korean";
      snapshot["revision"] = revision + 1;
      const auto document = snapshot.dump();
      const auto saved = response(msime_client_save_preferences(
          reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(), revision,
          reinterpret_cast<const uint8_t *>(document.data()), document.size()));
      require(saved.value("revision", uint64_t{}) > revision, "Korean saved as the scheme");
      const auto press = [&](fcitx::KeySym sym, fcitx::KeyStates states = fcitx::KeyStates()) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym, states));
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const auto preedit = [&] { return ic.inputPanel().clientPreedit().toString(); };
      auto before = ic.committed;
      require(press(FcitxKey_d) && press(FcitxKey_k) && press(FcitxKey_s), "Korean letters compose");
      require(state->view_.at("scheme") == 4 && preedit() == "안" && ic.committed == before,
              ("the syllable is drawn inline while it composes: scheme=" +
               state->view_.at("scheme").dump() + " preedit=" + preedit() +
               " committed=" + std::to_string(ic.committed != before)).c_str());
      require(state->view_.at("candidates").empty(), "Korean offers no candidates before the Hanja key");
      require(ic.inputPanel().clientPreedit().cursor() == static_cast<int>(std::string("안").size()),
              "the caret follows the syllable");
      require(press(FcitxKey_s) && ic.committed == before + "안" && preedit() == "ㄴ",
              "starting a new syllable commits the previous one");
      require(press(FcitxKey_u) && press(FcitxKey_d) && preedit() == "녕" &&
                  state->view_.at("editing_text") == "sud",
              "the open syllable keeps its key letters");
      require(press(FcitxKey_BackSpace) && preedit() == "녀", "Backspace removes one jamo");
      require(!press(FcitxKey_space) && ic.committed == before + "안녀" && preedit().empty(),
              "Space commits the syllable and still reaches the application");
      before = ic.committed;
      require(press(FcitxKey_R, fcitx::KeyStates(fcitx::KeyState::Shift)) && preedit() == "ㄲ" &&
                  state->view_.value("local_mode", std::string("none")) == "none",
              "Shift+R types ㄲ rather than opening a local mode");
      require(press(FcitxKey_Escape) && preedit().empty() && ic.committed == before, "Escape discards the syllable");
      require(press(FcitxKey_R, fcitx::KeyStates(fcitx::KeyState::CapsLock)) && preedit() == "ㄱ",
              "CapsLock does not shift a jamo");
      require(press(FcitxKey_k) && press(FcitxKey_period) && ic.committed == before + "가." && preedit().empty(),
              "a mark follows the open syllable in one commit");
      before = ic.committed;
      require(!press(FcitxKey_period) && !press(FcitxKey_period) && ic.committed == before,
              "an idle mark is left to the application as ASCII, and repeating it never makes it Chinese");
      require(press(FcitxKey_r) && press(FcitxKey_k) && !press(FcitxKey_1) && ic.committed == before + "가" &&
                  preedit().empty(),
              "a digit ends the syllable and reaches the application");
      require(press(FcitxKey_r) && press(FcitxKey_k) && !press(FcitxKey_Return) && ic.committed == before + "가가",
              "Enter commits the syllable and reaches the application");
      require(press(FcitxKey_r) && press(FcitxKey_apostrophe) && ic.committed == before + "가가ㄱ'",
              "an apostrophe is a mark after the syllable");
      // Hangul_Hanja or a bare F9 converts the composing syllable to Hanja (msime_client.h, MSIME_CONVERT_HANJA). With the list open the candidate keys choose, Escape and Backspace only close it, a letter closes it and composes, a mark writes the Hangul with it, and a trigger is never passed on while a syllable composes.
      {
        const auto candidates = [&] { return state->view_.value("candidates", Json::array()); };
        const auto first = [&] {
          const auto list = candidates();
          return list.empty() ? std::string() : list.at(0).value("text", std::string());
        };
        const auto hangul = [&] { return press(FcitxKey_g) && press(FcitxKey_k) && press(FcitxKey_s); };
        state->preferences_["number_row_selection"] = true;
        before = ic.committed;
        require(!press(FcitxKey_F9) && !press(FcitxKey_Hangul_Hanja) && ic.committed == before,
                "with nothing composing the trigger is the application's");
        require(hangul() && press(FcitxKey_Hangul_Hanja) && first() == "韓" && preedit() == "한" &&
                    ic.committed == before,
                "Hangul_Hanja opens the Hanja list of the composing syllable");
        require(candidates().at(0).value("annotation", std::string()) == "나라 이름 한, 한나라 한",
                "a Hanja carries its 훈음 as the annotation");
        {
          const auto *list = ic.inputPanel().candidateList().get();
          require(list && list->size() > 0 && list->candidate(0).text().toString() == "韓  나라 이름 한, 한나라 한" &&
                      list->candidate(0).text().size() == 2 &&
                      list->candidate(0).text().formatAt(1) ==
                          fcitx::TextFormatFlags{fcitx::TextFormatFlag::Italic, fcitx::TextFormatFlag::DontCommit},
                  "the panel draws the 훈음 as the Hanja row's italic gloss");
        }
        require(press(FcitxKey_F9) && candidates().empty() && preedit() == "한", "the trigger again closes the list");
        require(press(FcitxKey_F9) && first() == "韓", "a bare F9 opens it too");
        require(press(FcitxKey_Down) && press(FcitxKey_Return) && ic.committed == before + "漢" &&
                    preedit().empty() && candidates().empty(),
                "Return chooses the highlighted Hanja instead of breaking the line");
        before = ic.committed;
        require(hangul() && press(FcitxKey_F9) && press(FcitxKey_space) && ic.committed == before + "韓",
                "Space chooses the highlighted Hanja");
        before = ic.committed;
        require(hangul() && press(FcitxKey_F9) && press(FcitxKey_2) && ic.committed == before + "漢",
                "a digit chooses from the page");
        before = ic.committed;
        require(hangul() && press(FcitxKey_F9) && press(FcitxKey_Escape) && candidates().empty() &&
                    preedit() == "한" && ic.committed == before,
                "Escape closes the list and keeps the syllable");
        require(press(FcitxKey_F9) && press(FcitxKey_BackSpace) && candidates().empty() && preedit() == "한" &&
                    ic.committed == before,
                "Backspace closes the list and keeps the syllable");
        require(press(FcitxKey_F9) && press(FcitxKey_period) && ic.committed == before + "한." &&
                    preedit().empty() && candidates().empty(),
                "a paging mark is punctuation that writes the Hangul, not a page turn");
        before = ic.committed;
        require(press(FcitxKey_r) && press(FcitxKey_k) && press(FcitxKey_F9) && press(FcitxKey_r) &&
                    preedit() == "각" && candidates().empty() && ic.committed == before,
                "a letter closes the list and composes");
        require(press(FcitxKey_Escape) && press(FcitxKey_r) && press(FcitxKey_F9) && preedit() == "ㄱ" &&
                    candidates().empty() && ic.committed == before,
                "a lone jamo has no Hanja, and its trigger is still not passed on");
        require(press(FcitxKey_Escape) && preedit().empty(), "Escape discards the lone jamo");
        state->preferences_["number_row_selection"] = false;
        require(hangul() && press(FcitxKey_F9) && !press(FcitxKey_1) && ic.committed == before + "한" &&
                    preedit().empty() && candidates().empty(),
                "with number-row selection off a digit writes the Hangul and reaches the application");
        state->preferences_["number_row_selection"] = true;
        before = ic.committed;
        require(hangul() && press(FcitxKey_F9) && !press(FcitxKey_c, fcitx::KeyStates(fcitx::KeyState::Ctrl)) &&
                    ic.committed == before + "한" && preedit().empty() && candidates().empty(),
                "a shortcut writes the Hangul, never a Hanja");
        // Traditional output is for Chinese text, so it leaves the Hanja list alone: s2t maps 后 to 後, and a row drawn through it would show 後 while committing 后.
        state->traditional_ = true;
        before = ic.committed;
        require(press(FcitxKey_g) && press(FcitxKey_n) && press(FcitxKey_F9) && preedit() == "후" &&
                    state->view_.value("page_count", size_t{0}) > 1 && press(FcitxKey_Page_Down) &&
                    candidates().size() == 2 && candidates().at(1).value("text", std::string()) == "后",
                "the second Hanja page of 후 holds 后 fourth");
        const auto *panel = ic.inputPanel().candidateList().get();
        require(panel && panel->size() == static_cast<int>(candidates().size()), "the panel shows the Hanja list");
        for (int row = 0; row < panel->size(); ++row)
          require(panel->candidate(row).text().toString().rfind(
                      candidates().at(row).value("text", std::string()), 0) == 0,
                  "with traditional output on a Hanja row shows the character it commits");
        require(press(FcitxKey_2) && ic.committed == before + "后", "the row showing 后 commits 后");
        state->traditional_ = false;
      }
      before = ic.committed;
      require(press(FcitxKey_r) && press(FcitxKey_k) && !press(FcitxKey_c, fcitx::KeyStates(fcitx::KeyState::Ctrl)) &&
                  ic.committed == before + "가" && preedit().empty(),
              "a shortcut finishes the syllable instead of discarding it");
      // Switching to another input method keeps what was typed.
      require(press(FcitxKey_r) && press(FcitxKey_k), "Korean composes before the switch");
      fcitx::InputContextEvent switched(&ic, fcitx::EventType::InputContextSwitchInputMethod);
      engine.deactivate(entry, switched);
      require(ic.committed == before + "가가", "switching input methods commits the open syllable");
      engine.activate(entry, focus);
      state->close();
      state->clearPanel();
    }
    // 注音需要语言词库：词库缺失时即使偏好存的是注音，也运行最后使用的中文方案，菜单不列出注音。装好词库后大千键盘的数字行用来拼写，空格是一声，转换时不打开列表，列表只在用户要求时打开。接着越南文用 VNI 数字内嵌组字，从不变成全角；最后藏文用威利转写内嵌组字，空格带音节点、斜杠带垂符上屏。
    {
      const auto dictionaries = std::filesystem::path(directory) / "language-dictionaries";
      std::filesystem::create_directory(dictionaries);
      options["language_dictionaries"] = dictionaries.string();
      options["preferences"]["scheme"] = "zhuyin";
      options["preferences"]["last_chinese_scheme"] = "quanpin";
      options["preferences"]["character_width"] = "halfwidth";
      options["preferences"]["number_row_selection"] = true;
      options["preferences"]["vietnamese"]["input_method"] = "vni";
      std::ofstream(path) << options.dump();
      // The store outranks the options file for the scheme and the width, as the status bar saves them there.
      {
        auto snapshot = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        const auto revision = snapshot.at("revision").get<uint64_t>();
        snapshot["preferences"]["scheme"] = "zhuyin";
        snapshot["preferences"]["character_width"] = "halfwidth";
        // 新会话的其余偏好也以存储为准，文件里这几项要同样写进存储才会生效。
        snapshot["preferences"]["last_chinese_scheme"] = "quanpin";
        snapshot["preferences"]["number_row_selection"] = true;
        snapshot["preferences"]["vietnamese"]["input_method"] = "vni";
        snapshot["revision"] = revision + 1;
        const auto document = snapshot.dump();
        const auto saved = response(msime_client_save_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size(), revision,
            reinterpret_cast<const uint8_t *>(document.data()), document.size()));
        require(saved.value("revision", uint64_t{}) > revision, "Zhuyin saved as the scheme");
      }
      const auto press = [&](fcitx::KeySym sym, fcitx::KeyStates states = fcitx::KeyStates()) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym, states));
        engine.keyEvent(entry, event);
        return event.accepted();
      };
      const auto preedit = [&] { return ic.inputPanel().clientPreedit().toString(); };
      const auto candidates = [&] { return state->view_.value("candidates", Json::array()); };
      const auto offered = [&](fcitx::Action *action) {
        const auto actions = engine.scheme_menu_.actions();
        return std::find(actions.begin(), actions.end(), action) != actions.end();
      };
      require(state->ensure(), "session with Zhuyin saved and its dictionary missing");
      require(state->effectiveScheme() == "quanpin" && state->view_.value("scheme", 10u) == 0 &&
                  state->modeIndicatorLabel() == "中",
              "a missing Zhuyin dictionary falls back to the last Chinese scheme");
      require(!offered(&engine.scheme_zhuyin_action_) && !offered(&engine.scheme_cantonese_action_) &&
                  !offered(&engine.scheme_stroke_action_) && offered(&engine.scheme_quanpin_action_) &&
                  offered(&engine.scheme_vietnamese_action_),
              "the scheme menu leaves out a scheme whose dictionary is missing");
      require(engine.scheme_quanpin_action_.isChecked(&ic) && !engine.scheme_zhuyin_action_.isChecked(&ic),
              "the scheme menu marks the fallback in use");
      require(!state->selectScheme("zhuyin") && state->view_.value("scheme", 10u) == 0,
              "Zhuyin cannot be selected without its dictionary");
      require(!state->selectScheme("stroke") && state->view_.value("scheme", 10u) == 0,
              "Stroke cannot be selected without its dictionary");
      const auto fixture =
          std::string("python3 '") + MSIME_ZHUYIN_DICTIONARY_FIXTURE + "' '" + dictionaries.string() + "'";
      require(std::system(fixture.c_str()) == 0, "Zhuyin dictionary fixture written");
      state->close();
      state->clearPanel();
      require(state->ensure() && state->effectiveScheme() == "zhuyin" && state->view_.value("scheme", 0u) == 6 &&
                  state->modeIndicatorLabel() == "注",
              "the saved Zhuyin runs once its dictionary is installed");
      require(offered(&engine.scheme_zhuyin_action_) && engine.scheme_zhuyin_action_.isChecked(&ic) &&
                  !offered(&engine.scheme_cantonese_action_),
              "the scheme menu offers and marks Zhuyin with its dictionary installed");
      auto before = ic.committed;
      require(press(FcitxKey_1) && press(FcitxKey_8) && ic.committed == before,
              "the digit row spells ㄅㄚ rather than choosing a candidate");
      require(press(FcitxKey_space) && preedit() == "八" && candidates().empty() && ic.committed == before,
              "Space gives the first tone and converts without opening a list");
      require(press(FcitxKey_Down) && candidates().size() == 2 &&
                  candidates().at(0).value("text", std::string()) == "八" &&
                  candidates().at(1).value("text", std::string()) == "巴",
              "Down opens the Zhuyin list");
      require(press(FcitxKey_2) && preedit() == "巴" && candidates().empty() && ic.committed == before,
              "a digit picks from the open list without committing");
      require(press(FcitxKey_Down) && !candidates().empty() && press(FcitxKey_1) && preedit() == "八" &&
                  ic.committed == before,
              "1 picks the first row of the list");
      require(press(FcitxKey_Return) && ic.committed == before + "八" && preedit().empty(),
              "Return commits the conversion");
      before = ic.committed;
      require(press(FcitxKey_1) && press(FcitxKey_8) && press(FcitxKey_space) && press(FcitxKey_F9) &&
                  !candidates().empty(),
              "F9 opens the Zhuyin list");
      require(press(FcitxKey_Escape) && candidates().empty() && preedit() == "八",
              "Escape closes the list and keeps the conversion");
      require(press(FcitxKey_Escape) && preedit().empty() && ic.committed == before,
              "a second Escape discards the conversion");
      require(press(FcitxKey_comma) && press(FcitxKey_space) && preedit() == "欸" && ic.committed == before,
              "the comma spells ㄝ rather than writing Chinese punctuation");
      require(press(FcitxKey_Return) && ic.committed == before + "欸", "Return commits 欸");
      // Vietnamese with VNI: the digits after a word place its marks inline and never open a list.
      require(state->selectScheme("vietnamese") && state->view_.value("scheme", 0u) == 7 &&
                  engine.scheme_vietnamese_action_.isChecked(&ic) && state->modeIndicatorLabel() == "越",
              "the scheme menu selects Vietnamese");
      before = ic.committed;
      require(!press(FcitxKey_6) && ic.committed == before, "an idle VNI digit is the application's");
      for (const auto sym : {FcitxKey_v, FcitxKey_i, FcitxKey_e, FcitxKey_t, FcitxKey_6, FcitxKey_5})
        require(press(sym), "VNI keys compose");
      require(preedit() == "việt" && candidates().empty() && ic.committed == before,
              "VNI digits compose việt inline");
      // A mark follows the word as ASCII, with fullwidth output on too, and an idle mark is left to the application unwidened.
      require(state->toggleWidth() && state->fullwidthOutput() && preedit() == "việt", "fullwidth output on");
      require(press(FcitxKey_comma) && ic.committed == before + "việt," && preedit().empty(),
              "the comma after a word commits with it as ASCII");
      require(!press(FcitxKey_comma) && ic.committed == before + "việt,",
              "an idle comma reaches the application as ASCII");
      require(press(FcitxKey_a) && preedit() == "a" && !press(FcitxKey_space) && ic.committed == before + "việt,a",
              "Space commits the word unwidened and reaches the application");
      require(state->toggleWidth() && !state->fullwidthOutput(), "fullwidth output off");
      // Caps Lock types a capital that starts a word.
      before = ic.committed;
      require(press(FcitxKey_A, fcitx::KeyStates(fcitx::KeyState::CapsLock)) && preedit() == "A" &&
                  ic.committed == before,
              "Caps Lock starts a Vietnamese word with a capital");
      require(!press(FcitxKey_Return) && ic.committed == before + "A" && preedit().empty(),
              "Return commits the word and reaches the application");
      // Switching input methods, and leaving a client that draws no preedit of its own, write the word out.
      require(press(FcitxKey_v) && press(FcitxKey_i) && preedit() == "vi", "Vietnamese composes before the switch");
      fcitx::InputContextEvent switched(&ic, fcitx::EventType::InputContextSwitchInputMethod);
      engine.deactivate(entry, switched);
      require(ic.committed == before + "Avi", "switching input methods commits the open word");
      engine.activate(entry, focus);
      ic.setCapabilityFlags(fcitx::CapabilityFlag::NoFlag);
      require(press(FcitxKey_v) && press(FcitxKey_i) && ic.inputPanel().preedit().toString() == "vi" &&
                  state->view_.value("scheme", 0u) == 7,
              "Vietnamese composes in the panel for a client without preedit");
      blur();
      require(ic.committed == before + "Avivi" && state->view_.value("editing_text", std::string{}).empty(),
              "focus out commits the open word");
      ic.setCapabilityFlags(fcitx::CapabilityFlags{fcitx::CapabilityFlag::Preedit,
                                                 fcitx::CapabilityFlag::SurroundingText});
      ic.focusIn();
      engine.activate(entry, focus);
      require(press(FcitxKey_a) && preedit() == "a" && ic.committed == before + "Avivi",
              "the next field starts a new word");
      // 藏文：威利原文内嵌显示为转换后的藏文，从不打开列表。空格和斜杠分别带音节点、垂符上屏并被吞掉，回车只上屏藏文，同样被吞掉。
      require(offered(&engine.scheme_tibetan_action_), "the scheme menu offers Tibetan");
      require(state->selectScheme("tibetan") && state->view_.value("scheme", 0u) == 8 &&
                  engine.scheme_tibetan_action_.isChecked(&ic) && !engine.scheme_vietnamese_action_.isChecked(&ic) &&
                  state->modeIndicatorLabel() == "藏",
              "the scheme menu selects Tibetan");
      before = ic.committed;
      require(!press(FcitxKey_1) && ic.committed == before, "an idle digit is the application's in Tibetan");
      for (const auto sym : {FcitxKey_b, FcitxKey_k, FcitxKey_r, FcitxKey_a})
        require(press(sym), "Wylie letters compose");
      require(preedit() == "བཀྲ" && candidates().empty() && ic.committed == before, "Wylie composes བཀྲ inline");
      require(press(FcitxKey_space) && ic.committed == before + "བཀྲ་" && preedit().empty(),
              "Space commits the syllable with a tsheg and keeps the key");
      for (const auto sym : {FcitxKey_s, FcitxKey_h, FcitxKey_i, FcitxKey_s})
        require(press(sym), "Wylie letters compose");
      require(press(FcitxKey_slash) && ic.committed == before + "བཀྲ་ཤིས།" && preedit().empty(),
              "the slash commits the syllable with a shad");
      require(press(FcitxKey_slash) && ic.committed == before + "བཀྲ་ཤིས།།", "an idle slash writes a shad");
      // 撇号在空闲时也是拼写（achung 开头的音节），加号是叠写；带 Shift 或 CapsLock 的大写字母是另一个字母。
      before = ic.committed;
      require(press(FcitxKey_apostrophe) && press(FcitxKey_o) && press(FcitxKey_d) && preedit() == "འོད" &&
                  ic.committed == before,
              "an apostrophe starts an achung syllable");
      require(press(FcitxKey_Return) && ic.committed == before + "འོད" && preedit().empty(),
              "Return commits the syllable without a tsheg and keeps the key");
      before = ic.committed;
      require(press(FcitxKey_p) && press(FcitxKey_a) && press(FcitxKey_d) &&
                  press(FcitxKey_plus, fcitx::KeyStates(fcitx::KeyState::Shift)) && press(FcitxKey_m) &&
                  press(FcitxKey_a) && preedit() == "པདྨ" && ic.committed == before,
              "the plus stacks the Wylie letters");
      require(press(FcitxKey_Return) && ic.committed == before + "པདྨ", "Return commits the stacked syllable");
      before = ic.committed;
      require(press(FcitxKey_T, fcitx::KeyStates(fcitx::KeyState::Shift)) && press(FcitxKey_a) && preedit() == "ཊ" &&
                  ic.committed == before,
              "Shift types the uppercase Wylie letter");
      require(press(FcitxKey_Return) && ic.committed == before + "ཊ", "Return commits the retroflex letter");
      before = ic.committed;
      require(press(FcitxKey_D, fcitx::KeyStates(fcitx::KeyState::CapsLock)) && press(FcitxKey_a) &&
                  preedit() == "ཌ" && ic.committed == before,
              "Caps Lock starts a syllable with the uppercase Wylie letter");
      require(press(FcitxKey_Return) && ic.committed == before + "ཌ", "Return commits the Caps Lock syllable");
      // 第一次 Esc 把显示退回威利原文，第二次丢弃组字；Backspace 删一个原文按键。
      before = ic.committed;
      require(press(FcitxKey_k) && press(FcitxKey_a) && preedit() == "ཀ" && press(FcitxKey_Escape) &&
                  preedit() == "ka" && ic.committed == before,
              "Escape restores the raw Wylie");
      require(press(FcitxKey_Escape) && preedit().empty() && ic.committed == before,
              "a second Escape discards the composition");
      require(press(FcitxKey_k) && press(FcitxKey_a) && press(FcitxKey_BackSpace) && preedit() == "ཀ" &&
                  ic.committed == before,
              "Backspace takes back one Wylie key");
      require(press(FcitxKey_Escape) && press(FcitxKey_Escape) && preedit().empty(), "the composition is discarded");
      // 其他标点跟在藏文后面写成 ASCII，全角输出打开时也一样；空闲的标点和空格都交给应用，不变全角。
      require(state->toggleWidth() && state->fullwidthOutput(), "fullwidth output on");
      before = ic.committed;
      require(press(FcitxKey_k) && press(FcitxKey_a) && press(FcitxKey_comma) && ic.committed == before + "ཀ," &&
                  preedit().empty(),
              "the comma after a syllable commits with it as ASCII");
      require(!press(FcitxKey_comma) && !press(FcitxKey_space) && ic.committed == before + "ཀ,",
              "an idle comma or space reaches the application unwidened");
      require(state->toggleWidth() && !state->fullwidthOutput(), "fullwidth output off");
      // 导航键先把藏文按显示写出去（不带音节点），再交给应用；切换输入法同样把组字写出去。
      before = ic.committed;
      require(press(FcitxKey_k) && press(FcitxKey_a) && !press(FcitxKey_Left) && ic.committed == before + "ཀ" &&
                  preedit().empty(),
              "Left writes the syllable out without a tsheg and reaches the application");
      require(press(FcitxKey_g) && press(FcitxKey_a) && preedit() == "ག", "Tibetan composes before the switch");
      fcitx::InputContextEvent tibetanSwitch(&ic, fcitx::EventType::InputContextSwitchInputMethod);
      engine.deactivate(entry, tibetanSwitch);
      require(ic.committed == before + "ཀག", "switching input methods commits the open syllable");
      engine.activate(entry, focus);
      // 装好 msime-stroke.db 并重新读取选项后，笔画进入菜单；它是中文方案，选中后记为最后使用的中文方案。
      const auto strokeFixture =
          std::string("python3 '") + MSIME_STROKE_DICTIONARY_FIXTURE + "' '" + dictionaries.string() + "'";
      require(std::system(strokeFixture.c_str()) == 0, "Stroke dictionary fixture written");
      state->close();
      state->clearPanel();
      require(state->ensure() && offered(&engine.scheme_stroke_action_) && offered(&engine.scheme_zhuyin_action_) &&
                  !engine.scheme_stroke_action_.isChecked(&ic),
              "the scheme menu offers Stroke with its dictionary installed");
      require(engine.scheme_menu_.actions().size() == 9, "the menu lists Zhuyin and Stroke beside the seven base schemes");
      engine.scheme_stroke_action_.activate(&ic);
      require(state->effectiveScheme() == "stroke" && state->view_.value("scheme", 0u) == 9 &&
                  engine.scheme_stroke_action_.isChecked(&ic) && state->modeIndicatorLabel() == "笔",
              "the scheme menu selects Stroke");
      {
        const auto stored = response(msime_client_load_preferences(
            reinterpret_cast<const uint8_t *>(preferenceDirectory.data()), preferenceDirectory.size()));
        require(stored.at("preferences").value("last_chinese_scheme", std::string()) == "stroke",
                "Stroke is recorded as the last Chinese scheme");
      }
      // 空闲时只有五个笔画字母开始组合：通配符 x 和其他字母都交给应用。
      before = ic.committed;
      require(!press(FcitxKey_x) && !press(FcitxKey_a) && preedit().empty() && ic.committed == before,
              "an idle Stroke wildcard or other letter is the application's");
      // 原样预编辑样式从 `reading` 画出笔画字形；editing_text 保留字母。
      require(press(FcitxKey_h) && preedit() == "一" && !candidates().empty() &&
                  candidates().at(0).value("text", std::string()) == "一" &&
                  state->view_.value("editing_text", std::string()) == "h",
              "h composes the stroke 一");
      require(press(FcitxKey_s) && preedit() == "一丨" && candidates().size() == 2 &&
                  candidates().at(0).value("text", std::string()) == "十" &&
                  candidates().at(1).value("text", std::string()) == "木" &&
                  press(FcitxKey_Page_Down) && !candidates().empty() &&
                  candidates().at(0).value("text", std::string()) == "古" &&
                  press(FcitxKey_Page_Up),
              "h s lists 十 exactly and then its completions");
      require(press(FcitxKey_q) && preedit() == "一丨" && ic.committed == before,
              "a letter that is no stroke is swallowed while composing");
      require(press(FcitxKey_space) && ic.committed == before + "十" && preedit().empty(),
              "Space commits the highlighted Stroke candidate");
      before = ic.committed;
      require(press(FcitxKey_h) && press(FcitxKey_x) && preedit() == "一＊" && candidates().size() >= 2 &&
                  candidates().at(0).value("text", std::string()) == "十" &&
                  candidates().at(1).value("text", std::string()) == "二",
              "the wildcard x matches any one stroke");
      require(press(FcitxKey_2) && ic.committed == before + "二" && preedit().empty(),
              "a digit picks the Stroke candidate");
      before = ic.committed;
      require(press(FcitxKey_h) && press(FcitxKey_s) && press(FcitxKey_BackSpace) && preedit() == "一" &&
                  ic.committed == before,
              "Backspace removes the last stroke");
      require(press(FcitxKey_s) && press(FcitxKey_Return) && ic.committed == before + "hs" && preedit().empty(),
              "Return commits the typed stroke letters");
      before = ic.committed;
      require(press(FcitxKey_p) && press(FcitxKey_n) && preedit() == "丿丶" && press(FcitxKey_Escape) &&
                  preedit().empty() && ic.committed == before,
              "Escape clears the Stroke composition");
      state->close();
      state->clearPanel();
    }
    removeFixtureTree(directory);
    std::cout << "Fcitx5 native context tests passed\n";
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
