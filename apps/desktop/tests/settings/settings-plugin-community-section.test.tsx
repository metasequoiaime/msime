// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsPluginCommunitySection,
  type SettingsPluginCommunitySectionProps,
  type SettingsShellView,
} from "@msime/ui";
import { useSettingsCommunityViews } from "../../../../packages/ui/src/settings/use-settings-community-views";

afterEach(cleanup);

function model(): SettingsPluginCommunitySectionProps["model"] {
  return {
    client: {
      load: vi.fn(),
      save: vi.fn(),
      communityPlugins: {
        list: vi.fn().mockResolvedValue({ plugins: [], has_more: false }),
        detail: vi.fn(),
        packPreview: vi.fn(),
        publish: vi.fn(),
        install: vi.fn(),
        rate: vi.fn(),
        delete: vi.fn(),
      },
    },
    page: "plugins",
    openAccountLogin: vi.fn(),
  };
}

function Harness({ state }: { state: SettingsPluginCommunitySectionProps["model"] }) {
  const [pluginView, setPluginView] = useState<SettingsShellView>("mine");
  const views = useSettingsCommunityViews({
    page: state.page,
    mobile: false,
    harmony: false,
    skinView: "mine",
    onSkinViewChange: vi.fn(),
    pluginView,
    hasCommunitySkins: false,
    hasCommunityCandidateSkins: false,
    hasCommunityPlugins: Boolean(state.client.communityPlugins),
  });
  return (
    <SettingsPluginCommunitySection
      model={state}
      views={views}
      pluginView={pluginView}
      onPluginViewChange={setPluginView}
    />
  );
}

test("来源由外层控制，切回本地卸载图库并在重开后清空搜索", async () => {
  const state = model();
  const client = state.client.communityPlugins!;
  render(<Harness state={state} />);
  expect(client.list).not.toHaveBeenCalled();
  expect(screen.getByRole("tab", { name: "我的插件" }).getAttribute("aria-selected")).toBe("true");
  fireEvent.click(screen.getByRole("tab", { name: "社区插件" }));
  await waitFor(() => expect(client.list).toHaveBeenCalledOnce());
  fireEvent.change(screen.getByRole("textbox", { name: "搜索插件" }), {
    target: { value: "合成查询" },
  });
  fireEvent.click(screen.getByRole("tab", { name: "我的插件" }));
  expect(screen.queryByRole("textbox", { name: "搜索插件" })).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "社区插件" }));
  await waitFor(() => expect(client.list).toHaveBeenCalledTimes(2));
  expect((screen.getByRole("textbox", { name: "搜索插件" }) as HTMLInputElement).value).toBe("");
});

test("离开插件页隐藏来源与图库，返回保留外层社区分类", async () => {
  const state = model();
  const client = state.client.communityPlugins!;
  const view = render(<Harness state={state} />);
  fireEvent.click(screen.getByRole("tab", { name: "社区插件" }));
  await waitFor(() => expect(client.list).toHaveBeenCalledOnce());
  view.rerender(<Harness state={{ ...state, page: "input" }} />);
  expect(screen.queryByRole("tablist", { name: "插件来源" })).toBeNull();
  expect(screen.queryByRole("textbox", { name: "搜索插件" })).toBeNull();
  view.rerender(<Harness state={state} />);
  await waitFor(() => expect(client.list).toHaveBeenCalledTimes(2));
  expect(screen.getByRole("tab", { name: "社区插件" }).getAttribute("aria-selected")).toBe("true");
});

test("没有社区服务时隐藏来源与图库", () => {
  const state = model();
  state.client.communityPlugins = undefined;
  render(<Harness state={state} />);
  expect(screen.queryByRole("tablist", { name: "插件来源" })).toBeNull();
  expect(screen.queryByRole("textbox", { name: "搜索插件" })).toBeNull();
});
