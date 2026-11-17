#pragma once

#include "TypingStatisticsModel.h"
#include <limits>
#include <set>

namespace msime::settings::typing {

using DailyKeyLanguages = std::map<std::string, std::map<std::string, Counts>>;

struct KeyLanguageChoice {
  const char *id;
  const wchar_t *title;
};

inline constexpr std::array<KeyLanguageChoice, 10> key_language_choices{{
    {"all", L"全部"}, {"chinese", L"中文"}, {"english", L"英文"},
    {"japanese", L"日文"}, {"korean", L"韩文"}, {"cantonese", L"粤语"},
    {"vietnamese", L"越南文"}, {"tibetan", L"藏文"},
    {"common", L"通用键"}, {"unknown", L"未分类"},
}};

// 只聚合共享 Rust 已分类的桶，不按当前输入方案猜测历史语言。
class KeyLanguageSnapshot {
public:
  KeyLanguageSnapshot(std::map<std::string, Counts> const &daily_keys,
                      DailyKeyLanguages const &daily_languages,
                      std::optional<std::vector<std::string>> const &scope) {
    const auto included = scope ? std::optional<std::set<std::string>>(
                                     std::set<std::string>(scope->begin(), scope->end()))
                               : std::nullopt;
    for (auto const &[day, keys] : daily_keys)
      if (!included || included->count(day))
        merge(keys, total_);
    for (auto const &[day, languages] : daily_languages)
      if (!included || included->count(day))
        for (auto const &[language, keys] : languages)
          merge(keys, groups_[language]);
  }

  std::vector<KeyLanguageChoice> choices() const {
    std::vector<KeyLanguageChoice> result;
    for (auto const &choice : key_language_choices) {
      const auto group = groups_.find(choice.id);
      if (std::string_view(choice.id) == "all" ||
          (group != groups_.end() && !group->second.empty()))
        result.push_back(choice);
    }
    return result;
  }

  // 时间范围或重载后没有记录的选择回到完整总计。
  std::string resolved(std::string const &selection) const {
    for (auto const &choice : choices())
      if (selection == choice.id)
        return selection;
    return "all";
  }

  Counts const &counts(std::string const &selection) const {
    const auto language = resolved(selection);
    return language == "all" ? total_ : groups_.at(language);
  }

private:
  static void merge(Counts const &keys, Counts &result) {
    for (auto const &[key, count] : keys)
      if (std::isfinite(count) && count > 0) {
        auto &old = result[key];
        const auto maximum = std::numeric_limits<double>::max();
        old = old > maximum - count ? maximum : old + count;
      }
  }

  Counts total_;
  std::map<std::string, Counts> groups_;
};

} // namespace msime::settings::typing
