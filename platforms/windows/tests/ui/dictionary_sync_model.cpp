#include "../../settings/DictionarySyncModel.h"
#include "../../common/DictionarySyncRetry.h"
#include <cstdio>
#include <string>
#include <vector>

using namespace msime::settings::dictionary_sync;
namespace retry = msime::windows::dictionary_sync;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
Device device(std::wstring id, std::wstring name, bool approved, bool current) {
  Device value;
  value.id = std::move(id);
  value.name = std::move(name);
  value.code = L"0123 4567 8901 2345";
  value.approved = approved;
  value.current = current;
  return value;
}
} // namespace

int main() {
  // 错误码与共享页一致；「测试连接」和同步对 unsupported 的说法不同，不认识的码给通用说明。
  check(error_message(L"dictionary_sync_unauthorized") == L"网盘拒绝了这个账号或应用密码。", "unauthorized");
  check(error_message(L"dictionary_sync_unavailable") ==
            L"连不上网盘，请检查地址和网络；证书须由公网信任的机构签发。",
        "unavailable");
  check(error_message(L"dictionary_sync_unsupported", true).find(L"独占创建") != std::wstring::npos,
        "probe unsupported names the WebDAV operations");
  check(error_message(L"dictionary_sync_unsupported").find(L"更新水杉输入法") != std::wstring::npos,
        "sync unsupported asks for an update");
  check(error_message(L"clipboard_keystore_conflict") == error_message(L"clipboard_key_conflict"),
        "both key ring conflicts read the same");
  check(error_message(L"dictionary_sync_rollback").find(L"确认网盘现状并恢复同步") != std::wstring::npos,
        "rollback explains the recovery");
  check(error_message(L"something_new") == L"同步失败，请稍后再试。", "unknown sync code");
  check(error_message(L"something_new", true) == L"测试连接失败，请稍后再试。", "unknown probe code");
  check(error_message(L"") == L"同步失败，请稍后再试。", "empty code");

  // 概况：还没用过网盘、等待允许和已获准三种说法；数量为 0 的项不写。
  Status status;
  check(approval_text(status).find(L"立即同步") != std::wstring::npos, "never connected");
  status.device_id = std::wstring(32, L'a');
  status.code = L"0123 4567 8901 2345";
  check(approval_text(status).find(L"等待允许") != std::wstring::npos, "pending device");
  status.approved = true;
  check(approval_text(status) == L"本设备已获准，可以与其他设备同步。", "approved device");
  status.words = 12;
  check(counts_text(status) == L"「我的词条」12 个。", "counts without pending work");
  status.pending_upload = 3;
  status.pending_apply = 4;
  check(counts_text(status) == L"「我的词条」12 个；待发送 3 个；待写入本机 4 个。", "counts with pending work");
  check(held_deletions_text(60).find(L"少了 60 个词条") != std::wstring::npos, "held deletions count");
  check(saved_note(L"https://dav.example.test/").find(L"https://dav.example.test/") != std::wstring::npos,
        "saved note names the address");

  // 手动同步的结果：未获准时不说话，输入法占着词库时说明稍后写入。
  check(run_notice(false, false).empty(), "pending device run says nothing");
  check(run_notice(true, false) == L"已同步。", "plain success");
  check(run_notice(true, true).find(L"稍后写入本机") != std::wstring::npos, "deferred apply");

  // 同一台设备的几条登记合成一条：获准、本机任一条是就算是，名称取最后一条非空的；顺序按第一次出现。
  const auto groups = group_devices({
      device(L"b", L"旧名字", false, false),
      device(L"a", L"本机", true, true),
      device(L"c", L"等待的电脑", false, false),
      device(L"b", L"新名字", true, false),
      device(L"c", L"", false, false),
  });
  check(groups.approved.size() == 2 && groups.waiting.size() == 1, "one entry per device");
  check(groups.approved[0].id == L"b" && groups.approved[0].name == L"新名字" && groups.approved[0].approved,
        "merged registration is approved and renamed");
  check(groups.approved[1].id == L"a" && groups.approved[1].current, "current device stays in the approved group");
  check(groups.waiting[0].id == L"c" && groups.waiting[0].name == L"等待的电脑", "empty name keeps the earlier one");
  check(device_label(groups.approved[1]) == L"本机（本设备）", "current device label");
  check(device_label(device(L"d", L"", false, false)) == L"未命名设备", "unnamed device label");
  check(approve_question(groups.waiting[0]).find(L"0123 4567 8901 2345") != std::wstring::npos,
        "approval question shows the code");
  check(device_detail(groups.waiting[0]) == L"核对码 0123 4567 8901 2345", "device detail without a date");

  // 窗口重新获得焦点时重读的概况：每一项都参与比较，完全相同才算没变（不重画）。
  Status before;
  before.device_id = L"a";
  before.approved = true;
  before.joined = true;
  before.words = 12;
  before.pending_upload = 1;
  before.last_success_ms = 1000;
  Status after = before;
  check(after == before && !(after != before), "an identical status is unchanged");
  after.pending_upload = 0;
  check(after != before, "a sent word changes the status");
  after = before;
  after.last_success_ms = 2000;
  check(after != before, "a later sync changes the status");
  after = before;
  after.last_error = L"dictionary_sync_unavailable";
  check(after != before, "a new error changes the status");
  after = before;
  after.approved = false;
  check(after != before, "approval changes the status");
  after = before;
  after.budget_wait_until = 3000;
  check(after != before, "a budget wait changes the status");

  // 自动同步在等额度时说什么时候继续：今天之内只写时分，跨了天写上月日；没在等或已经过了时不说。本地时间由宿主换好交进来。
  const LocalMinute resumes{2026, 10, 28, 9, 5};
  const LocalMinute morning{2026, 10, 28, 8, 40};
  const LocalMinute late_night{2026, 10, 27, 23, 50};
  const uint64_t until = 1'800'000'000'000ULL;
  check(budget_wait_text(until, until - 25 * 60 * 1000, resumes, morning) ==
            L"额度用完，将于 09:05 继续自动同步；可点「立即同步」。",
        "a wait later today names the time");
  check(budget_wait_text(until, until - 9 * 60 * 60 * 1000, resumes, late_night) ==
            L"额度用完，将于 10月28日 09:05 继续自动同步；可点「立即同步」。",
        "a wait on another day names the date");
  check(budget_wait_text(until, until, resumes, resumes).empty(), "a wait ending now is not shown");
  check(budget_wait_text(until, until + 60'000, resumes, resumes).empty(), "a past wait is not shown");
  check(budget_wait_text(0, until, resumes, morning).empty(), "no wait means no text");
  check(budget_wait_text(until, until - 60'000, LocalMinute{2026, 1, 2, 0, 7}, LocalMinute{2026, 1, 1, 23, 59}) ==
            L"额度用完，将于 1月2日 00:07 继续自动同步；可点「立即同步」。",
        "single-digit month and day are not padded, hours and minutes are");

  // 密钥串身份冲突时重试，最多三次；别的错误和成功都不重试。
  int calls = 0;
  const auto conflicted = [](const std::string &code) { return code == retry::keystore_conflict; };
  auto answer = retry::with_keystore_retry([&] { ++calls; return std::string(calls < 2 ? "clipboard_keystore_conflict" : "ok"); }, conflicted);
  check(answer == "ok" && calls == 2, "a conflict is retried once it clears");
  calls = 0;
  answer = retry::with_keystore_retry([&] { ++calls; return std::string("clipboard_keystore_conflict"); }, conflicted);
  check(answer == "clipboard_keystore_conflict" && calls == retry::keystore_conflict_attempts,
        "a persistent conflict stops after the attempt budget");
  calls = 0;
  answer = retry::with_keystore_retry([&] { ++calls; return std::string("dictionary_sync_unavailable"); }, conflicted);
  check(answer == "dictionary_sync_unavailable" && calls == 1, "other failures are not retried");

  if (failures)
    return 1;
  std::puts("dictionary sync model: shared error texts, status lines, budget wait text, status comparison, one entry per device and keystore conflict retries passed");
  return 0;
}
