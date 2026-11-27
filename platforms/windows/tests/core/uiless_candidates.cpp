#include "UiLessCandidateSync.h"
#include "uiless_page.h"
#include "PipeMetadata.h"
#include <stdexcept>
using namespace msime::windows;
void require(bool value) {
  if (!value)
    throw std::runtime_error("UI-less 候选同步失败");
}
std::u16string payload(const EncodedReply &reply) {
  require(static_cast<bool>(reply));
  std::u16string result;
  for (auto unit : reply.packet.candidate_string) {
    if (!unit)
      break;
    result += static_cast<char16_t>(unit);
  }
  return result;
}
std::wstring frame_text(const std::vector<uint8_t> &frame) {
  std::wstring text;
  for (size_t n = 4; n + 1 < frame.size(); n += 2) {
    const auto unit = static_cast<wchar_t>(frame[n] | (frame[n + 1] << 8));
    if (!unit)
      break;
    text += unit;
  }
  return text;
}
int main() {
  const auto tagged = uiless_reply(27, "ni hao", {"你好", "拟好"}, 1, 42);
  require(payload(tagged) == u"ni hao\t你好,拟好\t1\t42");
  const auto parsed = FanyImeUiLessPage::Parse(L"ni hao\t你好,拟好\t1\t42");
  require(parsed.hasPage && parsed.preedit == L"ni hao" &&
          parsed.page == L"你好,拟好" && parsed.selection == 1 &&
          parsed.generation == 42);
  const auto legacy = FanyImeUiLessPage::Parse(L"ni\t你,拟\t1");
  require(legacy.hasPage && legacy.selection == 1 && legacy.generation == 0);
  const auto preeditOnly = FanyImeUiLessPage::Parse(L"ni");
  require(!preeditOnly.hasPage && preeditOnly.preedit == L"ni");
  require(FanyImeUiLessPage::Parse(L"ni\t你,拟").hasPage);
  for (const auto malformed :
       {L"ni\t你\t-1\t42", L"ni\t你\t0\t-1", L"ni\t你\t0\t18446744073709551616",
        L"ni\t你\t0\t42junk", L"ni\t你\t2147483648\t1"})
    require(!FanyImeUiLessPage::Parse(malformed).hasPage);
  require(!FanyImeUiLessPage::IsCurrent(41, 42));
  require(FanyImeUiLessPage::IsCurrent(42, 42) &&
          FanyImeUiLessPage::IsCurrent(43, 42));
  require(FanyImeUiLessPage::IsCurrent(0, 42));
  const auto capabilities = FanyImeProtocol::Capabilities | FanyImeProtocol::UiLessCandidates;
  const auto hello = FanyImeProtocol::Hello(42, 1, capabilities);
  require(!(FanyImeProtocol::Negotiate(hello).capabilities & FanyImeProtocol::UiLessCandidates));
  require(FanyImeProtocol::Negotiate(hello, capabilities).capabilities & FanyImeProtocol::UiLessCandidates);
  require(PipeMetadata::key_modifiers(FanyImePipeFlags::UiLess |
              FanyImePipeFlags::ServerUiLessCandidates | PipeMetadata::CandidateActive | 3u) == 3u);

  {
    // 同一份已确认回复之后，云候选/AI/重排只能更新它所属的 UI-less 页。
    UiLessCandidateSync sync;
    FocusLease lease{PipeTicket{42, {1, 2, 3}}, 7, 9};
    lease.transport.capabilities |= FanyImeProtocol::UiLessCandidates;
    nlohmann::json view = {{"session", 12},
                           {"generation", 20},
                           {"focused", true},
                           {"editing_text", "ni"},
                           {"preedit", "ni"},
                           {"candidates",
                            {{{"text", "你"}, {"highlighted", true}},
                             {{"text", "拟"}, {"highlighted", false}}}}};
    PendingReply reply;
    reply.source.transition = {{"view", view}};
    reply.next_prefix = "好";
    FanyImeNamedpipeData packet{};
    require(!sync.updated(lease, view));
    sync.delivered(lease, reply, packet);
    view["generation"] = 21;
    require(!sync.updated(lease, view));
    packet.modifiers_down = FanyImePipeFlags::UiLess;
    sync.delivered(lease, reply, packet);
    require(!sync.updated(lease, view));
    packet.modifiers_down |= FanyImePipeFlags::ServerUiLessCandidates;
    auto oldLease = lease;
    oldLease.transport.capabilities &= ~FanyImeProtocol::UiLessCandidates;
    sync.delivered(oldLease, reply, packet);
    require(!sync.updated(oldLease, view));
    sync.delivered(lease, reply, packet);
    auto stale = lease;
    ++stale.token;
    require(!sync.updated(stale, view));
    stale = lease;
    ++stale.transport.generations[0];
    require(!sync.updated(stale, view));
    auto other = view;
    other["session"] = 13;
    require(!sync.updated(lease, other));
    other = view;
    other["generation"] = 19;
    require(!sync.updated(lease, other));
    other = view;
    other["editing_text"] = "";
    require(!sync.updated(lease, other));
    auto frame = sync.updated(lease, view);
    require(frame &&
            frame->size() == sizeof(FanyImeNamedpipeDataToTsfWorkerThread));
    require((*frame)[0] == FanyImeWorkerReplyType::UiLessCandidatesChanged);
    std::wstring text;
    for (size_t n = 4; n + 1 < frame->size(); n += 2) {
      const auto unit =
          static_cast<wchar_t>((*frame)[n] | ((*frame)[n + 1] << 8));
      if (!unit)
        break;
      text += unit;
    }
    const auto changed = FanyImeUiLessPage::Parse(text);
    require(changed.hasPage && changed.preedit == L"好ni" &&
            changed.page == L"你,拟" && changed.generation == 21);
    require(!sync.updated(lease, view));
    view["generation"] = 22;
    view["candidates"][0]["text"] = std::string(199, 'a');
    const auto oversized = sync.updated(lease, view);
    require(oversized && oversized->empty());
    reply.source.transition["view"]["editing_text"] = "";
    sync.delivered(lease, reply, packet);
    view["candidates"][0]["text"] = "你";
    require(!sync.updated(lease, view));
  }
  {
    // 云的占位席位不出现在 UI-less 页面上：宿主自己画的列表里不再有「☁」方框，编号也和数字键数的一样。
    UiLessCandidateSync sync;
    FocusLease lease{PipeTicket{43, {4, 5, 6}}, 7, 9};
    lease.transport.capabilities |= FanyImeProtocol::UiLessCandidates;
    nlohmann::json view = {{"session", 30},
                           {"generation", 40},
                           {"focused", true},
                           {"editing_text", "ni"},
                           {"preedit", "ni"},
                           {"candidates", {{{"text", "你"}, {"highlighted", true}, {"source", 0}}}}};
    PendingReply reply;
    reply.source.transition = {{"view", view}};
    FanyImeNamedpipeData packet{};
    packet.modifiers_down = FanyImePipeFlags::UiLess | FanyImePipeFlags::ServerUiLessCandidates;
    sync.delivered(lease, reply, packet);
    view["generation"] = 41;
    view["candidates"] = {{{"text", "你"}, {"highlighted", true}, {"source", 0}},
                          {{"text", "☁"}, {"highlighted", false}, {"source", 15}},
                          {{"text", "好"}, {"highlighted", false}, {"source", 0}}};
    auto frame = sync.updated(lease, view);
    require(frame && frame->size() == sizeof(FanyImeNamedpipeDataToTsfWorkerThread));
    const auto first = FanyImeUiLessPage::Parse(frame_text(*frame));
    require(first.hasPage && first.preedit == L"ni" && first.page == L"你,好" &&
            first.selection == 0 && first.generation == 41);
    // 高亮停在占位席位上时不发这一帧：宿主没有这一行可以画，最旧一页保持不变。
    view["generation"] = 42;
    view["candidates"][0]["highlighted"] = false;
    view["candidates"][1]["highlighted"] = true;
    require(!sync.updated(lease, view));
    // 高亮移到占位席位后面的可见行时，选中编号按看得见的行算，是 1 而不是引擎下标 2。
    view["generation"] = 43;
    view["candidates"][1]["highlighted"] = false;
    view["candidates"][2]["highlighted"] = true;
    frame = sync.updated(lease, view);
    require(frame && frame->size() == sizeof(FanyImeNamedpipeDataToTsfWorkerThread));
    const auto second = FanyImeUiLessPage::Parse(frame_text(*frame));
    require(second.hasPage && second.page == L"你,好" && second.selection == 1 &&
            second.generation == 43);
    // 整页只剩占位席位时发空页，宿主清掉旧列表，不留下一行选不中的占位。
    view["generation"] = 44;
    view["candidates"] = {{{"text", "☁"}, {"highlighted", false}, {"source", 15}}};
    frame = sync.updated(lease, view);
    require(frame && frame->size() == sizeof(FanyImeNamedpipeDataToTsfWorkerThread));
    const auto empty = FanyImeUiLessPage::Parse(frame_text(*frame));
    require(empty.hasPage && empty.page.empty() && empty.generation == 44);
  }
  return 0;
}
