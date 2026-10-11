#pragma once
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cwchar>
#include <cwctype>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "AccountPageModel.h"
#include "PluginPageModel.h"

// 「主题」页的「社区皮肤」、「插件」页的「社区插件」两个标签和「账号与同步」页「我的内容」打开的社区词库与回复模板的规则和文案，照共享设置在 Windows 上的样子移植：`community-helpers.ts`（错误码的句子、授权行、评分、大小）、`community-skin-category.ts`（皮肤分类）、`community-report.tsx`（举报原因）、`community-publish-validation.ts` 和 `use-community-publication-draft.ts`（发布名称与说明的校验）、`community-card-author.tsx`、`community-rating-metrics.tsx`、`candidate-skin-publish-dialog.tsx` 与 `community-plugins.tsx` 的固定文案，以及 `community-resources.tsx`（词库与回复模板的图库、详情和发布对话框）。社区操作本身在 Rust（msime_client_community），不在这里。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::community {

// ---- 文字 ----

// String.prototype.trim。
inline std::wstring trim(std::wstring_view value) { return account::normalize_name(value); }

// `[...text].length`：按码点数。
inline size_t code_points(std::wstring_view value) { return account::code_points(value); }

// 下标 i 处的码点，代理对合成一个；`width` 是它占的 UTF-16 单元数。
inline char32_t code_point_at(std::wstring_view value, size_t i, size_t &width) {
  const wchar_t c = value[i];
  if (c >= 0xD800 && c <= 0xDBFF && i + 1 < value.size() && value[i + 1] >= 0xDC00 &&
      value[i + 1] <= 0xDFFF) {
    width = 2;
    return 0x10000 + ((static_cast<char32_t>(c) - 0xD800) << 10) +
           (static_cast<char32_t>(value[i + 1]) - 0xDC00);
  }
  width = 1;
  return static_cast<char32_t>(c);
}

// 接在前一个字后面、不自成一字的码点：组合附加符号、变体选择符、零宽连接符、肤色修饰、标签字符和朝鲜文的中声与终声。
constexpr bool extends_cluster(char32_t c) {
  return (c >= 0x0300 && c <= 0x036F) || (c >= 0x1AB0 && c <= 0x1AFF) ||
         (c >= 0x1DC0 && c <= 0x1DFF) || (c >= 0x20D0 && c <= 0x20FF) ||
         (c >= 0xFE20 && c <= 0xFE2F) || (c >= 0xFE00 && c <= 0xFE0F) ||
         (c >= 0xE0100 && c <= 0xE01EF) || c == 0x200D || (c >= 0x1F3FB && c <= 0x1F3FF) ||
         (c >= 0xE0020 && c <= 0xE007F) || (c >= 0x1160 && c <= 0x11FF) ||
         (c >= 0xD7B0 && c <= 0xD7FF);
}

constexpr bool regional_indicator(char32_t c) { return c >= 0x1F1E6 && c <= 0x1F1FF; }

// boundedGraphemes：最多留 `maximum` 个字（字素簇）。共享页用 Intl.Segmenter 分字；这里按 UAX #29 里名称和搜索词会碰到的几条规则分：附加符号、变体选择符和肤色修饰接在前一个字上，零宽连接符把后一个字连进来（表情序列），两个地区指示符成一面旗，CR LF 算一个字。
inline std::wstring bounded_graphemes(std::wstring_view value, size_t maximum) {
  size_t clusters = 0;
  size_t i = 0;
  char32_t previous = 0;
  bool pending_flag = false;
  while (i < value.size()) {
    size_t width = 0;
    const char32_t c = code_point_at(value, i, width);
    bool joins = false;
    if (i > 0) {
      if (extends_cluster(c) || previous == 0x200D || (previous == L'\r' && c == L'\n'))
        joins = true;
      else if (regional_indicator(c) && pending_flag)
        joins = true;
    }
    if (!joins) {
      if (clusters == maximum)
        return std::wstring(value.substr(0, i));
      ++clusters;
      pending_flag = regional_indicator(c);
    } else if (regional_indicator(c)) {
      pending_flag = false;
    }
    previous = c;
    i += width;
  }
  return std::wstring(value);
}

