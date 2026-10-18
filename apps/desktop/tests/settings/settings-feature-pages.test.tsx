// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsFeaturePages,
  useSettingsShellState,
  type CommunityGalleryView,
  type SettingsClient,
  type SettingsPageId,
  type Snapshot,
} from "@msime/ui";
import { useSettingsPageModel } from "../../../../packages/ui/src/settings/use-settings-page-model";
import { testHost } from "../support/host";

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "");
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "shuangpin",
    shuangpin_profile: "xiaohe",
    global_theme: "night",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function client(mobile = true): SettingsClient {
  return {
    load: vi.fn().mockResolvedValue(snapshot),
    save: vi.fn(),
    host: testHost({ platform: mobile ? "harmony" : "macos" }),
    chat: {
      models: vi.fn().mockResolvedValue({ data: [{ id: "fixture" }], defaultModel: "fixture" }),
      complete: vi.fn().mockResolvedValue("合成回复"),
    },
  };
}

function Harness({
  state,
  page,
  gallery = null,
  onOpenPage = vi.fn(),
  onOpenChat,
}: {
  state: SettingsClient;
  page: SettingsPageId;
  gallery?: CommunityGalleryView | null;
  onOpenPage?: (page: string) => void;
  onOpenChat?: () => void;
}) {
  const model = useSettingsPageModel({ client: state });
  const chatState = useSettingsShellState(page);
  return (
    <SettingsFeaturePages
      model={{ ...model, page, communityGallery: gallery }}
      chatState={chatState}
      onOpenPage={onOpenPage}
      onOpenChat={onOpenChat}
    />
  );
}

test("触屏聊天首次进入才挂载，切页保留草稿和进行中的回复", async () => {
  const state = client();
  let resolve!: (value: string) => void;
  state.chat!.complete = vi.fn(() => new Promise<string>((done) => (resolve = done)));
  const view = render(<Harness state={state} page="input" />);
  expect(state.chat!.models).not.toHaveBeenCalled();
  view.rerender(<Harness state={state} page="chat" />);
  const field = screen.getByRole("textbox", { name: "聊天消息" }) as HTMLTextAreaElement;
  await waitFor(() => expect(state.chat!.models).toHaveBeenCalledOnce());
  fireEvent.change(field, { target: { value: "合成草稿" } });
  view.rerender(<Harness state={state} page="input" />);
  expect(screen.queryByRole("textbox", { name: "聊天消息" })).toBeNull();
  expect(field.isConnected).toBe(true);
  view.rerender(<Harness state={state} page="chat" />);
  expect(screen.getByRole("textbox", { name: "聊天消息" })).toBe(field);
  expect(field.value).toBe("合成草稿");

  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  await waitFor(() => expect(state.chat!.complete).toHaveBeenCalledOnce());
  view.rerender(<Harness state={state} page="input" />);
  await act(async () => resolve("合成延迟回复"));
  view.rerender(<Harness state={state} page="chat" />);
  expect(await screen.findByText("合成延迟回复")).toBeTruthy();
  expect(screen.getByRole("textbox", { name: "聊天消息" })).toBe(field);
});

test("桌面聊天离开页面卸载，重开不保留草稿", async () => {
  const state = client(false);
  const view = render(<Harness state={state} page="chat" />);
  const field = screen.getByRole("textbox", { name: "聊天消息" }) as HTMLTextAreaElement;
  await waitFor(() => expect(state.chat!.models).toHaveBeenCalledOnce());
  fireEvent.change(field, { target: { value: "合成桌面草稿" } });
  view.rerender(<Harness state={state} page="input" />);
  expect(field.isConnected).toBe(false);
  view.rerender(<Harness state={state} page="chat" />);
  await waitFor(() => expect(state.chat!.models).toHaveBeenCalledTimes(2));
  expect((screen.getByRole("textbox", { name: "聊天消息" }) as HTMLTextAreaElement).value).toBe("");
});

test("社区子图库按范围 key 重建，离开所属页卸载", async () => {
  const state = client();
  const list = vi.fn().mockResolvedValue({ items: [], has_more: false });
  state.communityResources = {
    list,
    detail: vi.fn(),
    publish: vi.fn(),
    apply: vi.fn(),
    save: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
  };
  const gallery: CommunityGalleryView = { category: "reply", scope: "saved", initialMine: false };
  const view = render(<Harness state={state} page="ai" gallery={gallery} />);
  await waitFor(() => expect(list).toHaveBeenLastCalledWith("reply", "saved", "", 0));
  fireEvent.change(screen.getByRole("textbox", { name: "搜索回复模板" }), {
    target: { value: "合成图库查询" },
  });
  view.rerender(<Harness state={state} page="ai" gallery={{ ...gallery, scope: "mine" }} />);
  await waitFor(() => expect(list).toHaveBeenLastCalledWith("reply", "mine", "", 0));
  expect((screen.getByRole("textbox", { name: "搜索回复模板" }) as HTMLInputElement).value).toBe(
    "",
  );
  view.rerender(<Harness state={state} page="input" gallery={gallery} />);
  expect(screen.queryByRole("dialog", { name: "社区回复模板" })).toBeNull();
});

test("首页继续委托页面导航与聊天入口", async () => {
  const state = client(false);
  state.home = {};
  const onOpenPage = vi.fn();
  const onOpenChat = vi.fn();
  render(<Harness state={state} page="home" onOpenPage={onOpenPage} onOpenChat={onOpenChat} />);
  fireEvent.click(await screen.findByRole("button", { name: /边聊天，边试键盘/ }));
  expect(onOpenChat).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: /输入方案/ }));
  expect(onOpenPage).toHaveBeenCalledWith("input");
});
