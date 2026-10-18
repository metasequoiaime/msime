// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useSettingsCommunityViews } from "../../../../packages/ui/src/settings/use-settings-community-views";
import type { SettingsShellView } from "../../../../packages/ui/src/settings/use-settings-shell-state";

test("鸿蒙手机皮肤页的社区分类显示完整社区图库", () => {
  const { result } = renderHook(() =>
    useSettingsCommunityViews({
      page: "skin",
      mobile: true,
      harmony: true,
      onSkinViewChange: vi.fn(),
      skinView: "mine",
      pluginView: "mine",
      hasCommunitySkins: true,
      hasCommunityCandidateSkins: false,
      hasCommunityPlugins: false,
    }),
  );

  expect(result.current.phoneSkinCommunityShown).toBe(false);
  expect(result.current.skinTabs.tabList).toBeTruthy();
  act(() => result.current.skinTabs.select("community"));
  expect(result.current.skinTabs.selected).toBe("community");
  expect(result.current.phoneSkinCommunityShown).toBe(true);
});

test("候选皮肤图库和手机皮肤图库都算作皮肤页覆盖视图", () => {
  const view: SettingsShellView = "community";
  const { result: desktop } = renderHook(() =>
    useSettingsCommunityViews({
      page: "skin",
      mobile: false,
      harmony: false,
      onSkinViewChange: vi.fn(),
      skinView: view,
      pluginView: "mine",
      hasCommunitySkins: false,
      hasCommunityCandidateSkins: true,
      hasCommunityPlugins: false,
    }),
  );
  expect(desktop.current.skinCommunityShown).toBe(true);

  const { result: phone } = renderHook(() =>
    useSettingsCommunityViews({
      page: "skin",
      mobile: true,
      harmony: true,
      onSkinViewChange: vi.fn(),
      skinView: view,
      pluginView: "mine",
      hasCommunitySkins: true,
      hasCommunityCandidateSkins: false,
      hasCommunityPlugins: false,
    }),
  );
  act(() => phone.current.skinTabs.select("community"));
  expect(phone.current.skinCommunityShown).toBe(true);
});

test("插件社区视图只在插件页且服务可用时覆盖设置表单", () => {
  const { result } = renderHook(() =>
    useSettingsCommunityViews({
      page: "plugins",
      mobile: false,
      harmony: false,
      onSkinViewChange: vi.fn(),
      skinView: "mine",
      pluginView: "community",
      hasCommunitySkins: false,
      hasCommunityCandidateSkins: false,
      hasCommunityPlugins: true,
    }),
  );
  expect(result.current.pluginCommunityShown).toBe(true);
});
