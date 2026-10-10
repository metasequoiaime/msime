// Real Fcitx input contexts and the real Host API. All input is synthetic.
#include "../FcitxEngine.cpp"
#include <fcitx-config/option.h>
#include <array>
#include <cstdlib>
#include <iostream>
#include <filesystem>
#include <thread>
#include <chrono>
#include <limits>
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

using namespace msime::fcitx_host;
class FixtureContext : public fcitx::InputContext {
public:
  explicit FixtureContext(fcitx::InputContextManager &manager) : InputContext(manager, "msime-test") { created(); }
  ~FixtureContext() override { destroy(); }
  const char *frontend() const override { return "msime-test"; }
  void commitStringImpl(const std::string &text) override { committed += text; }
  void deleteSurroundingTextImpl(int, unsigned int) override {}
  void forwardKeyImpl(const fcitx::ForwardKeyEvent &event) override {
    forwarded.emplace_back(event.rawKey(), event.isRelease());
  }
  void updatePreeditImpl() override {}
  std::string committed;
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
  require(shown(row(Json{{"text", "在线"}, {"corrected", true}, {"source", 2}})) == "在线*  ☁️",
          "marker comes before the cloud badge");
  require(shown(row(Json{{"text", "汉语"}, {"corrected", true}}, true)) == "漢語*",
          "marker follows the traditional-converted word");
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
  require(shown(glossed).toString() == "韓  나라 이름 한, 한나라 한  Korea" && shown(glossed).size() == 3 &&
              shown(glossed).formatAt(1) == secondary &&
              shown(glossed).formatAt(2) == fcitx::TextFormatFlags(fcitx::TextFormatFlag::NoFlag),
          "a translation follows the 훈음 and the annotation is not drawn twice");
  const auto helpcode = word(Json{{"text", "你"}, {"annotation", "ab"}}, true, std::string());
  require(shown(helpcode).toString() == "你  ab" && shown(helpcode).size() == 1, "a helpcode stays plain row text");
  require(word(Json{{"text", "你"}, {"annotation", "ab"}}, false, std::string()).text() == "你" &&
              shown(word(Json{{"text", "你"}, {"annotation", "ab"}}, false, std::string())).toString() == "你",
          "annotations off still hide a helpcode");
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
  require(decorated.find("Gravity=Top Right\nOverlayOffsetX=19\nOverlayOffsetY=28\nHideOverlayIfOversize=False\n") !=
              std::string::npos,
          "overlay pinned top right, its bottom one padding below the card top (8 + 25 + 7 - 12)");
  require(decorated.find("[InputPanel/ContentMargin]\nLeft=19\nRight=19\nTop=40\n") != std::string::npos,
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
  // toolbar_theme "follow" defers to the global mode, whose "system" follows the desktop, so a light desktop gets a light badge.
  require(!dark(Json{{"toolbar_theme", "follow"}, {"theme", "system"}}, false), "follow on a light desktop gives a light badge");
  require(dark(Json{{"toolbar_theme", "follow"}, {"theme", "system"}}, true), "follow on a dark desktop gives a dark badge");
  require(dark(Json{{"toolbar_theme", "follow"}, {"theme", "dark"}}, false), "follow defers to a dark global theme");
  // An explicit toolbar mode wins over the global mode and over the candidate mode.
  require(!dark(Json{{"toolbar_theme", "light"}, {"theme", "dark"}, {"candidate_theme", "dark"}}, true),
          "an explicit light toolbar theme wins");
  require(dark(Json{{"toolbar_theme", "dark"}, {"theme", "light"}, {"candidate_theme", "light"}}, false),
          "an explicit dark toolbar theme wins");
  require(!dark(Json{{"toolbar_theme", "follow"}, {"theme", "system"}, {"candidate_theme", "dark"}}, false),
          "the candidate mode does not colour the badge when toolbar_theme is present");
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
int main(int argc, char **argv) {
  try {
    if (argc == 2 && std::string(argv[1]) == "--theme-priority") return candidateThemePriority();
#ifdef MSIME_FCITX5_HINT_FONT
    if (argc == 2 && std::string(argv[1]) == "--theme-hint") return candidateThemeHint();
#endif
    autocorrectMarker();
    koreanHanjaGlossRow();
    candidateThemeDecoration();
    modeBadgeTheme();
    classicuiTakeoverRecord();
    translationPreferenceChangesIncludeAccount();
    require(argc == 2 || (argc == 3 && (std::string(argv[2]) == "--ai" ||
                                       std::string(argv[2]) == "--ctrl-space" ||
                                       std::string(argv[2]) == "--local-modes")),
            "usage: fcitx5-native-test <verified-resources> [--ai|--ctrl-space|--local-modes]");
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
    options["preferences"]["candidate_page_size"] = 2;
    options["preferences"]["clipboard_history"] = true;
    options["preferences"]["voice_input"]["hotkey_hold_space_lock"] = false;
    // A stored commit strategy, which the Linux settings page does not offer and the hosts ignore: the streaming preedit below must still appear.
    options["preferences"]["voice_input"]["commit_mode"] = "ctrl_v";
    const auto clipboardPath = std::filesystem::path(options.at("preferences_directory").get<std::string>()) /
                               "clipboard_history.json";
    std::ofstream(clipboardPath) << Json::array({"剪贴板合成测试", "第二条"}).dump();
    options["clipboard_history_path"] = clipboardPath.string();
    options["preferences"]["cloud_candidates"] = !ai;
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
    require(!state->voice_hotkey_hold_space_lock_,
            "initial voice context reads the hold-to-lock preference");
    require(state->view_.contains("candidates"), "focus must unpack transition view");
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
        press(FcitxKey_Shift_L, shiftState);
        require(press(sym, shiftState), "Shift local-mode shortcut must be consumed");
        require(state->view_.value("local_mode", std::string("none")) == mode,
                "Shift local-mode shortcut must enter its mode");
        press(sym, shiftState, true);
        press(FcitxKey_Shift_L, fcitx::KeyStates(), true);
        require(state->input_enabled_, "Shift chord release must not switch to English");
        require(press(FcitxKey_Escape) &&
                    state->view_.value("local_mode", std::string("none")) == "none",
                "Escape must leave the local mode");
      }
      require(!press(FcitxKey_V, shiftState), "Disabled expression mode must pass through");
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
    // One per addAction() in activate(), plus the toolbar entry the host adds
    // once it has a session. Adding or removing a status action changes this on
    // purpose; the count is here so one going missing is noticed.
    require(ic.statusArea().actions(fcitx::StatusGroup::InputMethod).size() == 24,
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
    require(engine.global_theme_action_.shortText(&ic) == "主题：跟随系统", "theme action starts at the shared default");
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
    require(engine.global_theme_action_.shortText(&ic) == "主题：Solarized", "theme action names the package drawn");
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
    require(ic.statusArea().actions(fcitx::StatusGroup::InputMethod).size() == 24,
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
      require(state->session_ != 0 && heldForTray() && statusActions() == 15,
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
      require(state->session_ != 0 && !heldForTray() && statusActions() == 24,
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
      require(state->session_ != 0 && statusActions() == 24, "focus brings the session and the status actions back");
      fcitx::InputContextEvent switched(&ic, fcitx::EventType::InputContextSwitchInputMethod);
      engine.deactivate(entry, switched);
      require(state->session_ == 0 && statusActions() == 0,
              ("switching input methods removes the status actions: " + std::to_string(statusActions())).c_str());
      engine.activate(entry, focus);
      require(state->session_ != 0 && statusActions() == 24, "switching back restores the status actions");
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
    engine.smart_punctuation_action_.activate(&ic);
    require(state->preferences_.value("smart_punctuation", false),
            "smart punctuation status action restores live mode");
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
    require(engine.desktop_tools_menu_.actions().size() == 13,
            "desktop tools menu attached");
    // The status area opens with the design menu: 中文/英文; 全角/标点/译文; 输入方案; 主题/词库…/设置…/关于.
    {
      const auto status = ic.statusArea().actions(fcitx::StatusGroup::InputMethod);
      const std::vector<fcitx::Action *> design{
          &engine.input_mode_action_, &engine.width_action_,
          &engine.chinese_punctuation_action_, &engine.candidate_translation_action_, &engine.scheme_action_,
          &engine.global_theme_action_, &engine.dictionary_action_, &engine.settings_action_, &engine.about_action_};
      require(status.size() > design.size() && std::equal(design.begin(), design.end(), status.begin()),
              "status area starts with the design menu");
      require(engine.input_mode_action_.shortText(&ic) == "中文" && engine.width_action_.shortText(&ic) == "全角字符" &&
                  engine.candidate_translation_action_.shortText(&ic) == "显示译文" &&
                  engine.dictionary_action_.shortText(&ic) == "词库…" && engine.settings_action_.shortText(&ic) == "设置…" &&
                  engine.about_action_.shortText(&ic) == "关于水杉输入法",
              "design menu entries use the design labels");
      // The Engine's dedicated English mode is not the design's 英文 (that is the 中文 toggle unchecked); it stays reachable in 输入选项, next to 混合英文.
      const auto input = engine.input_group_menu_.actions();
      const auto mixed = std::find(input.begin(), input.end(), &engine.mixed_english_action_);
      require(std::find(status.begin(), status.end(), &engine.english_action_) == status.end() &&
                  mixed != input.end() && std::next(mixed) != input.end() && *std::next(mixed) == &engine.english_action_,
              "dedicated English mode moves into 输入选项");
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
      for (auto *menu : {&engine.scheme_menu_, &engine.desktop_tools_menu_})
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
    require(state->wave_overlay_.actions_visible && state->wave_overlay_.listening,
            "voice start arms the native wave overlay actions");
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
    state->voice_job_ = std::async(std::launch::async, [] {
      std::this_thread::sleep_for(std::chrono::milliseconds(250));
      return Json{{"text", "已取消语音"}};
    }).share();
    state->voice_loading_ = true;
    state->voice_socket_.clear();
    state->voice_generation_ = 0;
    const auto cancelStarted = std::chrono::steady_clock::now();
    require(state->cancelVoice(), "voice cancellation accepts an active delayed provider");
    require(std::chrono::steady_clock::now() - cancelStarted < std::chrono::milliseconds(100),
            "voice cancellation does not wait for the provider future");
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
      state->voice_phase_ = "录音中";
      const auto voiceAux = [&] { return ic.inputPanel().auxUp().toString(); };
      state->updateVoiceOverlay();
      require(shows == 1 && state->wave_overlay_failed_ && !state->wave_overlay_visible_,
              "a surface that cannot show is marked failed");
      require(voiceAux().rfind("语音：录音中", 0) == 0, "a failed surface falls back to the auxiliary text");
      state->voice_transcript_ = "你好";
      state->updateVoiceOverlay();
      require(shows == 1 && voiceAux() == "语音：录音中：你好",
              "later updates stay on the auxiliary text without retrying the surface");
      state->render();
      require(voiceAux() == "语音：录音中：你好", "a panel redraw keeps the voice status");
      state->voice_loading_ = false;
      state->voice_transcript_.clear();
      state->wave_overlay_failed_ = false;
      state->wave_overlay_surface_ = std::move(nativeSurface);
      ic.inputPanel().setAuxUp(fcitx::Text());
    }
    // A provider that gives no result is a provider failure, as in the IBus host, and a named missing dependency says what to install; neither may read as 未识别到文字, which recording again cannot fix.
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
      require(ic.inputPanel().auxUp().toString() == "语音：" + notice,
              "a provider failure shows its fixed notice");
      state->voice_failure_visible_ = false;
      state->hideVoiceOverlay();
      ic.inputPanel().setAuxUp(fcitx::Text());
    }
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
      struct Descriptor { int fd; ~Descriptor() { if (fd >= 0) close(fd); } } server{translationServer};
      pollfd ready{server.fd, POLLIN, 0};
      if (poll(&ready, 1, 5000) <= 0) return false;
      Descriptor client{accept(server.fd, nullptr, nullptr)};
      if (client.fd < 0) return false;
      std::string request;
      const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
      while (request.size() < 16384 && request.find('\n') == std::string::npos &&
             std::chrono::steady_clock::now() < deadline) {
        pollfd readable{client.fd, POLLIN, 0};
        if (poll(&readable, 1, 100) <= 0) continue;
        char buffer[1024];
        const auto count = read(client.fd, buffer, sizeof(buffer));
        if (count <= 0) return false;
        request.append(buffer, count);
      }
      const auto document = Json::parse(request);
      if (document.at("kind") != "translation" || document.at("query").at("target_language") != "en") return false;
      const auto &texts = document.at("query").at("candidates");
      if (texts.empty() || !texts.at(0).is_string()) return false;
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
    require(key(FcitxKey_n) && key(FcitxKey_i), "translation composition");
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
    const auto glossPath = std::filesystem::path(options.at("user_data").get<std::string>()) /
                           "translation-glosses.db";
    require(std::filesystem::exists(glossPath), "English translation gloss database exists");
    std::ifstream glossFile(glossPath);
    const std::string glossContent((std::istreambuf_iterator<char>(glossFile)),
                                   std::istreambuf_iterator<char>());
    require(glossContent.find("synthetic-gloss") != std::string::npos,
            "English translation gloss is persisted");
    const auto translatedText = state->view_.at("candidates").at(0).at("text").get<std::string>();
    const auto beforeTranslatedCommit = ic.committed;
    ic.inputPanel().candidateList()->candidate(0).select(&ic);
    require(ic.committed == beforeTranslatedCommit + translatedText, "gloss excluded from committed text");
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
      require(state->cycleHelpcodeSchema() && !state->preferences_save_retry_ && storedPack().empty() &&
                  storedHelpcodeSchema() == packedSchema,
              "the first status bar click clears the pack in the store and keeps its schema");
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
