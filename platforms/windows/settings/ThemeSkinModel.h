#pragma once
#include <cstddef>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 「主题」页皮肤包卡片和候选颜色取色器的规则，照共享设置的 `packages/ui/src/theme/global-theme.ts`（skinSlot、applyCandidateSkin、removeCandidateSkin、customThemeBase、candidateSkinFor、skinDrawsIn）、`settings/theme-selection-updates.ts`（updateCandidateColor）、`settings/pages/skin-page.tsx`（使用中的皮肤、卡片停在哪一款、两种明暗各用哪款的说明）、`skin/external-skins.tsx`（ExternalSkinCard 的明暗、能否选用和文案）和 `settings/candidate-colors-section.tsx`（七个取色器、它们的默认色和「跟随主题」按钮）。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::skins {

// 以某个 base 为底的皮肤包放在哪个槽位：固定明暗的内置主题给出它自己的明暗，`system` 两个槽位都放（skinSlot 的 null）。
enum class Slot { light, dark, either };

// 自定义主题里与皮肤包有关的三项：底（`custom_theme.base`）和浅色、深色两个槽位（`candidate_skin`、`candidate_skin_dark`），空串表示没有。
struct CustomSkins {
  std::wstring base = L"system";
  std::wstring light;
  std::wstring dark;
  bool operator==(CustomSkins const &other) const {
    return base == other.base && light == other.light && dark == other.dark;
  }
};

// 选着 `current` 时用了取色器，自定义主题画在哪个底上：屏幕上的主题留在下面，已经选着的自定义主题保留自己的底。`native` 不能当底，从它开始自定义时底是 `system`。
inline std::wstring custom_theme_base(std::wstring_view current, std::wstring_view custom_base) {
  if (current == L"custom")
    return custom_base.empty() ? std::wstring(L"system") : std::wstring(custom_base);
  return current == L"native" ? std::wstring(L"system") : std::wstring(current);
}

// 把皮肤包 `id`（清单 base 为 `base`、槽位为 `slot`）放进它所属的槽位：深色皮肤写深色槽位，浅色皮肤写浅色槽位，`system` 底的写两个；底改成包的 base。写深色槽位时，浅色槽位里确知是深色的旧皮肤一并清掉；写浅色槽位时，深色槽位还空着、而浅色槽位里原来那款确知是深色或 `system` 底的，先把它挪到深色槽位。`slot_of` 给出目录里一款包的槽位，不在目录里时为空，这时不挪。
template <typename SlotOf>
CustomSkins apply_candidate_skin(CustomSkins const &custom, std::wstring const &id,
                                 std::wstring const &base, Slot slot, SlotOf const &slot_of) {
  CustomSkins next = custom;
  next.base = base;
  if (slot != Slot::light)
    next.dark = id;
  if (slot == Slot::dark && !custom.light.empty()) {
    const std::optional<Slot> previous = slot_of(custom.light);
    if (previous == Slot::dark)
      next.light.clear();
  }
  if (slot != Slot::dark) {
    if (slot == Slot::light && custom.dark.empty() && !custom.light.empty()) {
      const std::optional<Slot> previous = slot_of(custom.light);
      if (previous == Slot::dark || previous == Slot::either)
        next.dark = custom.light;
    }
    next.light = id;
  }
  return next;
}

// 取消使用皮肤包 `id`：清掉放着它的槽位，另一个槽位不动。取下的是最后一款皮肤时底改回 `system`。
inline CustomSkins remove_candidate_skin(CustomSkins const &custom, std::wstring const &id) {
  CustomSkins next = custom;
  if (custom.light != id && custom.dark != id)
    return next;
  if (next.light == id)
    next.light.clear();
  if (next.dark == id)
    next.dark.clear();
  if (next.light.empty() && next.dark.empty())
    next.base = L"system";
  return next;
}

// `dark` 模式下取哪个槽位的皮肤包：深色模式先取深色槽位，没设时与浅色模式一样取浅色槽位。空串表示没有。
inline std::wstring candidate_skin_for(CustomSkins const &custom, bool dark) {
  return dark && !custom.dark.empty() ? custom.dark : custom.light;
}

// 以 `slot` 为槽位的皮肤包能否在 `dark` 模式下画：固定明暗的只画在自己的明暗下，`system` 底的两种都画。
inline bool skin_draws_in(Slot slot, bool dark) {
  return slot == Slot::either || (slot == Slot::dark) == dark;
}

