#pragma once

#include <cstdint>
#include <nlohmann/json.hpp>

namespace msime::linux_host {

// 上屏后联想（#9433，偏好 next_word_prediction）：选中一个词之后，运行时在没有组字的会话里给出后续词，View 以 prediction=true 标记它（msime_client.h），editing_text 为空，candidates 是建议。IBus 宿主和 Fcitx5 插件共用这里的判断。
//
// 六个平台同一条规则：联想显示时，数字键选当前页上对应的那一行，空格选高亮的那一行（首行），鼠标点选被点的那一行；回车和标点照常输入，先收起联想，标点仍按中文标点的正常路径出字；Esc 收起联想。宿主的按键路由处处把「有候选」当成「在组字」：逗号句号和减号等号翻页、以词定字、导航键结束组字、按住退格都会把建议当成组字。所以除了选词的数字、空格和 Esc，宿主处理的每个键先送 Cancel 收起建议，再按没有候选的状态往下走，而不是在每一个分支里另加例外。选词一律按渲染出来的那一行的 id 选（msime_client_select），上屏它并接着给出下一组联想。

// 视图是否是上屏后联想。普通状态省略 prediction 字段，宿主缺省按 false 读。与 view_is_composing 一样不抛异常：会话关掉后视图是 null。
inline bool view_is_prediction(const nlohmann::json &view) noexcept {
  if (!view.is_object())
    return false;
  const auto prediction = view.find("prediction");
  return prediction != view.end() && prediction->is_boolean() && prediction->get<bool>();
}

// 联想显示时这一键怎么办。
enum class PredictionKey {
  // 没有联想，照常处理。
  Route,
  // 选词：不带修饰键的空格，或者在当前页上有对应行的选词数字（数字选词开着、数字没超出这一页的行数）。宿主按这一行的 id 选它。
  Pick,
  // 先送 Cancel 收起建议，再按没有候选的状态照常处理这一键：回车、标点、字母、退格、方向键、翻页键、快捷键，以及不选词的数字，都交给它们平常的去处。
  DismissThenRoute,
  // 不带修饰键的 Esc：收起建议并吃掉这一键，和关掉一页组字候选一样，不交给应用。
  Close,
};

// picks 由宿主按自己的选词规则给出：这一键在一页普通候选上会不会选中一行（空格选高亮行，数字选对应行）。
inline PredictionKey prediction_key(bool prediction, bool bare_escape, bool picks) noexcept {
  if (!prediction)
    return PredictionKey::Route;
  if (bare_escape)
    return PredictionKey::Close;
  return picks ? PredictionKey::Pick : PredictionKey::DismissThenRoute;
}

// 宿主自己上屏之后紧跟着到达的 reset 不收起联想。
//
// IBus 和 Fcitx5 的 reset 表示客户端那边的光标或内容变了，宿主据此送 Cancel，鼠标点到文档别处因此会收起联想。有的客户端会把输入法上屏引起的光标移动也当成需要 reset 的变化回头 reset 一次；照常 Cancel 的话联想刚出来就被收掉。2026-11-02 在 Xvfb 里实测，GTK3（Mousepad，经 IBus 和 Fcitx5）和 GTK4（GNOME 文本编辑器，经 IBus）上屏之后都不发这种 reset，只在鼠标点击移动光标时发；Qt、浏览器和 Electron 没有测过。这一小段窗口防的是这类客户端：回声紧跟着上屏，经 D-Bus 来回一趟只要几毫秒到几十毫秒，而上屏之后再去点文档别处远长于这个窗口。窗口里收到按键（宿主处理按键时会先收起或选中联想）或者过了窗口，reset 照常处理。Android 宿主对自己延迟上屏的选区回声做的是同一件事。
class PredictionResetEcho {
 public:
  // 回声的窗口，毫秒。
  static constexpr int64_t kWindowMs = 250;

  // 宿主刚上屏了一段文字；commit_shows_prediction 是上屏之后的视图是不是联想。
  void committed(bool commit_shows_prediction, int64_t now_ms) noexcept {
    deadline_ms_ = commit_shows_prediction ? now_ms + kWindowMs : 0;
  }

  // 用户按了键，或者联想以别的方式结束了：之后的 reset 照常处理。
  void forget() noexcept { deadline_ms_ = 0; }

  // 收到 reset 时调用：返回 true 表示这是上屏的回声，联想留着。prediction 是此刻的视图是不是联想。
  bool keeps(bool prediction, int64_t now_ms) const noexcept {
    return prediction && deadline_ms_ != 0 && now_ms <= deadline_ms_;
  }

 private:
  int64_t deadline_ms_ = 0;
};

} // namespace msime::linux_host
