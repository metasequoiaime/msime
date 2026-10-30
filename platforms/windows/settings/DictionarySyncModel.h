#pragma once
#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

// 「词库同步」（「我的词条」经 WebDAV 网盘同步，docs/dictionary-webdav-sync.md）在 Windows 两个设置窗口里的展示规则：错误码对应的说明、概况的几行字、设备按编号合并和分组、确认对话框的文字。文字与共享设置页（packages/ui/src/settings/dictionary-sync.ts）一致。同步、额度、退避和设备管理都在共享 Rust（`msime_client_dictionary_sync`），这里只管怎么说。不依赖 Windows、WinRT 和 JSON 头文件，WinUI（main.cpp）和 Windows 7 面板（win7/DictionarySyncForm.h）共用，主机上的单元测试（tests/ui/dictionary_sync_model.cpp）直接测它。
namespace msime::settings::dictionary_sync {

// 共享页的 dictionarySyncErrorMessage。`probe` 为真时是「测试连接」的结果：`dictionary_sync_unsupported` 在那里说的是网盘不合格，在同步时说的是网盘上的格式比本机新。不认识的码给通用说明。
inline std::wstring error_message(std::wstring_view code, bool probe = false) {
  if (code == L"dictionary_sync_not_configured")
    return L"地址、账号或应用密码不完整。";
  if (code == L"invalid_dictionary_sync")
    return L"请求无效，请关闭设置后重新打开再试。";
  if (code == L"dictionary_sync_unauthorized")
    return L"网盘拒绝了这个账号或应用密码。";
  if (code == L"dictionary_sync_unavailable")
    return L"连不上网盘，请检查地址和网络；证书须由公网信任的机构签发。";
  if (code == L"dictionary_sync_rate_limited")
    return L"网盘暂时限制了请求次数，或本机这 30 分钟的同步次数已用完，请稍后再试。";
  if (code == L"dictionary_sync_too_large")
    return L"网盘上的词库或本机的同步记录超过了上限，删减一些词条后再试。";
  if (code == L"dictionary_sync_rollback")
    return L"网盘上的词库比本机读到过的旧，或者被删改过。如果是你自己清理了网盘上的文件夹，请确认网盘现状并恢复同步。本机词条保留，但未上传的删除可能重新合并回来。";
  if (code == L"dictionary_sync_unsupported")
    return probe ? L"这个网盘不支持词库同步需要的文件操作（独占创建、列目录），请换一个 WebDAV 服务或文件夹。"
                 : L"网盘上的词库是更新版本的格式，请先更新水杉输入法。";
  if (code == L"dictionary_sync_corrupt")
    return L"网盘上的词库记录损坏了，自动同步已暂停。";
  if (code == L"dictionary_sync_storage")
    return L"本机的同步记录读写失败。";
  if (code == L"clipboard_device_pending")
    return L"本设备还在等待允许。";
  if (code == L"not_found")
    return L"这台设备已经不在列表里了，请刷新后再试。";
  if (code == L"clipboard_key_conflict" || code == L"clipboard_keystore_conflict")
    return L"另一台设备同时修改了密钥，请稍后再试。";
  if (code == L"clipboard_device_limit")
    return L"设备数已达上限，请先移除不再使用的设备。";
  return probe ? L"测试连接失败，请稍后再试。" : L"同步失败，请稍后再试。";
}

// 「测试连接」通过、只测不存时的说明。
inline constexpr std::wstring_view probe_passed = L"连接正常，可以保存";
// 「保存」时先测试，通过并写进偏好之后的说明。
inline constexpr std::wstring_view saved = L"连接正常，已保存词库同步的网盘配置。";

// 组首的说明：同步什么、网盘看得到什么、怎么批准、请求额度。
inline constexpr std::wstring_view explanation =
    L"把「我的词条」（包括打字时自动记下的词）和它们的删除同步到你自己的 WebDAV 网盘，不同步打字学到的权重、候选顺序和主词库。词条在本机加密，网盘只保存密文；新设备要在一台已获准的设备上核对核对码后允许，与云剪贴板分开批准。坚果云免费账号每 30 分钟 600 次请求与其他应用共用；移除设备后请在网盘上更换应用密码。";

// 服务器地址一行的说明：客户端在这个文件夹下自己建 msime-dictionary，与云剪贴板的 msime-clipboard 互不相干。
inline constexpr std::wstring_view url_note =
    L"坚果云填 https://dav.jianguoyun.com/dav/ 即可，水杉会在里面自动建 msime-dictionary 文件夹，不用自己先建；其他网盘填它给的 WebDAV 地址。使用 HTTPS 地址，证书要由公网信任的机构签发；仅本机回环地址允许 HTTP。";

// 已保存时配置组末的一句：设备身份绑定地址和账号。
inline std::wstring saved_note(std::wstring_view url) {
  return L"「我的词条」现在同步到 WebDAV 网盘 " + std::wstring(url) +
         L"。换了地址或账号后，本机要重新登记，并在其他设备上重新允许；换应用密码不用。";
}

// `status`（也是 `run` 结果里的 status）里宿主要显示的几项。还没用过网盘时 device_id 和 code 为空。
struct Status {
  std::wstring device_id;
  std::wstring code;
  bool approved = false;
  bool joined = false;
  uint64_t words = 0;
  uint64_t pending_upload = 0;
  uint64_t pending_apply = 0;
  uint64_t held_deletions = 0;
  bool clock_skew = false;
  uint64_t last_success_ms = 0;
  std::wstring last_error;
  // 自动同步因请求额度在等时什么时候继续（Unix 毫秒）：本机额度不够一轮，或者被网盘限流之后；没在等时（以及旧版本的 Rust 不给这个字段时）为 0。「立即同步」不受它限制。
  uint64_t budget_wait_until = 0;
};

// 两次读到的概况是否完全相同：窗口重新获得焦点时重读一次，相同就不重画。
inline bool operator==(const Status &a, const Status &b) {
  return a.device_id == b.device_id && a.code == b.code && a.approved == b.approved && a.joined == b.joined &&
         a.words == b.words && a.pending_upload == b.pending_upload && a.pending_apply == b.pending_apply &&
         a.held_deletions == b.held_deletions && a.clock_skew == b.clock_skew &&
         a.last_success_ms == b.last_success_ms && a.last_error == b.last_error &&
         a.budget_wait_until == b.budget_wait_until;
}

inline bool operator!=(const Status &a, const Status &b) { return !(a == b); }

// 焦点读取不占普通操作的忙状态：每次读取只接纳最新一次，普通操作开始就作废此前的读取。只在 UI 线程使用，页面和配置的生命周期仍由宿主原有的代次管理。
class StatusReadOrder {
public:
  uint64_t begin_read() { return ++sequence_; }
  void invalidate() { ++sequence_; }
  bool accepts(uint64_t sequence) const { return sequence == sequence_; }

private:
  uint64_t sequence_ = 0;
};

// 本机是否获准的一句。
inline std::wstring approval_text(const Status &status) {
  if (status.device_id.empty())
    return L"还没有连接过网盘，点「立即同步」开始。";
  if (status.approved)
    return L"本设备已获准，可以与其他设备同步。";
  return L"本设备还在等待允许：在一台已获准的设备上打开「词库同步」，确认那里显示的本设备核对码与这里完全一致后点「允许」，再回到这里点「立即同步」。";
}

// 词条和待处理的数量：「我的词条」N 个，待发送、待写入本机各几个（为 0 时不写）。
inline std::wstring counts_text(const Status &status) {
  std::wstring text = L"「我的词条」" + std::to_wstring(status.words) + L" 个";
  if (status.pending_upload)
    text += L"；待发送 " + std::to_wstring(status.pending_upload) + L" 个";
  if (status.pending_apply)
    text += L"；待写入本机 " + std::to_wstring(status.pending_apply) + L" 个";
  return text + L"。";
}

// 本机时区里的年月日时分。宿主把 Unix 毫秒换成本地时间后交给 budget_wait_text，这里不碰时区。
struct LocalMinute {
  int year = 0;
  int month = 0;
  int day = 0;
  int hour = 0;
  int minute = 0;
};

// 自动同步在等额度时概况里的一句：`until` 的「HH:MM」，与 `now` 不在同一天时写成「M月D日 HH:MM」；没在等（`until_ms` 为 0）或已经过了（不晚于 `now_ms`）时为空。与共享设置页的 dictionarySyncBudgetWaitMessage、iOS 和 Android 逐字相同。
inline std::wstring budget_wait_text(uint64_t until_ms, uint64_t now_ms, const LocalMinute &until, const LocalMinute &now) {
  if (!until_ms || until_ms <= now_ms)
    return {};
  const auto two_digits = [](int value) { return (value < 10 ? L"0" : L"") + std::to_wstring(value); };
  std::wstring when = two_digits(until.hour) + L":" + two_digits(until.minute);
  if (until.year != now.year || until.month != now.month || until.day != now.day)
    when = std::to_wstring(until.month) + L"月" + std::to_wstring(until.day) + L"日 " + when;
  return L"额度用完，将于 " + when + L" 继续自动同步；可点「立即同步」。";
}

// 本机时钟落后其他设备一天以上时的提示（合并照常）。
inline constexpr std::wstring_view clock_skew_note = L"其他设备的时间比本机快一天以上，请检查这台电脑的日期和时间。";

// 被挡下的大批删除：本机一次少了这么多词，等用户在「同步删除」和「从网盘恢复」之间选。
inline std::wstring held_deletions_text(uint64_t count) {
  return L"本机一次少了 " + std::to_wstring(count) +
         L" 个词条，像是词库被重置或损坏了，这些删除先没有同步出去。选「同步删除」会在所有设备上删掉它们；选「从网盘恢复」把它们写回本机。";
}

inline constexpr std::wstring_view confirm_deletions_question = L"在所有设备上删除这些词条？此操作无法撤销。";

// 手动同步成功后的一句：本机还没获准时什么都不说（概况里已经写着在等待允许）；输入法正占着词库时说明网盘上的改动稍后写进来。
inline std::wstring run_notice(bool approved, bool deferred) {
  if (!approved)
    return {};
  if (deferred)
    return L"已同步。输入法正在使用词库，网盘上的改动稍后写入本机。";
  return L"已同步。";
}

inline constexpr std::wstring_view restarted = L"已以本机的词条重新开始，其他设备获准后会把它们的词合并进来。";

// 设备管理的一台设备，形状与云剪贴板的 devices 相同。
struct Device {
  std::wstring id;
  std::wstring name;
  std::wstring created_at;
  std::wstring code;
  bool approved = false;
  bool current = false;
};

struct DeviceGroups {
  // 已获准的设备，含本机。
  std::vector<Device> approved;
  // 等待本机允许的其他设备。
  std::vector<Device> waiting;
};

// 共享页的 dictionarySyncDeviceGroups：按设备编号每台只留一条，同一台设备的几条登记合成一条（获准、本机任一条是就算是，名称取最后一条非空的），保持第一次出现的顺序；再分成已获准（含本机）和等待允许两组。
inline DeviceGroups group_devices(const std::vector<Device> &devices) {
  std::vector<Device> merged;
  for (const auto &device : devices) {
    bool found = false;
    for (auto &seen : merged) {
      if (seen.id != device.id)
        continue;
      if (!device.name.empty())
        seen.name = device.name;
      seen.approved = seen.approved || device.approved;
      seen.current = seen.current || device.current;
      found = true;
      break;
    }
    if (!found)
      merged.push_back(device);
  }
  DeviceGroups groups;
  for (auto &device : merged)
    (device.approved || device.current ? groups.approved : groups.waiting).push_back(std::move(device));
  return groups;
}

// 设备的显示名：没有名称时写「未命名设备」，本机加「（本设备）」。
inline std::wstring device_label(const Device &device) {
  std::wstring label = device.name.empty() ? std::wstring(L"未命名设备") : device.name;
  if (device.current)
    label += L"（本设备）";
  return label;
}

// 设备一行的说明：核对码和登记时间。
inline std::wstring device_detail(const Device &device) {
  std::wstring detail = L"核对码 " + device.code;
  if (!device.created_at.empty())
    detail += L" · 登记于 " + device.created_at;
  return detail;
}

inline constexpr std::wstring_view waiting_note =
    L"核对两台设备上的核对码一致再允许；不认识的设备请拒绝。允许后，这台设备就能读取和上传「我的词条」。";

inline std::wstring approve_question(const Device &device) {
  return L"允许「" + device_label(device) + L"」同步「我的词条」？\n核对码：" + device.code +
         L"\n请确认与那台设备上显示的核对码完全一致。";
}

inline std::wstring reject_question(const Device &device) {
  return L"拒绝「" + device_label(device) + L"」？它将不能读取「我的词条」。";
}

inline std::wstring remove_question(const Device &device) {
  return L"移除「" + device_label(device) + L"」？将更换密钥，其他设备需要重新允许一次；移除之后请在网盘上更换应用密码。";
}

inline constexpr std::wstring_view reset_question =
    L"重置词库同步的密钥？新密钥只给本设备，其他设备都要重新允许一次。网盘上的词条不会被删除。";

inline constexpr std::wstring_view trust_remote_question =
    L"仅在你自己清理过网盘文件夹时使用。保留当前本机词条，但忘掉同步位置和待发改动；未上传的删除可能从网盘重新合并回来。";

inline constexpr std::wstring_view restart_question =
    L"以本机为准重新开始？网盘上的共享词库换成本机现在的词条，其他设备获准后会把它们的词合并进来，不会因此删词。只在持有旧密钥的设备都不在了时使用。";

inline constexpr std::wstring_view clear_question =
    L"停止词库同步？本机不再连接这个网盘，网盘上的内容和设备登记不会被删除。";

} // namespace msime::settings::dictionary_sync
