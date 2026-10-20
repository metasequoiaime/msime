#pragma once

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "WaveOverlayPlacement.h"

namespace msime::linux_host {

// 悬浮工具栏（macOS 的 MetasequoiaFloatingToolbarPanel，FloatingToolbarPanel.mm）在 Linux 上的纯计算部分：读共享偏好、按钮和它们的字、几何、命中、摆放和空闲隐藏。不依赖 X11、cairo 或宿主，单测直接包含；IBus 宿主和 Fcitx5 插件经 FloatingToolbar 共用它。

// 工具栏的按钮，顺序与 macOS 相同。
enum class FloatingToolbarButton {
  InputMode,
  InputScheme,
  Punctuation,
  Fullwidth,
  CharacterSet,
  Emoji,
  Handwriting,
  ScreenKeyboard,
  Voice,
  Settings,
};

// 按钮和菜单项请求宿主做的事。宿主照「工具栏」子菜单里同一项的路径去做，不另写一份。
enum class FloatingToolbarAction {
  ToggleInputMode,
  SelectScheme,
  TogglePunctuation,
  ToggleFullwidth,
  ToggleCharacterSet,
  OpenEmoji,
  OpenHandwriting,
  OpenScreenKeyboard,
  ToggleVoice,
  OpenSettings,
  Promotion,
  Website,
  Help,
  About,
  Feedback,
  Hide,
};

// 共享偏好 `floating_toolbar` 里与这块窗口有关的部分。
struct FloatingToolbarConfig {
  bool enabled = true;
  double scale = 1.0;
  double font_size = 24.0;
  // 偏好 `show_app_logo`：开着时左端画水杉标志，关着时那里是一条握把。
  bool show_logo = false;
  std::vector<FloatingToolbarButton> buttons;
  bool operator==(const FloatingToolbarConfig &other) const {
    return enabled == other.enabled && scale == other.scale && font_size == other.font_size &&
           show_logo == other.show_logo && buttons == other.buttons;
  }
  bool operator!=(const FloatingToolbarConfig &other) const { return !(*this == other); }
};

// 读共享偏好，规则与 macOS 的 applySizingPreferences 相同：scale_percent 只认 75、100、125、150，font_size 只认 16 到 28 的偶数，其余回到 100% 和 24；各按钮的开关缺省时，表情、手写、屏幕键盘和语音为关，其余为开（与 preferences.rs 的 FloatingToolbarPreferences 默认值一致）。english_mode 开关管的是中/英按钮。不提供手写的版本（handwriting_offered 为 false）不放手写按钮；语音按钮另要语音输入开着（voice_available），与「工具栏」子菜单相同。
inline FloatingToolbarConfig floating_toolbar_config(const nlohmann::json &preferences, bool handwriting_offered,
                                                     bool voice_available) {
  FloatingToolbarConfig config;
  const auto toolbar_value = preferences.is_object() ? preferences.find("floating_toolbar") : preferences.end();
  const auto toolbar = preferences.is_object() && toolbar_value != preferences.end() && toolbar_value->is_object()
                           ? *toolbar_value
                           : nlohmann::json::object();
  const auto flag = [&](const char *key, bool fallback) {
    const auto found = toolbar.find(key);
    return found != toolbar.end() && found->is_boolean() ? found->get<bool>() : fallback;
  };
  config.enabled = flag("enabled", true);
  if (const auto scale = toolbar.find("scale_percent"); scale != toolbar.end() && scale->is_number_integer()) {
    const auto value = scale->get<std::int64_t>();
    if (value == 75 || value == 100 || value == 125 || value == 150) config.scale = static_cast<double>(value) / 100.0;
  }
  if (const auto font = toolbar.find("font_size"); font != toolbar.end() && font->is_number_integer()) {
    const auto value = font->get<std::int64_t>();
    if (value >= 16 && value <= 28 && value % 2 == 0) config.font_size = static_cast<double>(value);
  }
  if (preferences.is_object())
    if (const auto logo = preferences.find("show_app_logo"); logo != preferences.end() && logo->is_boolean())
      config.show_logo = logo->get<bool>();
  const std::pair<FloatingToolbarButton, bool> buttons[] = {
      {FloatingToolbarButton::InputMode, flag("english_mode", true)},
      {FloatingToolbarButton::InputScheme, flag("input_scheme", true)},
      {FloatingToolbarButton::Punctuation, flag("punctuation", true)},
      {FloatingToolbarButton::Fullwidth, flag("fullwidth", true)},
      {FloatingToolbarButton::CharacterSet, flag("character_set", true)},
      {FloatingToolbarButton::Emoji, flag("emoji", false)},
      {FloatingToolbarButton::Handwriting, handwriting_offered && flag("handwriting", false)},
      {FloatingToolbarButton::ScreenKeyboard, flag("screen_keyboard", false)},
      {FloatingToolbarButton::Voice, voice_available && flag("voice", false)},
      {FloatingToolbarButton::Settings, flag("settings", true)},
  };
  for (const auto &[button, present] : buttons)
    if (present) config.buttons.push_back(button);
  return config;
}

// 方案菜单里的一项。
struct FloatingToolbarScheme {
  std::string id;
  std::string title;
  bool operator==(const FloatingToolbarScheme &other) const { return id == other.id && title == other.title; }
};

// 工具栏显示的状态：中/英按钮的字、标点、全半角、简繁，以及方案菜单的内容。宿主在模式可能变化的地方整份交过来，变了才重画。
struct FloatingToolbarState {
  // 中文输入开着（关着是直接输入，按钮显示「英」）。
  bool input_enabled = true;
  // Engine 自己的英文候选模式，按钮显示「En」。
  bool english_candidates = false;
  bool caps_lock = false;
  // 正在用的方案 id（"quanpin"、"shuangpin"……）和它在菜单里的名字（「双拼（小鹤）」）。
  std::string scheme = "quanpin";
  std::string scheme_title;
  bool chinese_punctuation = true;
  bool fullwidth = false;
  bool traditional = false;
  std::vector<FloatingToolbarScheme> schemes;
  bool operator==(const FloatingToolbarState &other) const {
    return input_enabled == other.input_enabled && english_candidates == other.english_candidates &&
           caps_lock == other.caps_lock && scheme == other.scheme && scheme_title == other.scheme_title &&
           chinese_punctuation == other.chinese_punctuation && fullwidth == other.fullwidth &&
           traditional == other.traditional && schemes == other.schemes;
  }
  bool operator!=(const FloatingToolbarState &other) const { return !(*this == other); }
};

// 中/英按钮的字，与 macOS 的 updateEnglishInputMode:englishCandidateMode:scheme: 相同：大写锁定时 A，直接输入时「英」，英文候选模式「En」，其余是方案徽标——全拼 中、双拼 双、五笔 五、日语 日、韩语 한、粤拼 粤、注音 注、越南语 越、藏文 ཀ、笔画 笔。
inline std::string floating_toolbar_mode_glyph(const FloatingToolbarState &state) {
  if (state.caps_lock) return "A";
  if (!state.input_enabled) return "英";
  if (state.english_candidates) return "En";
  const std::pair<std::string_view, const char *> badges[] = {
      {"shuangpin", "双"}, {"wubi", "五"},      {"japanese", "日"}, {"korean", "한"},  {"cantonese", "粤"},
      {"zhuyin", "注"},    {"vietnamese", "越"}, {"tibetan", "ཀ"},  {"stroke", "笔"},
  };
  for (const auto &[scheme, badge] : badges)
    if (state.scheme == scheme) return badge;
  return "中";
}

// 按钮画的是字（Glyph）还是图标。图标用 cairo 路径画，对应 macOS 的 SF Symbols：list.bullet、face.smiling、hand.draw、keyboard、mic.fill、gearshape。
enum class FloatingToolbarIcon { Glyph, List, Smile, Handwriting, Keyboard, Microphone, Gear };

struct FloatingToolbarFace {
  std::string glyph;
  FloatingToolbarIcon icon = FloatingToolbarIcon::Glyph;
  // 按钮提示，也是 macOS 的辅助功能标签。
  std::string tooltip;
  // 按钮此刻能不能用：不能用时画成淡色、点了不做事，提示照旧出（见 floating_toolbar_button_needs_focus）。
  bool enabled = true;
};

// 一个按钮在当前状态下的样子，字和提示与 macOS 相同。display_name 是这一版的产品名（「水杉输入法」），写进设置按钮的提示。
inline FloatingToolbarFace floating_toolbar_face(FloatingToolbarButton button, const FloatingToolbarState &state,
                                                 std::string_view display_name) {
  switch (button) {
  case FloatingToolbarButton::InputMode: {
    const std::string action =
        !state.input_enabled || state.english_candidates ? "切换到中文输入" : "切换到英文输入";
    return {floating_toolbar_mode_glyph(state), FloatingToolbarIcon::Glyph,
            state.scheme_title.empty() ? action : state.scheme_title + " · " + action};
  }
  case FloatingToolbarButton::InputScheme:
    return {"", FloatingToolbarIcon::List, "切换输入方案"};
  case FloatingToolbarButton::Punctuation:
    return {state.chinese_punctuation ? "。" : ".", FloatingToolbarIcon::Glyph,
            state.chinese_punctuation ? "切换到西文标点" : "切换到中文标点"};
  case FloatingToolbarButton::Fullwidth:
    return {state.fullwidth ? "全" : "半", FloatingToolbarIcon::Glyph,
            state.fullwidth ? "切换到半角输入" : "切换到全角输入"};
  case FloatingToolbarButton::CharacterSet:
    return {state.traditional ? "繁" : "简", FloatingToolbarIcon::Glyph,
            state.traditional ? "切换到简体输出" : "切换到繁体输出"};
  case FloatingToolbarButton::Emoji:
    return {"", FloatingToolbarIcon::Smile, "打开水杉表情面板"};
  case FloatingToolbarButton::Handwriting:
    return {"", FloatingToolbarIcon::Handwriting, "打开水杉手写识别板"};
  case FloatingToolbarButton::ScreenKeyboard:
    return {"", FloatingToolbarIcon::Keyboard, "打开水杉屏幕键盘"};
  case FloatingToolbarButton::Voice:
    return {"", FloatingToolbarIcon::Microphone, "开始或结束语音输入"};
  case FloatingToolbarButton::Settings:
    return {"", FloatingToolbarIcon::Gear, "打开" + std::string(display_name) + "设置"};
  }
  return {};
}

// 左键点按钮要做的事。切换输入方案按钮不直接做事，弹出方案菜单（floating_toolbar_scheme_menu）。
inline std::optional<FloatingToolbarAction> floating_toolbar_button_action(FloatingToolbarButton button) {
  switch (button) {
  case FloatingToolbarButton::InputMode: return FloatingToolbarAction::ToggleInputMode;
  case FloatingToolbarButton::InputScheme: return std::nullopt;
  case FloatingToolbarButton::Punctuation: return FloatingToolbarAction::TogglePunctuation;
  case FloatingToolbarButton::Fullwidth: return FloatingToolbarAction::ToggleFullwidth;
  case FloatingToolbarButton::CharacterSet: return FloatingToolbarAction::ToggleCharacterSet;
  case FloatingToolbarButton::Emoji: return FloatingToolbarAction::OpenEmoji;
  case FloatingToolbarButton::Handwriting: return FloatingToolbarAction::OpenHandwriting;
  case FloatingToolbarButton::ScreenKeyboard: return FloatingToolbarAction::OpenScreenKeyboard;
  case FloatingToolbarButton::Voice: return FloatingToolbarAction::ToggleVoice;
  case FloatingToolbarButton::Settings: return FloatingToolbarAction::OpenSettings;
  }
  return std::nullopt;
}

// 中/英按钮在英文候选模式（显示「En」）下回到中文：退出英文候选模式，而不是关掉中文输入去「英」，与 macOS 的 floatingToolbarDidRequestToggleInputMode: 相同（中文输入开着且 dedicated_english 时 setDedicatedEnglishInputMode:NO，其余切换中英文）。宿主据此在 ToggleInputMode 时选英文候选模式的开关还是中文输入的开关。
inline bool floating_toolbar_input_mode_leaves_english_candidates(const FloatingToolbarState &state) {
  return state.input_enabled && state.english_candidates;
}

// 这个动作是否要往有焦点的输入框里写或改它的输入状态。IBus 宿主的这些路径只对获得焦点的引擎生效（与属性菜单相同），属主失焦后它们什么也不做，工具栏就把对应的按钮画成不能用（FloatingToolbar::Options::actions_need_focus）；打开面板、官网、帮助和隐藏工具栏不需要输入框。
inline bool floating_toolbar_action_needs_focus(FloatingToolbarAction action) {
  switch (action) {
  case FloatingToolbarAction::ToggleInputMode:
  case FloatingToolbarAction::SelectScheme:
  case FloatingToolbarAction::TogglePunctuation:
  case FloatingToolbarAction::ToggleFullwidth:
  case FloatingToolbarAction::ToggleCharacterSet:
  case FloatingToolbarAction::ToggleVoice:
  case FloatingToolbarAction::Promotion:
    return true;
  default:
    return false;
  }
}

// 按钮版本：切换输入方案按钮本身不做事，它弹出的菜单每一项都是 SelectScheme。
inline bool floating_toolbar_button_needs_focus(FloatingToolbarButton button) {
  if (button == FloatingToolbarButton::InputScheme) return true;
  const auto action = floating_toolbar_button_action(button);
  return action && floating_toolbar_action_needs_focus(*action);
}

// 弹出菜单的一项：分隔线、可勾选的方案或一个动作。
struct FloatingToolbarMenuItem {
  std::string label;
  FloatingToolbarAction action = FloatingToolbarAction::OpenSettings;
  std::string argument;
  bool checked = false;
  bool separator = false;
  // 此刻不能用的项画成淡色、点了不做事（与按钮的 enabled 同一条规则）。
  bool disabled = false;
};

// 切换输入方案按钮弹出的菜单：可用的方案，正在用的那个打勾，与输入法菜单的「输入方案」子菜单同一份。
inline std::vector<FloatingToolbarMenuItem> floating_toolbar_scheme_menu(const FloatingToolbarState &state) {
  std::vector<FloatingToolbarMenuItem> items;
  for (const auto &scheme : state.schemes)
    items.push_back({scheme.title, FloatingToolbarAction::SelectScheme, scheme.id, scheme.id == state.scheme, false});
  return items;
}

// 设置按钮的右键菜单，对应 macOS 的 CreateMetasequoiaFloatingToolbarUtilityMenu。macOS 的「表情与符号…」打开系统字符面板，Linux 没有那块面板，换成水杉表情面板；Linux 的包由发行版的包管理器更新，所以没有「检查更新…」。
inline std::vector<FloatingToolbarMenuItem> floating_toolbar_utility_menu(std::string_view display_name) {
  using A = FloatingToolbarAction;
  const std::string name(display_name);
  return {
      {"水杉表情面板…", A::OpenEmoji, "", false, false},
      {"打开设置…", A::OpenSettings, "", false, false},
      {"", A::OpenSettings, "", false, true},
      {"宣传" + name, A::Promotion, "", false, false},
      {"访问 msime.app", A::Website, "", false, false},
      {"使用帮助…", A::Help, "", false, false},
      {"关于" + name + "…", A::About, "", false, false},
      {"问题反馈…", A::Feedback, "", false, false},
      {"", A::OpenSettings, "", false, true},
      {"隐藏悬浮状态栏", A::Hide, "", false, false},
  };
}

// 工具栏的几何，逻辑像素，照搬 macOS 的常量：左端是 34 宽的标志（不显示标志时是 10 宽的握把）加 4 的间隙，这一段是拖动把手；再隔 3 是 1.2 宽的分隔线，再隔 4 是第一个按钮；按钮宽是字号加 8，高也是字号加 8，按钮之间 2；右端留 6；整条高字号加 20，圆角 10。所有长度再乘 scale_percent。表面四周另留阴影边（只在有合成器时）。
struct FloatingToolbarGeometry {
  static constexpr double logo_width = 34.0;
  static constexpr double grip_width = 10.0;
  static constexpr double logo_gap = 4.0;
  static constexpr double logo_divider_gap = 3.0;
  static constexpr double divider_width = 1.2;
  static constexpr double divider_button_gap = 4.0;
  static constexpr double trailing = 6.0;
  static constexpr double button_spacing = 2.0;
  static constexpr double button_padding = 8.0;
  static constexpr double glyph_scale = 0.95;
  static constexpr double logo_mark_side = 22.0;
  static constexpr double corner_radius = 10.0;
  static constexpr double shadow_margin = 10.0;
  // 第一次显示时离工作区右、下边 20；用户拖过以后恢复时至少离边 12，与 macOS 的 SizedToolbarFrame 相同。
  static constexpr double default_margin = 20.0;
  static constexpr double restored_margin = 12.0;
};

struct FloatingToolbarRect {
  double x = 0, y = 0, width = 0, height = 0;
  bool contains(double px, double py) const { return px >= x && px < x + width && py >= y && py < y + height; }
};

// 排好的工具栏，逻辑像素。plate 是工具栏本体在表面里的矩形，表面比它四周各多 margin。
struct FloatingToolbarLayout {
  double width = 0;
  double height = 0;
  double margin = 0;
  FloatingToolbarRect plate;
  // 标志（或握把）占的那一段，也是拖动把手。
  FloatingToolbarRect handle;
  FloatingToolbarRect divider;
  bool divider_visible = false;
  std::vector<FloatingToolbarRect> buttons;
  double radius = 0;
  double glyph_size = 0;
  double icon_size = 0;
  double scale = 1;
};

inline FloatingToolbarLayout floating_toolbar_layout(const FloatingToolbarConfig &config, bool shadow) {
  using G = FloatingToolbarGeometry;
  const double scale = config.scale;
  const double font = config.font_size;
  const std::size_t count = config.buttons.size();
  FloatingToolbarLayout layout;
  layout.scale = scale;
  layout.margin = shadow ? G::shadow_margin : 0.0;
  const double handle_width = (config.show_logo ? G::logo_width : G::grip_width) + G::logo_gap;
  const double leading = handle_width + G::logo_divider_gap + G::divider_width + G::divider_button_gap;
  const double gaps = count > 0 ? static_cast<double>(count - 1) : 0.0;
  // 与 macOS 一样向外取整，按钮不会被裁掉。
  const double plate_width =
      std::ceil((static_cast<double>(count) * (font + G::button_padding) + gaps * G::button_spacing + G::trailing + leading) *
                scale);
  const double plate_height = std::ceil((font + 20.0) * scale);
  layout.plate = {layout.margin, layout.margin, plate_width, plate_height};
  layout.width = plate_width + 2.0 * layout.margin;
  layout.height = plate_height + 2.0 * layout.margin;
  layout.handle = {layout.margin, layout.margin, handle_width * scale, plate_height};
  const double control = (font + G::button_padding) * scale;
  const double control_top = layout.margin + (plate_height - control) / 2.0;
  const double divider_x = layout.handle.x + layout.handle.width + G::logo_divider_gap * scale;
  layout.divider = {divider_x, control_top, G::divider_width * scale, control};
  // logo（或握把）在所有按钮都关掉时也留着，工具栏才拖得动；分隔线只在有按钮时画。
  layout.divider_visible = count > 0;
  double x = divider_x + (G::divider_width + G::divider_button_gap) * scale;
  for (std::size_t index = 0; index < count; ++index) {
    layout.buttons.push_back({x, control_top, control, control});
    x += control + G::button_spacing * scale;
  }
  layout.radius = G::corner_radius * scale;
  layout.glyph_size = font * scale * G::glyph_scale;
  layout.icon_size = font * scale;
  return layout;
}

// 指针在哪：某个按钮、拖动把手，或两者都不是（分隔线、按钮之间的缝、阴影边）。绘制、悬停和点击都用这一个判断，三者不会对不上。坐标是表面里的逻辑像素。
struct FloatingToolbarHit {
  std::optional<std::size_t> button;
  bool handle = false;
};

inline FloatingToolbarHit floating_toolbar_hit(const FloatingToolbarLayout &layout, double x, double y) {
  for (std::size_t index = 0; index < layout.buttons.size(); ++index)
    if (layout.buttons[index].contains(x, y)) return {index, false};
  return {std::nullopt, layout.handle.contains(x, y)};
}

// 工具栏本体左上角的根窗口坐标（设备像素），规则与 macOS 的 SizedToolbarFrame 相同：没拖过时放在工作区右下角、离右边和下边各 default_margin；拖过的位置照用，只是收进工作区四边各 restored_margin 以内。工作区比工具栏还小时贴左上。
inline WaveOverlayPosition floating_toolbar_position(std::optional<WaveOverlayPosition> saved, int plate_width,
                                                     int plate_height, WaveOverlayWorkArea work, int default_margin,
                                                     int restored_margin) {
  if (!saved) {
    const int x = work.x + work.width - plate_width - default_margin;
    const int y = work.y + work.height - plate_height - default_margin;
    return {std::max(work.x, x), std::max(work.y, y)};
  }
  const int minimum_x = work.x + restored_margin;
  const int maximum_x = std::max(minimum_x, work.x + work.width - plate_width - restored_margin);
  const int minimum_y = work.y + restored_margin;
  const int maximum_y = std::max(minimum_y, work.y + work.height - plate_height - restored_margin);
  return {std::clamp(saved->x, minimum_x, maximum_x), std::clamp(saved->y, minimum_y, maximum_y)};
}

// 记住的位置存在状态目录的 floating-toolbar-position.json（{"x":…,"y":…}，工具栏本体左上角的根窗口坐标），与 Windows 的 floating_toolbar_position.json 一样只属于这台机器的显示器布局，不进同步到云端的共享偏好。读不懂的文件当作没有记过，工具栏回到默认的右下角。
inline constexpr const char *kFloatingToolbarPositionFile = "floating-toolbar-position.json";

// 状态目录按版本分开（$XDG_STATE_HOME/<client_directory>，没有时是 ~/.local/state/<client_directory>），与候选面板的接管记录（PanelRestoreRecord.h）同一个目录；两个都不是绝对路径时不记位置。
inline std::optional<std::string> floating_toolbar_position_path(const char *xdg_state_home, const char *home,
                                                                 std::string_view client_directory) {
  const auto absolute = [](const char *path) { return path && *path == '/'; };
  std::string directory;
  if (absolute(xdg_state_home)) directory = xdg_state_home;
  else if (absolute(home)) directory = std::string(home) + "/.local/state";
  else return std::nullopt;
  return directory + "/" + std::string(client_directory) + "/" + kFloatingToolbarPositionFile;
}

inline std::optional<WaveOverlayPosition> parse_floating_toolbar_position(std::string_view text) {
  const auto document = nlohmann::json::parse(text, nullptr, false);
  if (!document.is_object()) return std::nullopt;
  WaveOverlayPosition position;
  for (const auto &[key, target] : {std::pair{"x", &position.x}, std::pair{"y", &position.y}}) {
    const auto found = document.find(key);
    if (found == document.end() || !found->is_number_integer()) return std::nullopt;
    if (found->is_number_unsigned() && found->get<std::uint64_t>() > 32767) return std::nullopt;
    const auto value = found->get<std::int64_t>();
    if (value < -32768 || value > 32767) return std::nullopt;
    *target = static_cast<int>(value);
  }
  return position;
}

inline std::string serialize_floating_toolbar_position(WaveOverlayPosition position) {
  return nlohmann::json{{"x", position.x}, {"y", position.y}}.dump();
}

// 显示规则的形状照 macOS 的 MetasequoiaFloatingToolbarShouldShow：开关开着、输入法处于激活状态、前台不是全屏应用。macOS 现在总是传「不是全屏」（面板带 FullScreenAuxiliary，盖在全屏应用上面）；Linux 与 Windows 一样在前台窗口全屏时收起，见 README「输入细节与平台差异」和 .agents/notes 里 2026-10-21 的决定。
constexpr bool floating_toolbar_should_show(bool enabled, bool active, bool fullscreen) {
  return enabled && active && !fullscreen;
}

struct FloatingToolbarTiming {
  // 连续 10 秒没有输入就自动隐藏，下一次按键或点击再回来，与 macOS 的 noteInputForDelegate 相同。
  static constexpr int idle_ms = 10000;
  // 指针停在按钮上这么久才出提示，与 GTK 的默认提示延迟相近。
  static constexpr int tooltip_ms = 600;
};

}  // namespace msime::linux_host