// new TextEncoder().encode(text).length。
inline size_t utf8_length(std::wstring_view value) {
  size_t bytes = 0;
  for (size_t i = 0; i < value.size();) {
    size_t width = 0;
    const char32_t c = code_point_at(value, i, width);
    bytes += c < 0x80 ? 1 : c < 0x800 ? 2 : c < 0x10000 ? 3 : 4;
    i += width;
  }
  return bytes;
}

// 多行输入框的文字按 textarea 的 value 写换行：WinUI 的 TextBox 把换行存成单独的 CR，粘贴进来的还可能是 CR LF，都换成 LF，发出去的说明和网页上输入的一样。
inline std::wstring line_feeds(std::wstring_view value) {
  std::wstring text;
  text.reserve(value.size());
  for (size_t i = 0; i < value.size(); ++i) {
    if (value[i] != L'\r') {
      text += value[i];
      continue;
    }
    text += L'\n';
    if (i + 1 < value.size() && value[i + 1] == L'\n')
      ++i;
  }
  return text;
}

// 把非空的几段用「 · 」连起来（`[...].filter(Boolean).join(" · ")`）。
inline std::wstring join_dot(std::vector<std::wstring> const &parts) {
  std::wstring joined;
  for (auto const &part : parts) {
    if (part.empty())
      continue;
    if (!joined.empty())
      joined += L" · ";
    joined += part;
  }
  return joined;
}

// ---- 数字 ----

// formatZhNumber：zh-CN 的整数写法，每三位一个逗号。
inline std::wstring zh_number(double value) {
  const bool negative = value < 0;
  auto whole = static_cast<uint64_t>((negative ? -value : value) + 0.5);
  std::wstring digits = std::to_wstring(whole);
  std::wstring grouped;
  for (size_t i = 0; i < digits.size(); ++i) {
    if (i > 0 && (digits.size() - i) % 3 == 0)
      grouped += L',';
    grouped += digits[i];
  }
  return negative ? L"-" + grouped : grouped;
}

// `value.toFixed(1)`。printf 把正好落在两数中间的值舍到偶数，toFixed 取大的那个；double 里正好在中间的一位小数只有四分之一的奇数倍（x.25、x.75），这时先加上半格。
inline std::wstring fixed1(double value) {
  const double quarters = value * 4;
  if (value >= 0 && quarters == std::floor(quarters) && std::fmod(quarters, 2) == 1)
    value += 0.05;
  wchar_t buffer[64] = {};
  std::swprintf(buffer, 64, L"%.1f", value);
  return buffer;
}

// communityRating：没有人评分时是「暂无评分」，否则是一位小数的平均分。
inline std::wstring rating(double count, double average) {
  return count == 0 ? L"暂无评分" : fixed1(average) + L" 分";
}

// CommunityRatingMetrics：「N 人下载 · 平均分 · N 人评分」。
inline std::wstring rating_metrics(double count, std::wstring_view label, double rating_count,
                                   double average) {
  return zh_number(count) + L" 人" + std::wstring(label) + L" · " + rating(rating_count, average) +
         L" · " + zh_number(rating_count) + L" 人评分";
}

// candidateSkinMegabytes：一位小数的 MB。
inline std::wstring megabytes(double bytes) { return fixed1(bytes / (1024.0 * 1024.0)) + L" MB"; }

// ---- 错误 ----

// communityModerationMessage：审核拒绝、审核不可用和账号被封三种，各宿主同一句；其余为空。
inline std::wstring moderation_message(std::wstring_view code) {
  if (code == L"community_blocked_content" || code == L"account_blocked_content")
    return L"内容包含不允许发布的词语，请修改后再提交";
  if (code == L"community_screening_unavailable" || code == L"account_screening_unavailable")
    return L"审核服务暂时不可用，请稍后重试";
  if (code == L"community_account_banned" || code == L"account_banned")
    return L"该账号已被封禁，暂时无法使用账号相关功能";
  return {};
}

// communityNeedsSignIn：只有未登录才在出错说明后面给「去登录」。
inline bool needs_sign_in(std::wstring_view code) { return code == L"community_unauthorized"; }

