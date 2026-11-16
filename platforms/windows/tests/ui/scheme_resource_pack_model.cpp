#include "../../settings/SchemeResourcePackModel.h"
#include <cassert>
#include <set>
#include <string_view>

namespace sr = msime::settings::scheme_resource;
namespace sp = msime::settings::sentence_pack;

int main() {
  // 瘦包的五笔和语言方案都有安装入口；只依赖核心词库的方案不额外下载。
  assert(sr::for_scheme(L"wubi")->id == L"wubi");
  assert(sr::for_scheme(L"japanese")->id == L"japanese");
  for (auto scheme : {L"cantonese", L"zhuyin", L"stroke"})
    assert(sr::for_scheme(scheme)->id == L"language-dictionaries");
  for (auto scheme : {L"quanpin", L"shuangpin", L"korean", L"vietnamese", L"tibetan", L"future"})
    assert(!sr::for_scheme(scheme));

  std::set<std::wstring_view> files;
  const auto exists = [&files](std::wstring_view name) { return files.count(name) > 0; };
  assert(!sr::bundled(*sr::find(L"wubi"), exists));
  files.insert(L"msime-wubi.db");
  assert(sr::bundled(*sr::find(L"wubi"), exists));
  files.insert(L"msime-japanese.dat");
  assert(!sr::bundled(*sr::find(L"japanese"), exists));
  files.insert(L"msime-mozc_dictionary_oss_README.txt");
  files.insert(L"msime-mozc_LICENSE.txt");
  assert(sr::bundled(*sr::find(L"japanese"), exists));

  // 已读状态决定可用动作；读取失败不能误给下载，内置码表不能删。
  for (auto const &pack : sr::packs) {
    auto view = sr::row_view(pack.id, sp::Knowledge::known, L"missing", false, false, false, false);
    assert(view.install_label == L"下载" && !view.remove);
    assert(view.subtitle.find(L"词库尚未安装") != std::wstring_view::npos);
    view = sr::row_view(pack.id, sp::Knowledge::failed, L"missing", false, false, false, false);
    assert(view.retry && view.install_label.empty());
    view = sr::row_view(pack.id, sp::Knowledge::unknown, L"installed", false, false, false, false);
    assert(view.install_label.empty() && !view.remove);
    view = sr::row_view(pack.id, sp::Knowledge::known, L"bundled", false, false, false, false);
    assert(view.subtitle == L"随安装包提供。" && view.install_label.empty() && !view.remove);
    view = sr::row_view(pack.id, sp::Knowledge::known, L"installed", false, false, false, false);
    assert(view.remove && view.install_label.empty());
    view = sr::row_view(pack.id, sp::Knowledge::known, L"missing", true, false, false, false);
    assert(view.cancel && view.subtitle == L"正在下载和校验词库…");
  }
  assert(sr::row_view(L"settled-model", sp::Knowledge::known, L"missing", false, false, false, false).subtitle ==
         sp::row_view(sp::Knowledge::known, L"missing", false, false, false, false).subtitle);
}
