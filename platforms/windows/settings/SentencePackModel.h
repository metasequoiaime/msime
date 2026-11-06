#pragma once
#include <string_view>

// 「输入 › 候选与联想」里神经联想模型和语法模型那几行（main.cpp 的 `sentence_pack_row`）：资源包状态由 `msime_client_resource_packs` 在工作线程上读回，读回之前页面不知道装没装，不能先当「没装」画「下载」（#10839）。这里管「读到哪一步了」和每一行该画什么，不依赖 Windows 和 WinRT 头文件，主机上的单元测试能直接查。
namespace msime::settings::sentence_pack {

// 这几行所在的页面 id（SettingsNavigation.h 的 `pages`）。状态读回、安装或删除结束后只在当前就是这一页时重画；以前比的是路由别名 "input"，`current_page_` 永远不等于它，读回的状态和「正在下载」都画不出来，页面一直停在「下载」（#10839）。
inline constexpr std::string_view host_page = "typing";

// 页面对资源包状态知道多少。
enum class Knowledge {
  // 还没读回过（刚打开窗口，或读失败后又进了这一页、点了「重试」）。
  unknown,
  // 最近一次读取成功，`main.cpp` 里的状态列表可信。
  known,
  // 最近一次读取失败；不自动重读（否则失败、重画、再读会一直转），等进页或点「重试」。
  failed,
};

// 状态读取的进度。同一时间只发一次读取：读的途中又要读（安装或删除刚结束、又进了这一页）只记下 `stale`，这次读回后再读一次，免得先发的那次后回来、拿旧结果盖掉新结果。
struct Catalog {
  Knowledge knowledge = Knowledge::unknown;
  bool reading = false;
  bool stale = false;
};

// 画页面时是否该发一次读取：还没读回过、或进页后标了要重读，且没有正在读的。
constexpr bool wants_read(Catalog const &catalog) {
  return !catalog.reading && (catalog.knowledge == Knowledge::unknown || catalog.stale);
}

// 要发读取时调用；返回 true 时调用方真的去读，false 表示已有一次在读，读回后会再读。
constexpr bool begin_read(Catalog &catalog) {
  if (catalog.reading) {
    catalog.stale = true;
    return false;
  }
  catalog.reading = true;
  catalog.stale = false;
  return true;
}

// 读回（`ok` 为 false 是 FFI 报错、返回内容解析不了或状态目录不可用）后调用；返回 true 时调用方马上再读一次。
constexpr bool finish_read(Catalog &catalog, bool ok) {
  catalog.reading = false;
  catalog.knowledge = ok ? Knowledge::known : Knowledge::failed;
  return catalog.stale;
}

// 每次进入「输入」页：已读回的也重读一次（模型可能在别处装上或删掉），读失败的回到「正在读取」。
constexpr void page_entered(Catalog &catalog) {
  if (catalog.knowledge == Knowledge::failed)
    catalog.knowledge = Knowledge::unknown;
  catalog.stale = true;
}

// 「重试」：读失败后回到「正在读取」，下一次画页面时重读。
constexpr void retry(Catalog &catalog) {
  if (catalog.knowledge == Knowledge::failed)
    catalog.knowledge = Knowledge::unknown;
}

// 一行该画的副标题和按钮。`install_label` 为空时不画「下载」「更新」和「从文件导入」（以及不联网安装的那一句）。
struct RowView {
  std::wstring_view subtitle;
  std::wstring_view install_label;
  bool update = false;
  bool remove = false;
  bool cancel = false;
  bool retry = false;
};

// `state` 是 `msime_client_resource_packs` 给这个包的 `state`（"installed"、"outdated"、"missing"），列表里没有这个包时传 "missing"。安装、删除进行中的说法优先；状态没读回时只说「正在读取」，读失败时只给「重试」，都不给「下载」：装好的模型在读回之前被当成没装、再点「下载」就是 #10839。
constexpr RowView row_view(Knowledge knowledge, std::wstring_view state, bool installing,
                           bool importing, bool removing, bool update_available) {
  RowView view;
  if (installing) {
    view.subtitle = importing ? L"正在导入和校验模型…" : L"正在下载和校验模型…";
    view.cancel = true;
  } else if (removing) {
    view.subtitle = L"正在删除模型…";
  } else if (knowledge == Knowledge::unknown) {
    view.subtitle = L"正在读取模型状态…";
  } else if (knowledge == Knowledge::failed) {
    view.subtitle = L"无法读取模型状态，请重试。";
    view.retry = true;
  } else if (state == L"installed") {
    view.subtitle = L"已安装；切换一次输入框后生效。";
    view.update = update_available;
    view.remove = true;
  } else if (state == L"outdated") {
    view.subtitle = L"已安装的文件需要更新。";
    view.install_label = L"更新";
  } else {
    view.subtitle = L"模型尚未安装。";
    view.install_label = L"下载";
  }
  return view;
}

} // namespace msime::settings::sentence_pack