// candidateSkinMessage：候选窗皮肤图库和发布对话框的句子。`publishing` 选发布时 community_invalid 的那一句。
inline std::wstring candidate_skin_message(std::wstring_view code, bool publishing = false) {
  if (auto moderation = moderation_message(code); !moderation.empty())
    return moderation;
  if (code == L"candidate_skin_package")
    return L"皮肤包未通过校验，无法分享或安装。";
  if (code == L"candidate_skin_file_type")
    return L"仅支持 PNG 或 JPEG 图片，且不能包含样式表。";
  if (code == L"candidate_skin_file_path")
    return L"皮肤文件名或数量不符合要求。";
  if (code == L"candidate_skin_too_large")
    return L"单个图片不超过 1 MB、合计不超过 2 MB，预览图不超过 256 KB。";
  if (code == L"candidate_skin_image_invalid")
    return L"图片已损坏或格式与扩展名不符。";
  if (code == L"candidate_skin_license_required")
    return L"发布前请在 skin.toml 的 [license] 中填写素材授权 assets。";
  if (code == L"candidate_skin_preview_required")
    return L"请在 skin.toml 中用 preview 指定一张 PNG/JPEG 预览图。";
  if (code == L"candidate_skin_exists")
    return L"已存在同名皮肤，安装会整体替换它。";
  if (code == L"storage")
    return L"无法写入皮肤目录。";
  if (code == L"community_invalid")
    return publishing ? L"服务器未接受这款皮肤：请确认图片每边不超过 2048 像素、能被正常解码，且皮肤 ID 不与内置主题重名。"
                      : L"内容无效，请修改后重试。";
  if (code == L"community_conflict")
    return L"发布信息已变更，或皮肤库已达到数量上限。";
  if (code == L"community_rate_limited")
    return L"发布太频繁，请稍后再试。";
  if (code == L"community_forbidden")
    return L"下载后才能评分，且不能给自己的作品评分。";
  if (code == L"community_unauthorized")
    return L"请先登录后执行此操作。";
  if (code == L"community_not_found")
    return L"作品不存在或已下架。";
  if (code == L"community_cancelled")
    return L"账号状态已变化，请重新加载。";
  if (code == L"community_storage")
    return L"无法安全读取登录状态，请检查设备安全设置。";
  return L"社区暂时不可用，请稍后重试。";
}

// communityPluginMessage：插件图库和发布对话框的句子；打包失败是 client-core 的 plugin_* 码，按「插件」页的说法（pluginErrorMessage，带上 detail）。
inline std::wstring plugin_message(std::wstring_view code, std::wstring const &detail,
                                   bool publishing = false) {
  if (auto moderation = moderation_message(code); !moderation.empty())
    return moderation;
  if (code == L"plugin_community_too_large")
    return L"插件压缩后不能超过 8 MB。";
  if (code == L"plugin_community_checksum")
    return L"下载的插件已损坏，请重试。";
  if (code == L"plugin_community_mismatch")
    return L"下载的插件与作品信息不符，已停止安装。";
  if (code == L"storage")
    return L"无法读写插件目录，请检查数据目录的权限。";
  if (code == L"community_invalid")
    return publishing ? L"服务器未接受这个插件：请确认名称、说明和包内容符合发布要求。"
                      : L"内容无效，请修改后重试。";
  if (code == L"community_conflict")
    return publishing ? L"发布信息已变更，或已达到发布上限（最多 20 个、合计 32 MB）。"
                      : L"作品状态已变化，请刷新后重试。";
  if (code == L"community_rate_limited")
    return publishing ? L"每小时最多发布 10 次，请稍后再试。" : L"请求过于频繁，请稍后再试。";
  if (code == L"community_forbidden")
    return L"下载后才能评分，且不能给自己的作品评分。";
  if (code == L"community_unauthorized")
    return L"请先登录后执行此操作。";
  if (code == L"community_not_found")
    return L"作品不存在或已下架。";
  if (code == L"community_cancelled")
    return L"账号状态已变化，请重新加载。";
  if (code == L"community_storage")
    return L"无法安全读取登录状态，请检查设备安全设置。";
  return plugins::error_message(code, detail, L"社区暂时不可用，请稍后重试。");
}