// 自定义主题正在使用的皮肤包：选着自定义主题时，两个槽位里设了的那些（浅色槽位在前）。
inline std::vector<std::wstring> skins_in_use(bool custom_selected, CustomSkins const &custom) {
  std::vector<std::wstring> ids;
  if (!custom_selected)
    return ids;
  if (!custom.light.empty())
    ids.push_back(custom.light);
  if (!custom.dark.empty())
    ids.push_back(custom.dark);
  return ids;
}

inline bool skin_in_use(bool custom_selected, CustomSkins const &custom, std::wstring const &id) {
  return custom_selected && !id.empty() && (custom.light == id || custom.dark == id);
}

// 目录里的一款皮肤包，规则只用到这几项。
struct Listed {
  std::wstring id;
  std::wstring name;
  Slot slot = Slot::either;
};

// 轮播停在哪款皮肤上：当前预览明暗下用的那款，没有时是另一个槽位的那款，都得在目录里。没有时为空，这时「自定义」卡片才算选中。
inline std::optional<std::size_t> shown_package(bool custom_selected, CustomSkins const &custom,
                                                bool preview_dark,
                                                std::vector<Listed> const &packages) {
  if (!custom_selected)
    return std::nullopt;
  std::vector<std::wstring> order;
  order.push_back(candidate_skin_for(custom, preview_dark));
  for (auto const &id : skins_in_use(custom_selected, custom))
    order.push_back(id);
  for (auto const &id : order) {
    if (id.empty())
      continue;
    for (std::size_t index = 0; index < packages.size(); ++index)
      if (packages[index].id == id)
        return index;
  }
  return std::nullopt;
}

// 卡片下方的说明：用着皮肤包时，浅色和深色模式各画哪一款。目录里还没有的包按 id 显示；槽位里的包画不了这种明暗时写「不用皮肤」。没有皮肤包在用时为空。
inline std::wstring slot_summary(bool custom_selected, CustomSkins const &custom,
                                 std::vector<Listed> const &packages) {
  if (skins_in_use(custom_selected, custom).empty())
    return {};
  std::wstring text;
  for (const bool dark : {false, true}) {
    std::wstring name;
    const auto id = candidate_skin_for(custom, dark);
    if (!id.empty()) {
      name = id;
      for (auto const &skin : packages)
        if (skin.id == id) {
          name = skin_draws_in(skin.slot, dark) ? skin.name : std::wstring();
          break;
        }
    }
    if (!text.empty())
      text += L"，";
    text += dark ? L"深色模式" : L"浅色模式";
    text += name.empty() ? std::wstring(L"不用皮肤") : L"用「" + name + L"」";
  }
  return text + L"。";
}

// 皮肤放在哪个槽位、用在哪种明暗下，写给用户看。
inline std::wstring_view slot_label(Slot slot) {
  switch (slot) {
  case Slot::light:
    return L"浅色皮肤，用在浅色模式";
  case Slot::dark:
    return L"深色皮肤，用在深色模式";
  case Slot::either:
    break;
  }
  return L"跟随明暗，浅色、深色模式都能用";
}

inline bool contains(std::vector<std::wstring> const &values, std::wstring_view value) {
  for (auto const &item : values)
    if (item == value)
      return true;
  return false;
}

inline std::wstring joined(std::vector<std::wstring> const &values, std::wstring_view separator) {
  std::wstring text;
  for (auto const &value : values) {
    if (!text.empty())
      text += separator;
    text += value;
  }
  return text;
}

// 卡片画在哪种明暗下：固定明暗的底就是那种；否则是卡片上「预览浅色 / 预览深色」选的那种，没选过时用当前明暗，包不支持当前明暗时用它清单里第一种明暗。
inline bool card_dark(Slot slot, std::optional<bool> override_dark, bool active_dark,
                      std::vector<std::wstring> const &themes) {
  if (slot != Slot::either)
    return slot == Slot::dark;
  if (override_dark)
    return *override_dark;
  if (contains(themes, active_dark ? L"dark" : L"light"))
    return active_dark;
  return !(themes.size() > 0 && themes.front() == L"light");
}

