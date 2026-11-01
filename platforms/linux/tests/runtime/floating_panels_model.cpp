#include "../src/overlay/FloatingPanelColors.h"
#include "../src/overlay/FloatingToolbar.h"
#include "../src/overlay/FloatingToolbarModel.h"
#include "../src/overlay/ShuangpinKeymapModel.h"

#include <unistd.h>

#include <algorithm>
#include <cassert>
#include <cmath>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <vector>

namespace host = msime::linux_host;
using Json = nlohmann::json;

namespace {

Json shuangpin_view(const std::string &editing) {
  return Json{{"scheme", 1},
              {"local_mode", "none"},
              {"dedicated_english", false},
              {"shuangpin_profile", "ziranma"},
              {"editing_text", editing}};
}

void keymap_rules() {
  // 偏好缺省按关，不是布尔值也按关，与 macOS 和 Windows 一致。
  assert(!host::shuangpin_keymap_preference(Json::object()));
  assert(host::shuangpin_keymap_preference(Json{{"shuangpin_keymap_hint", true}}));
  assert(!host::shuangpin_keymap_preference(Json{{"shuangpin_keymap_hint", "true"}}));

  // 双拼组字中才显示，高亮最后一个字母（转成大写）或分号。
  const auto hint = host::shuangpin_keymap_hint(shuangpin_view("nih"));
  assert(hint && hint->profile == "ziranma" && hint->key == 'H');
  assert(host::shuangpin_keymap_hint(shuangpin_view("ni;"))->key == ';');
  assert(host::shuangpin_keymap_hint(shuangpin_view("ni1"))->key == 0);
  assert(!host::shuangpin_keymap_hint(shuangpin_view("")));
  auto other = shuangpin_view("ni");
  other["scheme"] = 0;
  assert(!host::shuangpin_keymap_hint(other));
  other = shuangpin_view("ni");
  other["local_mode"] = "unicode";
  assert(!host::shuangpin_keymap_hint(other));
  other = shuangpin_view("ni");
  other["dedicated_english"] = true;
  assert(!host::shuangpin_keymap_hint(other));
  // 自定义方案的表在偏好里，按小鹤画会标错键，所以不显示；不认识的名字按小鹤处理。
  other = shuangpin_view("ni");
  other["shuangpin_profile"] = "custom";
  assert(!host::shuangpin_keymap_hint(other));
  other["shuangpin_profile"] = "someday";
  assert(host::shuangpin_keymap_hint(other)->profile == "xiaohe");

  // 三排键帽与 macOS 的键序相同，中排的分号只在方案给它分了韵母时出现。
  const auto rows = host::shuangpin_keymap_rows(Json{{"Q", "q / iu"}, {"S", "s / iong ong"}});
  assert(rows[0].size() == 10 && rows[1].size() == 9 && rows[2].size() == 7);
  assert(rows[0][0].key == "Q" && rows[0][0].codes == "q / iu");
  assert(rows[1][1].codes == "s / iong · ong");
  assert(host::shuangpin_keymap_rows(Json{{";", "ing"}})[1].size() == 10);
  // 零声母按条目排序，v 开头写成 ü。
  assert(host::shuangpin_keymap_zero_initial_text(Json{{"ang", "ah"}, {"a", "aa"}, {"ve", "vt"}}) ==
         "零声母  a=aa · ang=ah · üe=vt");
  assert(host::shuangpin_keymap_title("shoudao") == "首道双拼键位");
  assert(host::shuangpin_keymap_title("anything") == "小鹤双拼键位");

  // 键帽：620 宽的卡片，十个键的排两边各留 14，九个 31，七个 58，键间距 5。
  const auto first = host::shuangpin_keymap_key_rect(0, 0, 10);
  assert(first.x == 14.0 && first.height == 38.0);
  const auto last = host::shuangpin_keymap_key_rect(0, 9, 10);
  assert(std::abs(last.x + last.width - (620.0 - 14.0)) < 1e-9);
  assert(host::shuangpin_keymap_key_rect(1, 0, 9).x == 31.0);
  assert(host::shuangpin_keymap_key_rect(2, 0, 7).x == 58.0);
  // 三排之后是零声母一行，底边留 10，正好是 203 高。
  assert(std::abs(host::shuangpin_keymap_footer_top() + 13.0 + 10.0 - 203.0) < 1e-9);

  // 放在光标下方、让开候选窗再隔 8；下面放不下就翻到光标上方；左边对齐光标，收进工作区。
  const host::WaveOverlayWorkArea work{0, 0, 1920, 1080};
  auto position = host::shuangpin_keymap_position(100, 200, 20, 620, 203, 60, work, 8, 16);
  assert(position.x == 100 && position.y == 200 + 20 + 60 + 8);
  position = host::shuangpin_keymap_position(100, 900, 20, 620, 203, 60, work, 8, 16);
  assert(position.y == 900 - 60 - 8 - 203);
  position = host::shuangpin_keymap_position(1800, 200, 20, 620, 203, 60, work, 8, 16);
  assert(position.x == 1920 - 16 - 620);
  // 横排一行是字号加 42，竖排每行字号加 10 再加 24，拼音行另加 22。
  assert(host::shuangpin_keymap_candidate_clearance(18, 1.0, true, 5, false) == 60.0);
  assert(host::shuangpin_keymap_candidate_clearance(18, 1.0, false, 5, false) == 164.0);
  assert(host::shuangpin_keymap_candidate_clearance(18, 2.0, true, 5, true) == 164.0);
  // 宿主自己画的候选窗量出来的高度：在光标下方是底边离光标底边多远，翻到上方是顶边离光标顶边多远。
  assert(host::shuangpin_keymap_measured_clearance(200, 20, 220, 150) == 150);
  assert(host::shuangpin_keymap_measured_clearance(200, 20, 230, 150) == 160);
  assert(host::shuangpin_keymap_measured_clearance(900, 20, 760, 140) == 140);
}

void toolbar_config() {
  // 缺省：中/英、切换方案、标点、全半角、简繁和设置六个按钮，24 号字、100%，不画标志。
  const auto defaults = host::floating_toolbar_config(Json::object(), true, true);
  using B = host::FloatingToolbarButton;
  assert(defaults.enabled && defaults.scale == 1.0 && defaults.font_size == 24.0 && !defaults.show_logo);
  assert((defaults.buttons ==
          std::vector<B>{B::InputMode, B::InputScheme, B::Punctuation, B::Fullwidth, B::CharacterSet, B::Settings}));
  // 只认 macOS 的取值：缩放 75/100/125/150，字号 16 到 28 的偶数。
  auto preferences = Json{{"floating_toolbar", {{"scale_percent", 125}, {"font_size", 20}, {"emoji", true},
                                                 {"handwriting", true}, {"voice", true}, {"english_mode", false}}},
                          {"show_app_logo", true}};
  auto config = host::floating_toolbar_config(preferences, true, true);
  assert(config.scale == 1.25 && config.font_size == 20.0 && config.show_logo);
  assert((config.buttons == std::vector<B>{B::InputScheme, B::Punctuation, B::Fullwidth, B::CharacterSet, B::Emoji,
                                           B::Handwriting, B::Voice, B::Settings}));
  // 不提供手写的版本不放手写按钮，语音输入关着时不放语音按钮。
  config = host::floating_toolbar_config(preferences, false, false);
  assert(std::find(config.buttons.begin(), config.buttons.end(), B::Handwriting) == config.buttons.end());
  assert(std::find(config.buttons.begin(), config.buttons.end(), B::Voice) == config.buttons.end());
  preferences["floating_toolbar"]["scale_percent"] = 110;
  preferences["floating_toolbar"]["font_size"] = 21;
  config = host::floating_toolbar_config(preferences, true, true);
  assert(config.scale == 1.0 && config.font_size == 24.0);
  assert(!host::floating_toolbar_config(Json{{"floating_toolbar", {{"enabled", false}}}}, true, true).enabled);
}

void toolbar_faces() {
  host::FloatingToolbarState state;
  state.scheme_title = "全拼";
  // 中/英按钮：大写锁定 A，直接输入「英」，英文候选模式「En」，其余是方案徽标。
  assert(host::floating_toolbar_mode_glyph(state) == "中");
  state.scheme = "shuangpin";
  assert(host::floating_toolbar_mode_glyph(state) == "双");
  state.scheme = "tibetan";
  assert(host::floating_toolbar_mode_glyph(state) == "ཀ");
  state.english_candidates = true;
  assert(host::floating_toolbar_mode_glyph(state) == "En");
  state.input_enabled = false;
  assert(host::floating_toolbar_mode_glyph(state) == "英");
  state.caps_lock = true;
  assert(host::floating_toolbar_mode_glyph(state) == "A");

  host::FloatingToolbarState plain;
  plain.scheme_title = "双拼（小鹤）";
  using B = host::FloatingToolbarButton;
  // 提示与 macOS 的辅助功能标签相同：方案名写在中/英按钮动作的前面。
  assert(host::floating_toolbar_face(B::InputMode, plain, "水杉输入法").tooltip == "双拼（小鹤） · 切换到英文输入");
  assert(host::floating_toolbar_face(B::Punctuation, plain, "").glyph == "。");
  assert(host::floating_toolbar_face(B::Fullwidth, plain, "").glyph == "半");
  assert(host::floating_toolbar_face(B::CharacterSet, plain, "").glyph == "简");
  plain.chinese_punctuation = false;
  plain.fullwidth = true;
  plain.traditional = true;
  assert(host::floating_toolbar_face(B::Punctuation, plain, "").glyph == ".");
  assert(host::floating_toolbar_face(B::Fullwidth, plain, "").tooltip == "切换到半角输入");
  assert(host::floating_toolbar_face(B::CharacterSet, plain, "").glyph == "繁");
  assert(host::floating_toolbar_face(B::Settings, plain, "水杉输入法").tooltip == "打开水杉输入法设置");
  assert(host::floating_toolbar_face(B::Voice, plain, "").icon == host::FloatingToolbarIcon::Microphone);
  // 切换方案按钮不直接做事，弹出方案菜单。
  assert(!host::floating_toolbar_button_action(B::InputScheme));
  assert(host::floating_toolbar_button_action(B::Settings) == host::FloatingToolbarAction::OpenSettings);
  // 中/英按钮只在「En」（中文输入开着、英文候选模式）时退出英文候选模式，其余切换中文输入，与 macOS 相同。
  host::FloatingToolbarState mode;
  assert(!host::floating_toolbar_input_mode_leaves_english_candidates(mode));
  mode.english_candidates = true;
  assert(host::floating_toolbar_input_mode_leaves_english_candidates(mode));
  mode.input_enabled = false;
  assert(!host::floating_toolbar_input_mode_leaves_english_candidates(mode));
  // 要输入框的按钮：切换类、方案菜单、语音和宣传语；打开面板、设置、官网和隐藏不要。
  for (const auto button : {B::InputMode, B::InputScheme, B::Punctuation, B::Fullwidth, B::CharacterSet, B::Voice})
    assert(host::floating_toolbar_button_needs_focus(button));
  for (const auto button : {B::Emoji, B::Handwriting, B::ScreenKeyboard, B::Settings})
    assert(!host::floating_toolbar_button_needs_focus(button));
  assert(host::floating_toolbar_action_needs_focus(host::FloatingToolbarAction::Promotion));
  assert(!host::floating_toolbar_action_needs_focus(host::FloatingToolbarAction::Hide));
  assert(!host::floating_toolbar_action_needs_focus(host::FloatingToolbarAction::Website));

  plain.scheme = "wubi";
  plain.schemes = {{"quanpin", "全拼"}, {"wubi", "五笔 86"}};
  const auto schemes = host::floating_toolbar_scheme_menu(plain);
  assert(schemes.size() == 2 && !schemes[0].checked && schemes[1].checked && schemes[1].argument == "wubi");
  const auto utility = host::floating_toolbar_utility_menu("水杉输入法");
  assert(utility.back().action == host::FloatingToolbarAction::Hide && utility.back().label == "隐藏悬浮状态栏");
  assert(utility[3].label == "宣传水杉输入法");
}

void toolbar_geometry() {
  // 默认的一行与 macOS 的 kToolbarWidth、kToolbarHeight 相同：255 × 44。
  auto config = host::floating_toolbar_config(Json{{"show_app_logo", true}}, true, true);
  auto layout = host::floating_toolbar_layout(config, false);
  assert(layout.plate.width == 255.0 && layout.plate.height == 44.0 && layout.margin == 0.0);
  assert(layout.buttons.size() == 6 && layout.buttons[0].width == 32.0 && layout.radius == 10.0);
  // 第一个按钮在 34 + 4 的标志、3 的间隙、1.2 的分隔线和 4 的间隙之后。
  assert(std::abs(layout.buttons[0].x - 46.2) < 1e-9);
  assert(std::abs(layout.buttons[1].x - layout.buttons[0].x - 34.0) < 1e-9);
  // 不显示标志时左端是 10 宽的握把，整条窄 24。
  config.show_logo = false;
  layout = host::floating_toolbar_layout(config, false);
  assert(layout.plate.width == 231.0 && layout.handle.width == 14.0);
  // 缩放与 macOS 一样整体相乘再向外取整；阴影边只加在本体外面。
  config.scale = 1.5;
  layout = host::floating_toolbar_layout(config, true);
  assert(layout.plate.height == 66.0 && layout.margin == 10.0 && layout.width == layout.plate.width + 20.0);
  // 命中：按钮、拖动把手，或两者都不是（分隔线附近、阴影边）。
  const auto &button = layout.buttons[2];
  auto hit = host::floating_toolbar_hit(layout, button.x + 1, button.y + 1);
  assert(hit.button == 2u && !hit.handle);
  hit = host::floating_toolbar_hit(layout, layout.handle.x + 2, layout.handle.y + layout.handle.height / 2);
  assert(!hit.button && hit.handle);
  hit = host::floating_toolbar_hit(layout, 1, 1);
  assert(!hit.button && !hit.handle);
  // 一个按钮都没有时只剩把手，分隔线不画。
  config.buttons.clear();
  assert(!host::floating_toolbar_layout(config, false).divider_visible);

  // 没拖过：工作区右下角离边 20；拖过：照用，收进四边 12 以内。
  const host::WaveOverlayWorkArea work{0, 0, 1920, 1080};
  auto position = host::floating_toolbar_position(std::nullopt, 255, 44, work, 20, 12);
  assert(position.x == 1920 - 255 - 20 && position.y == 1080 - 44 - 20);
  position = host::floating_toolbar_position(host::WaveOverlayPosition{-50, 5000}, 255, 44, work, 20, 12);
  assert(position.x == 12 && position.y == 1080 - 44 - 12);

  // 位置文件按版本放在状态目录里，读不懂就当作没有记过。
  assert(host::floating_toolbar_position_path("/state", "/home/user", "msime-client") ==
         "/state/msime-client/floating-toolbar-position.json");
  assert(host::floating_toolbar_position_path("relative", "/home/user", "msime-client") ==
         "/home/user/.local/state/msime-client/floating-toolbar-position.json");
  assert(!host::floating_toolbar_position_path(nullptr, nullptr, "msime-client"));
  const auto parsed = host::parse_floating_toolbar_position(host::serialize_floating_toolbar_position({-30, 640}));
  assert(parsed && parsed->x == -30 && parsed->y == 640);
  assert(!host::parse_floating_toolbar_position("{\"x\":1}"));
  assert(!host::parse_floating_toolbar_position("{\"x\":1,\"y\":40000}"));
  assert(!host::parse_floating_toolbar_position("not json"));
}

void colors() {
  // 系统主题：Adwaita 的白底、深色字，悬停和分隔线是字色的一层淡色。工具栏的勾跟候选窗的选中底（白字画在它上面，默认是 Adwaita Blue 5 `kLinuxSelection`），键位图的高亮键是 GNOME 的强调色。
  host::CandidateTheme light;
  const auto toolbar = host::floating_toolbar_colors(light);
  assert(toolbar.surface == 0xFFFFFF && toolbar.accent == host::kLinuxSelection && !toolbar.dark);
  assert(toolbar.hover != toolbar.surface && toolbar.divider != toolbar.surface);
  const auto keymap = host::shuangpin_keymap_colors(light);
  assert(keymap.surface == 0xFFFFFF && keymap.accent == host::kLinuxAccent && keymap.accent_text == 0xFFFFFF);
  assert(keymap.key != keymap.surface);
  // 系统主题解析出的强调色是 Adwaita 写在卡片上的强调字色（深色下是浅蓝），高亮键照样填 GNOME 的强调底色。
  for (const bool dark : {false, true}) {
    host::CandidateTheme system;
    system.dark = dark;
    system.colors.accent = host::candidate_native_palette(dark).accent;
    assert(host::shuangpin_keymap_colors(system).accent == host::kLinuxAccent);
  }
  // 皮肤给了自己的悬停色时用它；高亮键与 macOS 一样填皮肤的强调色、白字，不是选中行的底色。
  host::CandidateTheme skin;
  skin.colors.background = 0x102030;
  skin.colors.text = 0xF0F0F0;
  skin.colors.menu.hover = host::ThemeColor{0x405060, 0xFF};
  skin.colors.selected = 0xD8F5E4;
  skin.colors.accent = 0x07C160;
  assert(host::floating_toolbar_colors(skin).hover == 0x405060);
  const auto skin_keymap = host::shuangpin_keymap_colors(skin);
  assert(skin_keymap.accent == 0x07C160 && skin_keymap.accent_text == 0xFFFFFF);
}

// 测试用的事件循环：记下定时器，由测试决定什么时候让它们到期。
struct FakeLoop {
  struct Timer {
    int milliseconds;
    std::function<void()> fire;
    bool alive = true;
  };
  std::vector<std::shared_ptr<Timer>> timers;
  host::OverlayLoop loop() {
    host::OverlayLoop result;
    result.watch = [](int, std::function<void()>) { return host::OverlayLoop::Token(); };
    result.timer = [this](int milliseconds, std::function<void()> fire) {
      auto timer = std::make_shared<Timer>(Timer{milliseconds, std::move(fire)});
      timers.push_back(timer);
      // 令牌析构就撤掉定时器。
      return host::OverlayLoop::Token(timer.get(), [timer](void *) { timer->alive = false; });
    };
    return result;
  }
  void expire(int milliseconds) {
    const auto pending = timers;
    for (const auto &timer : pending)
      if (timer->alive && timer->milliseconds == milliseconds) {
        timer->alive = false;
        timer->fire();
      }
  }
};

struct FakeSurface final : host::FloatingToolbarSurface {
  explicit FakeSurface(Callbacks callbacks) : callbacks(std::move(callbacks)) {}
  void present(const Content &content, std::optional<host::WaveOverlayPosition> saved) override {
    ++presented;
    last = content;
    last_saved = saved;
    shown = true;
  }
  void hide() override { shown = false; }
  bool visible() const override { return shown; }
  bool foreground_fullscreen() override { return fullscreen; }
  Callbacks callbacks;
  Content last;
  std::optional<host::WaveOverlayPosition> last_saved;
  int presented = 0;
  bool shown = false;
  bool fullscreen = false;
};

void controller() {
  FakeLoop loop;
  FakeSurface *surface = nullptr;
  std::vector<std::pair<const void *, host::FloatingToolbarAction>> actions;
  char directory_template[] = "/tmp/msime-toolbar-test-XXXXXX";
  const std::filesystem::path directory = mkdtemp(directory_template);
  host::FloatingToolbar::Options options;
  options.display_name = "水杉输入法";
  options.position_file = directory / "state" / "floating-toolbar-position.json";
  auto toolbar = host::FloatingToolbar::create(
      loop.loop(),
      [&](const void *owner, host::FloatingToolbarAction action, const std::string &) {
        actions.emplace_back(owner, action);
      },
      options,
      [&](host::FloatingToolbarSurface::Callbacks callbacks) {
        auto created = std::make_unique<FakeSurface>(std::move(callbacks));
        surface = created.get();
        return created;
      });
  assert(toolbar && surface);
  const int first = 1, second = 2;
  host::FloatingToolbarState state;
  // 配置好之前、或没有属主时不显示。
  toolbar->configure(host::floating_toolbar_config(Json::object(), true, true), host::FloatingToolbarColors{});
  assert(!surface->shown);
  toolbar->activate(&first, state);
  assert(surface->shown && surface->presented == 1 && !surface->last_saved);
  // 只有属主的状态才画出来；状态没变不重画。
  host::FloatingToolbarState english = state;
  english.input_enabled = false;
  toolbar->update(&second, english);
  assert(surface->last.state.input_enabled);
  toolbar->update(&first, state);
  assert(surface->presented == 1);
  toolbar->update(&first, english);
  assert(surface->presented == 2 && !surface->last.state.input_enabled);
  // 10 秒没有输入就隐藏，下一次按键回来。
  loop.expire(host::FloatingToolbarTiming::idle_ms);
  assert(!surface->shown);
  toolbar->note_input(&first);
  assert(surface->shown);
  // 同一个属主重新激活（换个输入框）不唤醒空闲隐藏的工具栏，换了属主才唤醒。
  loop.expire(host::FloatingToolbarTiming::idle_ms);
  assert(!surface->shown);
  toolbar->activate(&first, english);
  assert(!surface->shown);
  toolbar->activate(&second, english);
  assert(surface->shown && toolbar->owner() == &second);
  // 按钮的动作交给当前属主；点击工具栏本身也算输入。
  surface->callbacks.action(host::FloatingToolbarAction::TogglePunctuation, "");
  assert(actions.size() == 1 && actions[0].first == &second);
  // 失焦照 macOS 留着；前台全屏时收起；换了输入法时收起并放开属主，之后的按钮没人接。
  toolbar->focus_out(&second);
  assert(surface->shown);
  surface->fullscreen = true;
  surface->callbacks.focus_changed();
  assert(!surface->shown);
  surface->fullscreen = false;
  surface->callbacks.focus_changed();
  assert(surface->shown);
  toolbar->deactivate(&first);
  assert(surface->shown);
  toolbar->deactivate(&second);
  assert(!surface->shown && !toolbar->owner());
  surface->callbacks.action(host::FloatingToolbarAction::OpenSettings, "");
  assert(actions.size() == 1);
  // 关掉开关就收起；再打开算一次输入，立刻回来。
  toolbar->activate(&first, state);
  auto disabled = host::floating_toolbar_config(Json{{"floating_toolbar", {{"enabled", false}}}}, true, true);
  toolbar->configure(disabled, host::FloatingToolbarColors{});
  assert(!surface->shown);
  toolbar->configure(host::floating_toolbar_config(Json::object(), true, true), host::FloatingToolbarColors{});
  assert(surface->shown);
  // 拖动后的位置写进状态目录，下一次显示和下一个进程都从那里恢复。
  surface->callbacks.moved({300, 400});
  toolbar->deactivate(&first);
  toolbar->activate(&first, state);
  assert(surface->last_saved && surface->last_saved->x == 300 && surface->last_saved->y == 400);
  std::ifstream stream(*options.position_file);
  std::string text((std::istreambuf_iterator<char>(stream)), std::istreambuf_iterator<char>());
  assert(host::parse_floating_toolbar_position(text)->x == 300);
  FakeSurface *reloaded_surface = nullptr;
  auto reloaded = host::FloatingToolbar::create(loop.loop(), {}, options, [&](host::FloatingToolbarSurface::Callbacks callbacks) {
    auto created = std::make_unique<FakeSurface>(std::move(callbacks));
    reloaded_surface = created.get();
    return created;
  });
  reloaded->configure(host::floating_toolbar_config(Json::object(), true, true), host::FloatingToolbarColors{});
  reloaded->activate(&first, state);
  assert(reloaded_surface->last_saved && reloaded_surface->last_saved->y == 400);

  // 切换类的动作只对有焦点的属主生效的宿主（IBus）：属主失焦后工具栏留着，但交给后端的内容说这些按钮不能用；获得焦点后恢复。不设这一项的宿主（Fcitx5）失焦后照样能用。
  assert(surface->last.focus_actions);
  toolbar->focus_out(&first);
  assert(surface->shown && surface->last.focus_actions);
  FakeSurface *gated_surface = nullptr;
  auto gated_options = options;
  gated_options.actions_need_focus = true;
  gated_options.position_file.reset();
  auto gated = host::FloatingToolbar::create(loop.loop(), {}, gated_options, [&](host::FloatingToolbarSurface::Callbacks callbacks) {
    auto created = std::make_unique<FakeSurface>(std::move(callbacks));
    gated_surface = created.get();
    return created;
  });
  gated->configure(host::floating_toolbar_config(Json::object(), true, true), host::FloatingToolbarColors{});
  gated->activate(&first, state);
  assert(gated_surface->shown && gated_surface->last.focus_actions);
  gated->focus_out(&first);
  assert(gated_surface->shown && !gated_surface->last.focus_actions);
  gated->activate(&first, state);
  assert(gated_surface->last.focus_actions);

  // Wayland 会话里失焦就收起，获得焦点再回来。
  FakeSurface *wayland_surface = nullptr;
  options.hide_on_focus_out = true;
  options.position_file.reset();
  auto wayland = host::FloatingToolbar::create(loop.loop(), {}, options, [&](host::FloatingToolbarSurface::Callbacks callbacks) {
    auto created = std::make_unique<FakeSurface>(std::move(callbacks));
    wayland_surface = created.get();
    return created;
  });
  wayland->configure(host::floating_toolbar_config(Json::object(), true, true), host::FloatingToolbarColors{});
  wayland->activate(&first, state);
  assert(wayland_surface->shown);
  wayland->focus_out(&first);
  assert(!wayland_surface->shown);
  wayland->activate(&first, state);
  assert(wayland_surface->shown);

  // 没有显示后端时不建工具栏，宿主只用菜单。
  assert(!host::FloatingToolbar::create(loop.loop(), {}, options,
                                        [](host::FloatingToolbarSurface::Callbacks) { return nullptr; }));
  std::filesystem::remove_all(directory);
}

}  // namespace

int main() {
  keymap_rules();
  toolbar_config();
  toolbar_faces();
  toolbar_geometry();
  colors();
  controller();
  return 0;
}