// ---- 授权、作者和分类 ----

// communityLicenseLine：素材授权、代码授权和来源，空的不写，用「 / 」隔开。
inline std::wstring license_line(std::wstring_view assets, std::wstring_view code,
                                 std::wstring_view source) {
  std::wstring line;
  auto add = [&line](std::wstring_view label, std::wstring_view value) {
    const auto trimmed = trim(value);
    if (trimmed.empty())
      return;
    if (!line.empty())
      line += L" / ";
    line += std::wstring(label) + L" " + trimmed;
  };
  add(L"素材授权", assets);
  add(L"代码授权", code);
  add(L"来源", source);
  return line;
}

// CommunityCardAuthor：卡片上的作者一行，自己的作品写「我的作品」，私有和被下架的另外注明。
inline std::wstring card_author(std::wstring_view prefix, std::wstring_view author, bool owned,
                                bool is_private, bool removed) {
  std::wstring line;
  if (!prefix.empty())
    line += std::wstring(prefix) + L" · ";
  line += owned ? std::wstring(L"我的作品") : std::wstring(author);
  if (is_private)
    line += L" · 私有";
  if (owned && removed)
    line += L" · 已下架";
  return line;
}

struct Choice {
  std::wstring_view value;
  std::wstring_view label;
};

// communitySkinCategories：皮肤的发布分类，顺序即筛选按钮和下拉框的顺序，id 与服务端一致。
inline constexpr std::array<Choice, 8> skin_categories{{
    {L"nature", L"自然"},
    {L"guofeng", L"国风"},
    {L"acg", L"二次元"},
    {L"cute", L"可爱"},
    {L"food", L"美食"},
    {L"tech", L"科技夜色"},
    {L"minimal", L"简约"},
    {L"other", L"其他"},
}};

// communitySkinCategoryLabel：分类名；早于分类功能的服务端不返回分类，此时为空。
inline std::wstring category_label(std::wstring_view category) {
  for (auto const &item : skin_categories)
    if (item.value == category)
      return std::wstring(item.label);
  return {};
}

// communityPluginKinds：社区插件的类型筛选，与 client-core 的 PUBLISHABLE_KINDS 一致。
inline constexpr std::array<std::wstring_view, 8> plugin_kinds{
    L"sound", L"music", L"command_table", L"effect",
    L"phrase_table", L"helpcode", L"wordbook", L"symbol_set"};

// 发布对话框里本机的一项：名字和 id 相同时只写 id，否则写「名字（id）」。
inline std::wstring local_option_label(std::wstring_view name, std::wstring_view id) {
  return name == id ? std::wstring(id) : std::wstring(name) + L"（" + std::wstring(id) + L"）";
}

// ---- 举报 ----

// communityReportReasons：举报原因，顺序即对话框里的顺序；文字就是管理员看到的标签。
inline constexpr std::array<std::wstring_view, 6> report_reasons{
    L"侵权/抄袭", L"色情低俗", L"违法违规", L"垃圾广告", L"恶意插件", L"其他"};

inline constexpr size_t report_detail_limit = 1000;
inline constexpr std::wstring_view reported_notice = L"已收到举报，我们会尽快处理。";

// ---- 发布 ----

inline constexpr size_t name_limit = 32;
inline constexpr size_t description_limit = 280;
inline constexpr size_t search_limit = 128;

struct PublishFields {
  std::wstring name, description;
  bool name_valid = false;
  bool description_valid = false;
};

// communityPublishFields：名称和说明去掉首尾空白；名称非空、最多 32 个字，说明最多 280 个字。
inline PublishFields publish_fields(std::wstring_view name, std::wstring_view description) {
  PublishFields fields;
  fields.name = trim(name);
  fields.description = trim(description);
  fields.name_valid = !fields.name.empty() &&
                      bounded_graphemes(fields.name, name_limit) == fields.name &&
                      code_points(fields.name) <= name_limit;
  fields.description_valid = code_points(fields.description) <= description_limit;
  return fields;
}