// 能否选用：包得支持候选窗口的排列方式；固定明暗的包放进自己的槽位，当前明暗挡不住它，`system` 底的包还得支持当前明暗。卡片上的预览切换不影响这一点。
inline bool card_compatible(Slot slot, std::vector<std::wstring> const &layouts,
                            std::vector<std::wstring> const &themes, std::wstring_view layout,
                            bool active_dark) {
  return contains(layouts, layout) &&
         (slot != Slot::either || contains(themes, active_dark ? L"dark" : L"light"));
}

// 卡片标题下的说明：能选用时是包自己的描述，没有描述时写「基于」底的主题名；不能选用时说明它支持哪些排列方式和明暗。
inline std::wstring card_description(bool compatible, std::wstring const &description,
                                     std::wstring const &base_title,
                                     std::vector<std::wstring> const &layouts,
                                     std::vector<std::wstring> const &themes) {
  if (!compatible)
    return L"当前布局或明暗模式不受支持（" + joined(layouts, L"/") + L"，" + joined(themes, L"/") +
           L"）";
  return description.empty() ? L"基于 " + base_title : description;
}

// id、版本和作者，空的项不写。
inline std::wstring card_meta(std::wstring const &id, std::wstring const &version,
                              std::wstring const &author) {
  std::vector<std::wstring> parts;
  if (!id.empty())
    parts.push_back(id);
  if (!version.empty())
    parts.push_back(L"v" + version);
  if (!author.empty())
    parts.push_back(author);
  return joined(parts, L" · ");
}

// 一个候选颜色：`#RRGGBB`（不分大小写）读成小写，别的都算没设。
inline std::optional<std::wstring> candidate_text_color(std::wstring_view value) {
  if (value.size() != 7 || value[0] != L'#')
    return std::nullopt;
  std::wstring color(L"#");
  for (std::size_t i = 1; i < value.size(); ++i) {
    const wchar_t c = value[i];
    if (c >= L'0' && c <= L'9')
      color += c;
    else if (c >= L'a' && c <= L'f')
      color += c;
    else if (c >= L'A' && c <= L'F')
      color += static_cast<wchar_t>(c - L'A' + L'a');
    else
      return std::nullopt;
  }
  return color;
}

// 自定义主题的一个取色器：颜色槽（`custom_theme.candidate_colors` 的键）、标题、「跟随主题」按钮的读屏名（空时就叫「跟随主题」），以及按钮有没有按下状态（表面色和边框色的没有，点它总是清掉那一项），没设颜色时色块显示的浅色和深色默认色。
struct ColorSlot {
  std::wstring_view key;
  std::wstring_view title;
  std::wstring_view reset_label;
  bool pressed;
  std::wstring_view light;
  std::wstring_view dark;
};

// 先是背景，然后是写在背景上的内容，再是一行可能处于的各种状态，最后是边框。Windows 的候选窗口七种颜色都画，所以七个都列出来。
inline constexpr ColorSlot color_slots[] = {
    {L"surface", L"候选表面色", L"候选表面色跟随主题", false, L"#ffffff", L"#202020"},
    {L"text", L"候选文字颜色", L"", true, L"#1a1a1a", L"#e9e8e8"},
    {L"number", L"候选编号颜色", L"候选编号颜色跟随主题", true, L"#5f6368", L"#bdc1c6"},
    {L"accent", L"候选强调色", L"候选强调色跟随主题", true, L"#1a73e8", L"#8ab4f8"},
    {L"selected", L"候选选中色", L"候选选中色跟随主题", true, L"#e8e8e8", L"#3e3e3e"},
    {L"hover", L"候选悬停色", L"候选悬停色跟随主题", true, L"#ececec", L"#414141"},
    {L"border", L"候选边框色", L"候选边框色跟随主题", false, L"#dedede", L"#303030"},
};

// 设一个候选颜色或交还给主题时，除了那一项还要改什么：选了颜色就选上自定义主题；原来不是自定义主题时，把当时的主题留作底，并清掉两个槽位的皮肤包。交还给主题时别的都不动。
struct ColorChange {
  bool select_custom = false;
  bool reset_skins = false;
  std::wstring base;
};

inline ColorChange candidate_color_change(std::wstring_view current, std::wstring_view custom_base,
                                          bool clearing) {
  ColorChange change;
  change.select_custom = !clearing;
  change.reset_skins = !clearing && current != L"custom";
  if (change.reset_skins)
    change.base = custom_theme_base(current, custom_base);
  return change;
}

} // namespace msime::settings::skins
