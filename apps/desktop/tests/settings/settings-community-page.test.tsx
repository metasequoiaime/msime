// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsCommunityPage,
  type SettingsCommunityPageProps,
  type CommunityResourceClient,
  type CommunitySkinClient,
} from "@msime/ui";
import { useCommunityGalleryNavigation } from "../../../../packages/ui/src/settings/use-community-gallery-navigation";

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "");
});

function resources(): CommunityResourceClient {
  return {
    list: vi.fn().mockResolvedValue({ items: [], has_more: false }),
    detail: vi.fn(),
    publish: vi.fn(),
    apply: vi.fn(),
    save: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
  };
}

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

type Model = Omit<SettingsCommunityPageProps["model"], "communityTabs">;

function model(): Model {
  return {
    client: {
      load: vi.fn(),
      save: vi.fn(),
      communityResources: resources(),
      communitySkins: skins(),
    },
    draft: undefined,
    setDraft: vi.fn(),
    communityDestination: "all",
    initialCommunityMine: false,
    initialCommunityCategory: "skin",
    initialCommunityScope: "",
    keyboardPreviewTheme: "light",
    mobilePlatform: true,
    harmonyPlatform: true,
    openAccountLogin: vi.fn(),
  };
}

function Harness({ state, active = true }: { state: Model; active?: boolean }) {
  const { communityTabs } = useCommunityGalleryNavigation({
    mobile: state.mobilePlatform,
    hasCommunityPage: true,
    hasSkins: Boolean(state.client.communitySkins),
    hasResources: Boolean(state.client.communityResources),
    selectPage: vi.fn(),
    openSubview: vi.fn(),
  });
  return active ? <SettingsCommunityPage model={{ ...state, communityTabs }} /> : null;
}

test("桌面社区保留初始分类直到外壳目的地变化", async () => {
  const state = model();
  state.mobilePlatform = false;
  state.harmonyPlatform = false;
  state.initialCommunityCategory = "reply";
  state.initialCommunityScope = "saved";
  const client = state.client.communityResources!;
  const view = render(<Harness state={state} />);
  await waitFor(() => expect(client.list).toHaveBeenLastCalledWith("reply", "saved", "", 0));
  const next = {
    ...state,
    initialCommunityCategory: "dictionary" as const,
    initialCommunityScope: "mine" as const,
  };
  view.rerender(<Harness state={next} />);
  expect(screen.getByRole("tab", { name: "回复模板" }).getAttribute("aria-selected")).toBe("true");
  view.rerender(<Harness state={{ ...next, communityDestination: "saved-dictionary" }} />);
  await waitFor(() => expect(client.list).toHaveBeenLastCalledWith("dictionary", "mine", "", 0));
  expect(screen.getByRole("tab", { name: "词库" }).getAttribute("aria-selected")).toBe("true");
});

test("触屏分类跨根页卸载保留，未选图库不挂载且返回后重置搜索", async () => {
  const state = model();
  const skinClient = state.client.communitySkins!;
  const resourceClient = state.client.communityResources!;
  const view = render(<Harness state={state} />);
  await waitFor(() => expect(skinClient.list).toHaveBeenCalledOnce());
  expect(resourceClient.list).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("tab", { name: "词库" }));
  await waitFor(() => expect(resourceClient.list).toHaveBeenCalledOnce());
  fireEvent.change(screen.getByRole("textbox", { name: "搜索词库" }), {
    target: { value: "合成查询" },
  });
  view.rerender(<Harness state={state} active={false} />);
  expect(screen.queryByRole("textbox", { name: "搜索词库" })).toBeNull();
  view.rerender(<Harness state={state} />);
  await waitFor(() => expect(resourceClient.list).toHaveBeenCalledTimes(2));
  expect(screen.getByRole("tab", { name: "词库" }).getAttribute("aria-selected")).toBe("true");
  expect((screen.getByRole("textbox", { name: "搜索词库" }) as HTMLInputElement).value).toBe("");
  expect(skinClient.list).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("tab", { name: "皮肤" }));
  await waitFor(() => expect(skinClient.list).toHaveBeenCalledTimes(2));
});

test.each(["skin", "dictionary"] as const)(
  "触屏只有 %s 服务时直接显示图库且无分类标签",
  async (kind) => {
    const state = model();
    if (kind === "skin") state.client.communityResources = undefined;
    else state.client.communitySkins = undefined;
    const client =
      kind === "skin" ? state.client.communitySkins! : state.client.communityResources!;
    render(<Harness state={state} />);
    await waitFor(() => expect(client.list).toHaveBeenCalledOnce());
    expect(screen.queryByRole("tablist", { name: "社区分类" })).toBeNull();
    expect(
      screen.getByRole("textbox", { name: kind === "skin" ? "搜索皮肤设计" : "搜索词库" }),
    ).toBeDefined();
  },
);