// 公开发布的皮肤没写素材授权时可选的两种，以及「其他」的上限（写进 skin.toml 的 UTF-8 字节数）。
inline constexpr std::array<Choice, 2> asset_licenses{{
    {L"CC-BY-4.0", L"CC BY 4.0（推荐：可自由使用，需署名作者）"},
    {L"CC0-1.0", L"CC0 1.0（放弃权利，任何人可随意使用）"},
}};
inline constexpr size_t asset_license_limit = 120;

// 「其他」授权写得对不对：去掉首尾空白后非空，UTF-8 不超过 120 字节。
inline bool asset_license_valid(std::wstring_view value) {
  const auto trimmed = trim(value);
  return !trimmed.empty() && utf8_length(trimmed) <= asset_license_limit;
}

inline constexpr std::wstring_view skin_package_limit = L"2 MB";
inline constexpr std::wstring_view skin_publish_warning =
    L"仅上传 skin.toml 与 PNG/JPEG 图片（需包含预览图）；单个文件不超过 1 MB、合计不超过 2 MB、每边不超过 2048 像素。服务器会重新编码图片并去除元数据。登录期间修改本地皮肤，会自动同步到这款作品。";
inline constexpr std::wstring_view plugin_archive_limit = L"8 MB";
inline constexpr std::wstring_view plugin_publish_warning =
    L"上传的是插件目录中的全部文件（不含隐藏文件），压缩后不超过 8 MB；每个账号最多发布 20 个、合计 32 MB，每小时最多发布 10 次。其他用户下载后会按原样安装。";

// 社区插件图库空着时的那一句。`filtered` 是搜了词或选了类型，`publishable` 是本机能发布（宿主提供本机插件目录）。
inline std::wstring plugin_empty_notice(bool has_more, bool filtered, bool mine, bool publishable) {
  if (has_more)
    return filtered ? L"前面的插件这台设备都不能安装，点「加载更多」继续查找。"
                    : L"前面的插件这台设备都不能安装，点「加载更多」查看更早发布的插件。";
  if (mine)
    return L"你还没有发布过插件。";
  if (filtered)
    return L"没有匹配的插件。";
  return publishable ? L"社区里还没有插件，安装或制作插件后可以点「发布我的插件」分享出来。"
                     : L"社区里还没有插件。";
}

// ---- 词库与回复模板 ----

// resourceMessage：社区词库与回复模板的句子。没有「去登录」：共享页的资源图库不给它。
inline std::wstring resource_message(std::wstring_view code) {
  if (auto moderation = moderation_message(code); !moderation.empty())
    return moderation;
  if (code == L"community_invalid")
    return L"内容无效，请修改后重试。";
  if (code == L"community_unauthorized")
    return L"请先登录后执行此操作。";
  if (code == L"community_forbidden")
    return L"没有权限执行此操作。";
  if (code == L"community_conflict")
    return L"作品状态已变化或已达到发布上限，请刷新后重试。";
  if (code == L"community_not_found")
    return L"作品不存在或已下架。";
  if (code == L"community_rate_limited")
    return L"请求过于频繁，请稍后再试。";
  if (code == L"community_cancelled")
    return L"账号状态已变化，请重新加载。";
  if (code == L"community_storage")
    return L"无法安全保存本地模板，请稍后重试。";
  if (code == L"community_resource_library_format")
    return L"本地模板库无法读取，请检查后重试。";
  return L"社区暂时不可用，请稍后重试。";
}

// resourceKindTitle：dictionary 是「词库」，reply 是「回复模板」。
inline std::wstring resource_kind_title(std::wstring_view kind) {
  return kind == L"dictionary" ? L"词库" : L"回复模板";
}

// 社区分类（CommunityHomePage 在没有皮肤画廊的宿主上的两个标签），值是 resource_list 的 kind。
inline constexpr std::array<Choice, 2> resource_kinds{{
    {L"dictionary", L"词库"},
    {L"reply", L"回复模板"},
}};

// 范围按钮（CommunityResourceScopeButtons），顺序即按钮的顺序；值是 resource_list 的 scope。
inline constexpr std::array<Choice, 3> resource_scopes{{
    {L"", L"全部"},
    {L"saved", L"收藏"},
    {L"mine", L"我的作品"},
}};

