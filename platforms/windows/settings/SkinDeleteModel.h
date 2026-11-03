#pragma once
#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>

// 「主题」页皮肤包卡片上的「删除」和主题轮播的「回到当前」（#10746），照共享设置的 `packages/ui/src/skin/use-skin-deletion.tsx` 和 `settings/theme-carousel.tsx`。能不能删由 host-api 的 msime_client_skin_delete 回答（规则在 client-services 的 `candidate_sync::delete_unselected`：自定义主题的浅色或深色槽位还放着它时是 candidate_skin_in_use），这里只有确认框、结果和失败时给用户看的话，以及「回到当前」什么时候可点。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::skin_delete {

// 确认框的正文：说清楚删的是本机文件夹、云端私人副本下次同步时一起删、公开发布的不受影响。
inline std::wstring question(std::wstring const &name) {
  return L"「" + name +
         L"」的皮肤文件夹会从这台电脑上删掉，无法恢复。开着皮肤云端同步时，同步过的私人副本会在下次同步时一起从云端删掉；已经公开发布到社区的不受影响。";
}

// 删掉以后的一句话；`synced` 是 msime_client_skin_delete 回的这款皮肤是否和账号皮肤库同步过。
inline std::wstring done(std::wstring const &name, bool synced) {
  return L"已删除「" + name + L"」。" +
         (synced ? std::wstring(L"云端皮肤库里的私人副本会在下次同步时一起删掉。") : std::wstring());
}

// 删除没成功时的一句话，按 msime_client_skin_delete 回的码。`shown_in_use` 是卡片上正显示着「使用中」：槽位里留着这款皮肤、但当前全局主题不是自定义主题时，卡片的开关是关着的，要先打开再关掉才能把它从自定义主题里取下。
inline std::wstring failure(std::string_view code, std::wstring const &name, bool shown_in_use) {
  if (code == "candidate_skin_in_use")
    return shown_in_use ? L"「" + name + L"」正在使用，先关掉它的开关再删除。"
                        : L"自定义主题里还留着「" + name +
                              L"」，切回自定义主题时会用上它。先打开再关掉它的开关，把它从自定义主题里取下，再删除。";
  if (code == "candidate_skin_not_installed")
    return L"「" + name + L"」已经不在皮肤目录里了。";
  if (code == "candidate_skin_package")
    return L"「" + name + L"」不是普通的皮肤文件夹，请打开皮肤目录手动删除。";
  return L"没能删除「" + name + L"」，请稍后重试。";
}

// 删除失败后要不要重扫皮肤目录：已经不在目录里的皮肤，重扫一次让卡片也消失。
inline bool rescan_after_failure(std::string_view code) {
  return code == "candidate_skin_not_installed";
}

// 「回到当前」在轮播停在第 `at` 张、在用的是第 `selected` 张、共 `count` 张时可不可点：停在在用的那张时不可点。`selected` 越界时按最后一张算，与轮播自己跳过去时相同。
inline bool back_enabled(int32_t at, std::size_t selected, std::size_t count) {
  if (count == 0 || at < 0)
    return false;
  const std::size_t target = selected < count ? selected : count - 1;
  return static_cast<std::size_t>(at) != target;
}

} // namespace msime::settings::skin_delete
