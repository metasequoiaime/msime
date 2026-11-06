#include "../../settings/SentencePackModel.h"
#include "../../settings/SettingsNavigation.h"
#include <cstdio>

using namespace msime::settings::sentence_pack;
namespace nav = msime::settings;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
bool shows_install(RowView const &view) { return !view.install_label.empty(); }
} // namespace

int main() {
  // 重画判断比的是 `current_page_`，它只会是 `pages` 里的 id；「输入」页的 id 是 "typing"，"input" 只是路由别名。
  check(nav::find_page(host_page) != nullptr, "host page is a real page id, not a route alias");
  check(nav::page_for_route("input") == host_page, "the input route opens the page that hosts the model rows");

  // 刚打开窗口：要读，读回之前不给「下载」。
  Catalog catalog;
  check(wants_read(catalog), "a fresh window reads the pack states");
  auto view = row_view(catalog.knowledge, L"missing", false, false, false, false);
  check(!shows_install(view) && !view.remove && !view.retry, "no download button before the states arrive");
  check(view.subtitle == L"正在读取模型状态…", "the row says it is reading");

  check(begin_read(catalog), "the first read is issued");
  check(!wants_read(catalog), "no second read while one is in flight");
  check(!finish_read(catalog, true), "a single read does not ask for another");
  check(catalog.knowledge == Knowledge::known && !wants_read(catalog), "a successful read is final until the page is entered again");

  // 读回后：装好的给「删除」，不给「下载」；没装的给「下载」；要更新的给「更新」。
  view = row_view(catalog.knowledge, L"installed", false, false, false, false);
  check(!shows_install(view) && view.remove && !view.update, "an installed pack offers removal only");
  check(view.subtitle == L"已安装；切换一次输入框后生效。", "installed subtitle");
  check(row_view(catalog.knowledge, L"installed", false, false, false, true).update, "an available update is offered on an installed pack");
  view = row_view(catalog.knowledge, L"missing", false, false, false, false);
  check(view.install_label == L"下载" && !view.remove, "a missing pack offers download");
  check(row_view(catalog.knowledge, L"outdated", false, false, false, false).install_label == L"更新", "an outdated pack offers an update");

  // 安装、删除进行中优先于状态。
  view = row_view(catalog.knowledge, L"missing", true, false, false, false);
  check(view.cancel && !shows_install(view) && view.subtitle == L"正在下载和校验模型…", "installing shows progress and cancel");
  check(row_view(catalog.knowledge, L"missing", true, true, false, false).subtitle == L"正在导入和校验模型…", "importing says import");
  view = row_view(catalog.knowledge, L"installed", false, false, true, false);
  check(!view.remove && !view.cancel && view.subtitle == L"正在删除模型…", "removing hides the actions");

  // 读的途中又要读（安装刚结束）：不并发，读回后再读一次。
  check(begin_read(catalog), "a refresh after install starts a read");
  check(!begin_read(catalog), "an overlapping refresh is deferred");
  check(finish_read(catalog, true), "the deferred refresh runs after the first read returns");
  check(begin_read(catalog) && !finish_read(catalog, true), "the deferred read completes once");

  // 读失败：给「重试」不给「下载」，不自动重读；点「重试」或再进页才重读。
  check(begin_read(catalog), "read issued");
  check(!finish_read(catalog, false), "a failed read does not loop");
  check(catalog.knowledge == Knowledge::failed && !wants_read(catalog), "a failure waits for the user");
  view = row_view(catalog.knowledge, L"installed", false, false, false, false);
  check(view.retry && !shows_install(view) && !view.remove, "a failure offers retry and never download");
  retry(catalog);
  check(catalog.knowledge == Knowledge::unknown && wants_read(catalog), "retry reads again");
  check(begin_read(catalog) && !finish_read(catalog, false), "second failure");
  page_entered(catalog);
  check(catalog.knowledge == Knowledge::unknown && wants_read(catalog), "entering the page after a failure reads again");
  check(begin_read(catalog) && !finish_read(catalog, true), "recovered");

  // 读回过的再进页：保留已知状态照常显示，同时重读一次（模型可能在别处装上或删掉）。
  page_entered(catalog);
  check(catalog.knowledge == Knowledge::known && wants_read(catalog), "entering the page refreshes known states without hiding them");
  check(begin_read(catalog) && !finish_read(catalog, true) && !wants_read(catalog), "the page-entry refresh runs once");

  // 进页时恰好在读：读回后再读一次。
  check(begin_read(catalog), "read in flight");
  page_entered(catalog);
  check(!wants_read(catalog), "no parallel read on page entry");
  check(finish_read(catalog, true), "page entry during a read rereads afterwards");
  check(begin_read(catalog) && !finish_read(catalog, true), "done");

  static_assert(row_view(Knowledge::unknown, L"installed", false, false, false, false).install_label.empty());
  return failures == 0 ? 0 : 1;
}