// 「我的内容」的一行打开的分类和范围（communityDestinationView）：我发布的是 mine，收藏的是 saved。
struct ResourceDestination {
  std::wstring_view title, kind, scope;
};
inline constexpr std::array<ResourceDestination, 4> resource_destinations{{
    {L"我发布的词库", L"dictionary", L"mine"},
    {L"我发布的回复模板", L"reply", L"mine"},
    {L"收藏的词库", L"dictionary", L"saved"},
    {L"收藏的回复模板", L"reply", L"saved"},
}};

// 图库标题（CommunityResourcesPage 的 CommunityGalleryHeading）。
inline std::wstring resource_heading(std::wstring_view kind, std::wstring_view scope) {
  if (scope == L"mine")
    return L"我的" + resource_kind_title(kind) + L"作品";
  if (scope == L"saved")
    return L"收藏的" + resource_kind_title(kind);
  return kind == L"dictionary" ? L"好词，随手可得" : L"找到舒服的表达";
}

inline std::wstring resource_heading_note(std::wstring_view kind) {
  return kind == L"dictionary" ? L"把常用词带进云词库，让输入更顺手"
                               : L"收藏喜欢的语气，给每次回应一点灵感";
}

// ResourceCard 的图标字、没有说明时的那一句和收藏一行。
inline std::wstring resource_card_glyph(std::wstring_view kind) {
  return kind == L"dictionary" ? L"字" : L"话";
}

inline std::wstring resource_card_description(std::wstring_view kind, std::wstring_view description) {
  if (!description.empty())
    return std::wstring(description);
  return kind == L"dictionary" ? L"共享词条" : L"回复模板";
}

inline std::wstring resource_card_metrics(double saves, double rating_count, double average) {
  return L"☆ " + rating(rating_count, average) + L" · " + zh_number(saves) + L" 人收藏";
}

// 发布对话框里词条的类型（ResourceEditor 的「类型」下拉框），值与 client-core 的 DictionaryKind 一致。
inline constexpr std::array<Choice, 4> shared_word_kinds{{
    {L"pinyin", L"拼音"},
    {L"wubi", L"五笔"},
    {L"quick", L"快捷短语"},
    {L"english", L"英文"},
}};

inline constexpr size_t shared_word_limit = 128;
inline constexpr size_t reply_prompt_limit = 2000;
inline constexpr std::wstring_view default_shared_word_weight = L"100000";
inline constexpr std::wstring_view resource_publish_warning =
    L"发布内容会公开展示。请勿包含 API Key、私人聊天内容或其他个人资料；发布后可在“我的作品”中下架。";

struct SharedWord {
  std::wstring kind, code, word;
  int64_t weight = 0;
};

// Number(text) 是安全整数时的值（Number.isSafeInteger），否则为空。与 JS 一样先去掉首尾空白，空串是 0，认十进制（可带符号、小数点和指数，值仍须是整数）和不带符号的 0x、0o、0b。
inline std::optional<int64_t> js_safe_integer(std::wstring_view text) {
  const auto trimmed = trim(text);
  if (trimmed.empty())
    return 0;
  constexpr double maximum = 9007199254740991.0;
  if (trimmed.size() > 2 && trimmed[0] == L'0') {
    const auto marker = static_cast<wchar_t>(std::towlower(trimmed[1]));
    const int base = marker == L'x' ? 16 : marker == L'o' ? 8 : marker == L'b' ? 2 : 0;
    if (base != 0) {
      double value = 0;
      for (size_t i = 2; i < trimmed.size(); ++i) {
        const auto c = static_cast<wchar_t>(std::towlower(trimmed[i]));
        const int digit = c >= L'0' && c <= L'9'   ? c - L'0'
                          : c >= L'a' && c <= L'f' ? c - L'a' + 10
                                                   : 99;
        if (digit >= base)
          return std::nullopt;
        value = value * base + digit;
      }
      if (value > maximum)
        return std::nullopt;
      return static_cast<int64_t>(value);
    }
  }
  // StrDecimalLiteral：符号、数字、可选的小数点和小数、可选的指数。Infinity 不是安全整数。
  size_t i = 0;
  if (trimmed[i] == L'+' || trimmed[i] == L'-')
    ++i;
  size_t digits = 0;
  while (i < trimmed.size() && trimmed[i] >= L'0' && trimmed[i] <= L'9') {
    ++i;
    ++digits;
  }
  if (i < trimmed.size() && trimmed[i] == L'.') {
    ++i;
    while (i < trimmed.size() && trimmed[i] >= L'0' && trimmed[i] <= L'9') {
      ++i;
      ++digits;
    }
  }
  if (digits == 0)
    return std::nullopt;
  if (i < trimmed.size() && (trimmed[i] == L'e' || trimmed[i] == L'E')) {
    ++i;
    if (i < trimmed.size() && (trimmed[i] == L'+' || trimmed[i] == L'-'))
      ++i;
    size_t exponent = 0;
    while (i < trimmed.size() && trimmed[i] >= L'0' && trimmed[i] <= L'9') {
      ++i;
      ++exponent;
    }
    if (exponent == 0)
      return std::nullopt;
  }
  if (i != trimmed.size())
    return std::nullopt;
  const double value = std::wcstod(trimmed.c_str(), nullptr);
  if (!std::isfinite(value) || std::floor(value) != value || std::fabs(value) > maximum)
    return std::nullopt;
  return static_cast<int64_t>(value);
}

