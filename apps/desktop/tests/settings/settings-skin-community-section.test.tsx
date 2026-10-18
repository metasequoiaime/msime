// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  SettingsSkinCommunitySection,
  type SettingsSkinCommunitySectionProps,
  type CandidateSkinCommunityClient,
  type CommunitySkinClient,
} from "@msime/ui";
import { useSettingsCommunityViews } from "../../../../packages/ui/src/settings/use-settings-community-views";
import type { DesktopSkinTab } from "../../../../packages/ui/src/settings/desktop-skin-tabs";

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "");
});

function skins(): CommunitySkinClient {
  return {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn(),
    download: vi.fn(),
    rate: vi.fn(),
    publish: vi.fn(),
    unpublish: vi.fn(),
    setCategory: vi.fn(),
    finishTrial: vi.fn(),
  };
}

function candidateSkins(): CandidateSkinCommunityClient {
  return {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn(),
    preview: vi.fn(),
    install: vi.fn(),
    packPreview: vi.fn(),
    addPreview: vi.fn(),
    addLicense: vi.fn(),
    publish: vi.fn(),
    setVisibility: vi.fn(),
    setCategory: vi.fn(),
    sync: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
  };
}

function model(): SettingsSkinCommunitySectionProps["model"] {
  return {
    client: {
      load: vi.fn(),
      save: vi.fn(),
      communitySkins: skins(),
      communityCandidateSkins: candidateSkins(),
    },
    page: "skin",
    draft: undefined,
    setDraft: vi.fn(),
    keyboardPreviewTheme: "light",
    openAccountLogin: vi.fn(),
  };
}

function Harness({
  state,
  mobile = false,
}: {
  state: SettingsSkinCommunitySectionProps["model"];
  mobile?: boolean;
}) {
  const [skinView, setSkinView] = useState<DesktopSkinTab>("mine");
  const views = useSettingsCommunityViews({
    page: state.page,
    mobile,
    harmony: mobile,
    skinView,
    onSkinViewChange: setSkinView,
    pluginView: "mine",
    hasCommunitySkins: Boolean(state.client.communitySkins),
    hasCommunityCandidateSkins: Boolean(state.client.communityCandidateSkins),
    hasCommunityPlugins: false,
  });
  return (
    <SettingsSkinCommunitySection model={state} views={views} onSkinViewChange={setSkinView} />
  );
}

test("桌面分类使用外层受控状态，离开皮肤页不挂载图库", async () => {
  const state = model();
  const client = state.client.communityCandidateSkins!;
  const view = render(<Harness state={state} />);
  expect(client.list).not.toHaveBeenCalled();
  const local = screen.getByRole("tab", { name: "本地皮肤" });
  fireEvent.keyDown(local, { key: "ArrowRight" });
  const community = screen.getByRole("tab", { name: "社区皮肤" });
  await waitFor(() => expect(client.list).toHaveBeenCalledOnce());
  expect(document.activeElement).toBe(community);
  expect(screen.getByRole("tabpanel", { name: "社区皮肤" }).id).toBe(
    community.getAttribute("aria-controls"),
  );
  view.rerender(<Harness state={{ ...state, page: "input" }} />);
  expect(screen.queryByRole("tablist", { name: "皮肤来源" })).toBeNull();
  expect(screen.queryByRole("heading", { name: "社区皮肤" })).toBeNull();
  view.rerender(<Harness state={state} />);
  await waitFor(() => expect(client.list).toHaveBeenCalledTimes(2));
  fireEvent.keyDown(screen.getByRole("tab", { name: "社区皮肤" }), { key: "Home" });
  expect(screen.getByRole("tab", { name: "本地皮肤" }).getAttribute("aria-selected")).toBe("true");
  expect(screen.queryByRole("heading", { name: "社区皮肤" })).toBeNull();
});

test("触屏皮肤图库在切离页面时卸载，返回保留分类并重置搜索", async () => {
  const state = model();
  state.client.communityCandidateSkins = undefined;
  const client = state.client.communitySkins!;
  const view = render(<Harness state={state} mobile />);
  expect(client.list).not.toHaveBeenCalled();
  const tabs = screen.getByRole("tablist", { name: "皮肤分类" });
  fireEvent.click(within(tabs).getByRole("tab", { name: "社区" }));
  await waitFor(() => expect(client.list).toHaveBeenCalledOnce());
  fireEvent.change(screen.getByRole("textbox", { name: "搜索皮肤设计" }), {
    target: { value: "合成查询" },
  });
  view.rerender(<Harness state={{ ...state, page: "input" }} mobile />);
  expect(screen.queryByRole("textbox", { name: "搜索皮肤设计" })).toBeNull();
  view.rerender(<Harness state={state} mobile />);
  await waitFor(() => expect(client.list).toHaveBeenCalledTimes(2));
  expect(screen.getByRole("tab", { name: "社区" }).getAttribute("aria-selected")).toBe("true");
  expect((screen.getByRole("textbox", { name: "搜索皮肤设计" }) as HTMLInputElement).value).toBe(
    "",
  );
});

test.each([false, true])("无社区服务时 mobile=%s 不显示社区分类或图库", (mobile) => {
  const state = model();
  state.client.communitySkins = undefined;
  state.client.communityCandidateSkins = undefined;
  render(<Harness state={state} mobile={mobile} />);
  expect(screen.queryByRole("tab", { name: mobile ? "社区" : "社区皮肤" })).toBeNull();
  expect(screen.queryByRole("search")).toBeNull();
  expect(
    screen.getByRole("tab", { name: mobile ? "内置" : "本地皮肤" }).getAttribute("aria-selected"),
  ).toBe("true");
});
