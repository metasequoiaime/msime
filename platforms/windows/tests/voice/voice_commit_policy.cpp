#include "VoiceCommitPolicy.h"

#include <cstdint>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("voice commit policy failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)

constexpr std::uint32_t kTarget = 4242;
constexpr std::uint32_t kServer = 7;
} // namespace

int main() {
  try {
    // commit_mode 的取值，未知值和空值都按 tsf。
    REQUIRE(voice_commit_route("tsf") == VoiceCommitRoute::Tsf);
    REQUIRE(voice_commit_route("sendinput") == VoiceCommitRoute::SendInput);
    REQUIRE(voice_commit_route("ctrl_v") == VoiceCommitRoute::CtrlV);
    REQUIRE(voice_commit_route("") == VoiceCommitRoute::Tsf);
    REQUIRE(voice_commit_route("clipboard") == VoiceCommitRoute::Tsf);

    // 模拟按键和粘贴只投给录音时那个 TSF 客户端进程：前台换成别的进程、目标未知或是 Server 自己时都不投。
    REQUIRE(voice_target_in_foreground(kTarget, kTarget, kServer, false));
    REQUIRE(!voice_target_in_foreground(kTarget, kTarget + 1, kServer, false));
    REQUIRE(!voice_target_in_foreground(kTarget, 0, kServer, false));
    REQUIRE(!voice_target_in_foreground(0, 0, kServer, false));
    REQUIRE(!voice_target_in_foreground(kServer, kServer, kServer, false));
    // 前台窗口仍是开始录音时那一个就投递，哪怕它属于另一个进程（UWP 的 ApplicationFrameHost、传统控制台）；但前台是 Server 自己或没有前台窗口时仍然不投。
    REQUIRE(voice_target_in_foreground(kTarget, kTarget + 1, kServer, true));
    REQUIRE(voice_target_in_foreground(0, kTarget + 1, kServer, true));
    REQUIRE(!voice_target_in_foreground(kTarget, kServer, kServer, true));
    REQUIRE(!voice_target_in_foreground(kTarget, 0, kServer, true));

    // 投递时前台对得上还不够，焦点租约也必须仍是当前焦点：同一窗口里换了输入框，前台核对看不出来。
    REQUIRE(voice_delivery_target_current(true, true));
    REQUIRE(!voice_delivery_target_current(true, false));
    REQUIRE(!voice_delivery_target_current(false, true));
    REQUIRE(!voice_delivery_target_current(false, false));
    // 事务锁忙时 TSF 路线不验租约就返回，退路要靠租约核对拦住换了输入框的情况。
    REQUIRE(!voice_tsf_refusal_falls_back(false, false, true, false));
    REQUIRE(voice_tsf_refusal_falls_back(false, false, true, true));
    REQUIRE(!voice_tsf_refusal_falls_back(false, false, false, true));

    // TSF 被拒：焦点租约失效时整段丢弃，哪怕前台看起来还是同一个进程（焦点去了同一应用的另一个输入框）。
    REQUIRE(!voice_tsf_refusal_falls_back(true, false, true, true));
    REQUIRE(!voice_tsf_refusal_falls_back(true, false, false, false));

    // 管道写失败：Server 自己作废了租约并停下控制器，此刻租约核对必然失败（lease_current 为 false），前台仍是目标就照样退回 SendInput，不丢整段录音；前台换了仍然丢弃。
    REQUIRE(voice_tsf_refusal_falls_back(false, true, true, false));
    REQUIRE(!voice_tsf_refusal_falls_back(false, true, false, false));

    // 原生语音在录音或识别中失焦就取消；租约仍有效、没有进行中的会话或面板审阅录音都不在这里取消。
    REQUIRE(voice_focus_loss_cancels(true, true, false));
    REQUIRE(!voice_focus_loss_cancels(true, true, true));
    REQUIRE(!voice_focus_loss_cancels(true, false, false));
    REQUIRE(!voice_focus_loss_cancels(false, true, false));

    // ctrl_v 写剪贴板时带上的三个排除标记，名字必须与 Windows 约定逐字一致，值都是 0。
    REQUIRE(std::size(voice_clipboard_markers) == 3);
    REQUIRE(voice_clipboard_markers[0].format == L"ExcludeClipboardContentFromMonitorProcessing");
    REQUIRE(voice_clipboard_markers[1].format == L"CanIncludeInClipboardHistory");
    REQUIRE(voice_clipboard_markers[2].format == L"CanUploadToCloudClipboard");
    for (const auto &marker : voice_clipboard_markers)
      REQUIRE(marker.value == 0);
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