inline constexpr std::wstring_view shared_word_invalid =
    L"词条不能为空、不能重复，权重必须为非负整数，且最多 128 条。";

// ResourceEditor 的 addEntry：编码去掉首尾空白，词语原样；编码和词语不能为空，权重是非负的安全整数，同类型同编码同词语的不能重复，最多 128 条。能加时返回这一条。
inline std::optional<SharedWord> shared_word(std::vector<SharedWord> const &entries,
                                             std::wstring_view kind, std::wstring_view code,
                                             std::wstring_view word, std::wstring_view weight) {
  SharedWord value{std::wstring(kind), trim(code), std::wstring(word), 0};
  const auto number = js_safe_integer(weight);
  if (value.code.empty() || value.word.empty() || !number || *number < 0 ||
      entries.size() >= shared_word_limit)
    return std::nullopt;
  for (auto const &item : entries)
    if (item.kind == value.kind && item.code == value.code && item.word == value.word)
      return std::nullopt;
  value.weight = *number;
  return value;
}

// ResourceEditor 的 submit：名称和说明照发布的规则；回复模板的提示词去掉空白后不能为空、最多 2000 个字，词库至少一条；第一次发布要确认权利。能发布时为空，否则是要显示的那一句。
inline std::wstring resource_publish_error(std::wstring_view kind, std::wstring_view name,
                                           std::wstring_view description, std::wstring_view prompt,
                                           size_t entries, bool existing, bool agreed) {
  const auto fields = publish_fields(name, description);
  const bool content = kind == L"reply"
                           ? !trim(prompt).empty() && code_points(prompt) <= reply_prompt_limit
                           : entries > 0;
  if (fields.name_valid && fields.description_valid && content && (existing || agreed))
    return {};
  return existing ? L"请填写有效的作品信息。" : L"请填写有效内容并确认拥有公开发布所需的权利。";
}

// 导入本机词库的一次请求：source 是词条原来的类型（请求 id 里用它），kind 是本机词库，text 是这一组的标准格式文本。
struct LocalImport {
  std::wstring source, kind, text;
};

// importCommunityDictionary：按词条类型分组，每组一次标准格式的导入（「词语\t编码\t权重」一行一条），组的顺序是类型第一次出现的顺序；快捷短语的本机词库叫 quick_phrase。
inline std::vector<LocalImport> local_import_groups(std::vector<SharedWord> const &entries) {
  std::vector<LocalImport> groups;
  for (auto const &entry : entries) {
    auto found = groups.begin();
    while (found != groups.end() && found->source != entry.kind)
      ++found;
    if (found == groups.end()) {
      groups.push_back({entry.kind, entry.kind == L"quick" ? L"quick_phrase" : entry.kind, L""});
      found = groups.end() - 1;
    } else {
      found->text += L'\n';
    }
    found->text += entry.word + L'\t' + entry.code + L'\t' + std::to_wstring(entry.weight);
  }
  return groups;
}

} // namespace msime::settings::community
