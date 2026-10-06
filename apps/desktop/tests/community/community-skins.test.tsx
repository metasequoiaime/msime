// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  CommunitySkinsPage,
  SettingsPage,
  type CommunitySkin,
  type CommunitySkinClient,
  type CommunitySkinDownload,
  type CommunitySkinPage,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

const design = {
  background: 0xe8f0eb,
  keyBackground: 0xffffff,
  keyForeground: 0x17251d,
  accent: 0x185c47,
  actionBackground: 0x185c47,
  cornerRadius: 8,
  borderWidth: 0,
  shadow: 0,
  pattern: 0 as const,
  monospaced: false,
};

function skin(id: string, name: string): CommunitySkin {
  return {
    id,
    name,
    description: `${name} 的公开说明`,
    author: "示例作者",
    design,
    downloads: 12,
    rating_count: 2,
    rating_average: 4.5,
    owned: false,
    my_rating: 0,
  };
}

function client(overrides: Partial<CommunitySkinClient> = {}): CommunitySkinClient {
  return {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn().mockImplementation(async (id) => skin(id, "详情皮肤")),
    download: vi.fn().mockResolvedValue({
      skin: skin("10000000-0000-4000-8000-000000000099", "下载皮肤"),
      trial: { id: "trial", name: "下载皮肤" },
    }),
    rate: vi.fn().mockResolvedValue(undefined),
    publish: vi.fn().mockResolvedValue(undefined),
    unpublish: vi.fn().mockResolvedValue(undefined),
    setCategory: vi.fn().mockImplementation(async (id: string, category) => ({
      ...skin(id, "我的皮肤"),
      owned: true,
      category,
    })),
    finishTrial: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

test("settings expose community only with the Android capability and omit preference actions", async () => {
  const without = render(
    <SettingsPage client={{ load: async () => preferences, save: vi.fn() }} />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: "社区" })).toBeNull();
  without.unmount();

  const communitySkins = client();
  render(
    <SettingsPage
      client={{ load: async () => preferences, save: vi.fn(), communitySkins }}
      initialPage="community"
    />,
  );
  expect(await screen.findByRole("heading", { name: "社区" })).not.toBeNull();
  await waitFor(() => expect(communitySkins.list).toHaveBeenCalledWith(0, "", false, null));
  expect(screen.queryByRole("form", { name: "设置" })).toBeNull();
  expect(screen.queryByRole("button", { name: "重新读取" })).toBeNull();
});

test("initial load and submitted search preserve the exact query", async () => {
  const communitySkins = client();
  render(<CommunitySkinsPage client={communitySkins} theme="light" />);
  await waitFor(() => expect(communitySkins.list).toHaveBeenCalledWith(0, "", false, null));
  fireEvent.change(screen.getByRole("textbox", { name: "搜索皮肤设计" }), {
    target: { value: " C++ 星 " },
  });
  fireEvent.click(screen.getByRole("button", { name: "搜索" }));
  await waitFor(() =>
    expect(communitySkins.list).toHaveBeenLastCalledWith(0, " C++ 星 ", false, null),
  );
});

test("load more advances the transport offset and removes duplicate ids", async () => {
  const first = skin("10000000-0000-4000-8000-000000000001", "第一款");
  const second = skin("10000000-0000-4000-8000-000000000002", "第二款");
  const third = skin("10000000-0000-4000-8000-000000000003", "第三款");
  const list = vi
    .fn()
    .mockResolvedValueOnce({ skins: [first, second], has_more: true })
    .mockResolvedValueOnce({ skins: [second, third], has_more: false });
  render(<CommunitySkinsPage client={client({ list })} theme="dark" />);
  fireEvent.click(await screen.findByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(2, "", false, null));
  expect(await screen.findByRole("button", { name: "查看皮肤 第三款" })).not.toBeNull();
  expect(screen.getAllByRole("button", { name: /查看皮肤/ })).toHaveLength(3);
});

test("an older response cannot replace a newer search", async () => {
  const oldPage = deferred<CommunitySkinPage>();
  const newer = skin("10000000-0000-4000-8000-000000000004", "新搜索结果");
  const list = vi
    .fn()
    .mockReturnValueOnce(oldPage.promise)
    .mockResolvedValueOnce({ skins: [newer], has_more: false });
  render(<CommunitySkinsPage client={client({ list })} theme="dark" />);
  await waitFor(() => expect(list).toHaveBeenCalledTimes(1));
  fireEvent.change(screen.getByRole("textbox", { name: "搜索皮肤设计" }), {
    target: { value: "新查询" },
  });
  fireEvent.submit(screen.getByRole("search"));
  expect(await screen.findByRole("button", { name: "查看皮肤 新搜索结果" })).not.toBeNull();
  oldPage.resolve({
    skins: [skin("10000000-0000-4000-8000-000000000005", "迟到旧结果")],
    has_more: false,
  });
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: "查看皮肤 迟到旧结果" })).toBeNull(),
  );
  expect(screen.getByRole("button", { name: "查看皮肤 新搜索结果" })).not.toBeNull();
});

test("opening a card refreshes detail without mutating preferences or the local library", async () => {
  const original = skin("10000000-0000-4000-8000-000000000006", "列表名称");
  const detail = vi
    .fn()
    .mockResolvedValue({ ...original, name: "详情名称", description: "详细说明" });
  const communitySkins = client({
    list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
    detail,
  });
  const save = vi.fn();
  const mutate = vi.fn();
  render(
    <SettingsPage
      client={{
        load: async () => preferences,
        save,
        communitySkins,
        customSkinLibrary: { load: vi.fn().mockResolvedValue([]), mutate },
      }}
      initialPage="community"
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看皮肤 列表名称" }));
  await waitFor(() => expect(detail).toHaveBeenCalledWith(original.id));
  expect(await screen.findByRole("heading", { name: "详情名称" })).not.toBeNull();
  expect(screen.getByText("详细说明")).not.toBeNull();
  expect(save).not.toHaveBeenCalled();
  expect(mutate).not.toHaveBeenCalled();
});

test("mobile skin details join the WebView history stack and system back restores the list", async () => {
  window.history.replaceState({ msimeSettings: true, page: "community" }, "");
  const original = skin("10000000-0000-4000-8000-000000000060", "历史皮肤");
  render(
    <CommunitySkinsPage
      client={client({
        list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
        detail: vi.fn().mockResolvedValue(original),
      })}
      theme="light"
      mobile
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: `查看皮肤 ${original.name}` }));
  expect(window.history.state.communityDetail).toEqual({ kind: "skin", id: original.id });
  expect(await screen.findByRole("button", { name: "返回社区" })).not.toBeNull();
  const state = { msimeSettings: true, page: "community" };
  window.history.replaceState(state, "");
  window.dispatchEvent(new PopStateEvent("popstate", { state }));
  expect(await screen.findByRole("button", { name: `查看皮肤 ${original.name}` })).not.toBeNull();
});

test("community failures use stable messages and never expose backend details", async () => {
  const communitySkins = client({
    list: vi
      .fn()
      .mockRejectedValue({ code: "community_rate_limited", message: "private backend detail" }),
  });
  render(<CommunitySkinsPage client={communitySkins} theme="dark" />);
  expect((await screen.findByRole("alert")).textContent).toBe("请求过于频繁，请稍后再试。");
  expect(screen.queryByText(/private backend detail/)).toBeNull();
});

test("downloads enter a recoverable trial and can restore the previous skin", async () => {
  const original = skin("10000000-0000-4000-8000-000000000007", "可试用皮肤");
  const downloaded = { ...original, design: { ...design, background: 0x123456 } };
  const download = vi.fn().mockResolvedValue({
    skin: downloaded,
    trial: { id: "20000000-0000-4000-8000-000000000001", name: original.name },
  });
  const finishTrial = vi.fn().mockResolvedValue(undefined);
  render(
    <CommunitySkinsPage
      client={client({
        list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
        detail: vi.fn().mockResolvedValue(original),
        download,
        finishTrial,
      })}
      theme="dark"
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: `查看皮肤 ${original.name}` }));
  fireEvent.click(await screen.findByRole("button", { name: "下载并试用" }));
  await waitFor(() => expect(download).toHaveBeenCalledWith(original.id, original.name));
  expect(await screen.findByText(`正在试用：${original.name}`)).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "恢复原皮肤" }));
  await waitFor(() =>
    expect(finishTrial).toHaveBeenCalledWith("20000000-0000-4000-8000-000000000001", false),
  );
  expect(await screen.findByText("已恢复试用前的皮肤。")).not.toBeNull();
});

test("ignores a second skin download while the first is pending", async () => {
  const original = skin("10000000-0000-4000-8000-000000000071", "重复下载皮肤");
  const pending = deferred<CommunitySkinDownload>();
  const download = vi.fn().mockReturnValue(pending.promise);
  render(
    <CommunitySkinsPage
      client={client({
        list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
        detail: vi.fn().mockResolvedValue(original),
        download,
      })}
      theme="dark"
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: `查看皮肤 ${original.name}` }));
  const button = await screen.findByRole("button", { name: "下载并试用" });
  act(() => {
    fireEvent.click(button);
    fireEvent.click(button);
  });
  expect(download).toHaveBeenCalledOnce();
  pending.resolve({
    skin: original,
    trial: { id: "trial-duplicate", name: original.name },
  });
  await waitFor(() => expect(screen.getByText(`正在试用：${original.name}`)).toBeTruthy());
});

test("a download response from a replaced community client is ignored", async () => {
  const original = skin("10000000-0000-4000-8000-000000000070", "替换客户端皮肤");
  const pending = deferred<CommunitySkinDownload>();
  const oldClient = client({
    list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
    detail: vi.fn().mockResolvedValue(original),
    download: vi.fn().mockReturnValue(pending.promise),
  });
  const replacement = client({
    list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
    detail: vi.fn().mockResolvedValue(original),
  });
  const view = render(<CommunitySkinsPage client={oldClient} theme="dark" />);
  fireEvent.click(await screen.findByRole("button", { name: `查看皮肤 ${original.name}` }));
  fireEvent.click(await screen.findByRole("button", { name: "下载并试用" }));
  await waitFor(() => expect(oldClient.download).toHaveBeenCalledWith(original.id, original.name));
  view.rerender(<CommunitySkinsPage client={replacement} theme="dark" />);
  pending.resolve({
    skin: { ...original, design: { ...design, background: 0x123456 } },
    trial: { id: "stale-trial", name: original.name },
  });
  await waitFor(() => expect(screen.queryByText(`正在试用：${original.name}`)).toBeNull());
});

test("replacing the community client clears an active skin trial", async () => {
  const original = skin("10000000-0000-4000-8000-000000000071", "替换客户端试用");
  const oldFinishTrial = vi.fn().mockResolvedValue(undefined);
  const oldClient = client({
    list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
    detail: vi.fn().mockResolvedValue(original),
    download: vi.fn().mockResolvedValue({
      skin: original,
      trial: { id: "stale-trial", name: original.name },
    }),
    finishTrial: oldFinishTrial,
  });
  const replacementFinishTrial = vi.fn().mockResolvedValue(undefined);
  const replacement = client({
    list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
    detail: vi.fn().mockResolvedValue(original),
    finishTrial: replacementFinishTrial,
  });
  const view = render(<CommunitySkinsPage client={oldClient} theme="dark" />);
  fireEvent.click(await screen.findByRole("button", { name: `查看皮肤 ${original.name}` }));
  fireEvent.click(await screen.findByRole("button", { name: "下载并试用" }));
  await screen.findByText(`正在试用：${original.name}`);

  view.rerender(<CommunitySkinsPage client={replacement} theme="dark" />);
  await waitFor(() => expect(oldFinishTrial).toHaveBeenCalledWith("stale-trial", false));
  expect(screen.queryByText(`正在试用：${original.name}`)).toBeNull();
  expect(replacementFinishTrial).not.toHaveBeenCalled();
});

test("leaving an active trial restores it and keeping it suppresses later recovery", async () => {
  const original = skin("10000000-0000-4000-8000-000000000008", "退出恢复皮肤");
  const finishTrial = vi.fn().mockResolvedValue(undefined);
  const communitySkins = client({
    list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
    detail: vi.fn().mockResolvedValue(original),
    download: vi.fn().mockResolvedValue({
      skin: original,
      trial: { id: "20000000-0000-4000-8000-000000000002", name: original.name },
    }),
    finishTrial,
  });
  const view = render(<CommunitySkinsPage client={communitySkins} theme="light" />);
  fireEvent.click(await screen.findByRole("button", { name: `查看皮肤 ${original.name}` }));
  fireEvent.click(await screen.findByRole("button", { name: "下载并试用" }));
  await screen.findByRole("button", { name: "保留使用" });
  view.unmount();
  await waitFor(() =>
    expect(finishTrial).toHaveBeenCalledWith("20000000-0000-4000-8000-000000000002", false),
  );
});

test("rating refreshes detail while owned skins omit rating controls", async () => {
  const original = skin("10000000-0000-4000-8000-000000000009", "评分皮肤");
  const rated = { ...original, my_rating: 4, rating_count: 3, rating_average: 4.3 };
  const detail = vi.fn().mockResolvedValueOnce(original).mockResolvedValueOnce(rated);
  const rate = vi.fn().mockResolvedValue(undefined);
  const view = render(
    <CommunitySkinsPage
      client={client({
        list: vi.fn().mockResolvedValue({ skins: [original], has_more: false }),
        detail,
        rate,
      })}
      theme="dark"
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: `查看皮肤 ${original.name}` }));
  fireEvent.click(await screen.findByRole("button", { name: "评 4 星" }));
  await waitFor(() => expect(rate).toHaveBeenCalledWith(original.id, 4));
  expect(await screen.findByText("我的评分：4 星")).not.toBeNull();
  view.unmount();

  const owned = { ...original, id: "10000000-0000-4000-8000-000000000010", owned: true };
  render(
    <CommunitySkinsPage
      client={client({
        list: vi.fn().mockResolvedValue({ skins: [owned], has_more: false }),
        detail: vi.fn().mockResolvedValue(owned),
      })}
      theme="dark"
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: `查看皮肤 ${owned.name}` }));
  await screen.findByRole("heading", { name: owned.name });
  expect(screen.queryByRole("button", { name: "评 4 星" })).toBeNull();
});

test("publishes a selected local design only after explicit rights confirmation", async () => {
  const local = { id: "30000000-0000-4000-8000-000000000001", name: "我的森林", design };
  const publish = vi.fn().mockResolvedValue(undefined);
  const list = vi.fn().mockResolvedValue({ skins: [], has_more: false });
  render(
    <CommunitySkinsPage
      client={client({ list, publish })}
      theme="light"
      localSkinLibrary={{
        load: vi.fn().mockResolvedValue([local]),
        mutate: vi.fn(),
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的设计" }));
  expect(await screen.findByRole("dialog", { name: "发布我的皮肤" })).not.toBeNull();
  const submit = screen.getByRole("button", { name: "公开发布" }) as HTMLButtonElement;
  expect(submit.disabled).toBe(true);
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));
  fireEvent.click(submit);
  await waitFor(() =>
    expect(publish).toHaveBeenCalledWith(
      expect.stringMatching(/^[0-9a-f-]{36}$/),
      "我的森林",
      "",
      design,
      "other",
    ),
  );
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "发布我的皮肤" })).toBeNull());
});

test("publish dialog ignores a same-tick duplicate submission", async () => {
  const local = { id: "30000000-0000-4000-0002", name: "重复提交设计", design };
  const pending = deferred<void>();
  const publish = vi.fn().mockReturnValue(pending.promise);
  render(
    <CommunitySkinsPage
      client={client({ publish })}
      theme="light"
      localSkinLibrary={{
        load: vi.fn().mockResolvedValue([local]),
        mutate: vi.fn(),
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的设计" }));
  await screen.findByRole("dialog", { name: "发布我的皮肤" });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));

  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
    fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  });
  expect(publish).toHaveBeenCalledOnce();
  pending.resolve();
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "发布我的皮肤" })).toBeNull());
});

test("a publish response from a replaced community client is ignored", async () => {
  const local = { id: "30000000-0000-4000-0001", name: "替换发布设计", design };
  const pending = deferred<void>();
  const oldClient = client({
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    publish: vi.fn().mockReturnValue(pending.promise),
  });
  const replacement = client({ list: vi.fn().mockResolvedValue({ skins: [], has_more: false }) });
  const view = render(
    <CommunitySkinsPage
      client={oldClient}
      theme="light"
      localSkinLibrary={{ load: vi.fn().mockResolvedValue([local]), mutate: vi.fn() }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的设计" }));
  await screen.findByText("替换发布设计");
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  await waitFor(() => expect(oldClient.publish).toHaveBeenCalled());
  view.rerender(
    <CommunitySkinsPage
      client={replacement}
      theme="light"
      localSkinLibrary={{ load: vi.fn().mockResolvedValue([local]), mutate: vi.fn() }}
    />,
  );
  pending.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(screen.getByRole("dialog", { name: "发布我的皮肤" })).not.toBeNull();
  expect(screen.queryByText("已发布到社区。")).toBeNull();
});

test("a download refused for want of a sign-in offers the way there", async () => {
  // Browsing works signed out and downloading does not, so this is the first wall a new user meets.
  // It used to say the login had expired and leave them to find the account page themselves.
  const offered = skin("10000000-0000-4000-8000-000000000011", "需要登录的皮肤");
  const download = vi.fn().mockRejectedValue({ code: "community_unauthorized" });
  const onLogin = vi.fn();
  render(
    <CommunitySkinsPage
      client={client({
        list: vi.fn().mockResolvedValue({ skins: [offered], has_more: false }),
        detail: vi.fn().mockResolvedValue(offered),
        download,
      })}
      theme="light"
      onLogin={onLogin}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: `查看皮肤 ${offered.name}` }));
  fireEvent.click(await screen.findByRole("button", { name: "下载并试用" }));
  fireEvent.click(await screen.findByRole("button", { name: "去登录" }));
  expect(onLogin).toHaveBeenCalledTimes(1);
});

test("a publish refused for want of a sign-in offers the way there", async () => {
  // MSIME-Apple's SavedSkinPublishFlow puts the sign-in form in front of the user rather than
  // telling them to go and find it. The shared dialog cannot show a form, but it can be the one tap.
  const local = { id: "30000000-0000-4000-8000-000000000003", name: "待发布", design };
  const publish = vi.fn().mockRejectedValue({ code: "community_unauthorized" });
  const onLogin = vi.fn();
  render(
    <CommunitySkinsPage
      client={client({ publish })}
      theme="light"
      onLogin={onLogin}
      localSkinLibrary={{
        load: vi.fn().mockResolvedValue([local]),
        mutate: vi.fn(),
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的设计" }));
  fireEvent.click(await screen.findByRole("checkbox", { name: "确认拥有发布素材权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  const go = await screen.findByRole("button", { name: "去登录" });
  expect(onLogin).not.toHaveBeenCalled();
  fireEvent.click(go);
  expect(onLogin).toHaveBeenCalledTimes(1);
  // The dialog gets out of the way, or the account page opens behind a modal nobody can see past.
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "发布我的皮肤" })).toBeNull());
});

test("only a sign-in failure offers the sign-in button", async () => {
  const local = { id: "30000000-0000-4000-8000-000000000004", name: "冲突设计", design };
  const publish = vi.fn().mockRejectedValue({ code: "community_conflict" });
  render(
    <CommunitySkinsPage
      client={client({ publish })}
      theme="light"
      onLogin={vi.fn()}
      localSkinLibrary={{
        load: vi.fn().mockResolvedValue([local]),
        mutate: vi.fn(),
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的设计" }));
  fireEvent.click(await screen.findByRole("checkbox", { name: "确认拥有发布素材权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  await screen.findByRole("alert");
  expect(screen.queryByRole("button", { name: "去登录" })).toBeNull();
});

test("keeps the publication id for retries but changes it when metadata changes", async () => {
  const local = { id: "30000000-0000-4000-8000-000000000002", name: "可重试设计", design };
  const publish = vi.fn().mockRejectedValue(new Error("temporary failure"));
  render(
    <CommunitySkinsPage
      client={client({ publish })}
      theme="light"
      localSkinLibrary={{
        load: vi.fn().mockResolvedValue([local]),
        mutate: vi.fn(),
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的设计" }));
  fireEvent.click(await screen.findByRole("checkbox", { name: "确认拥有发布素材权利" }));
  const submit = screen.getByRole("button", { name: "公开发布" });
  fireEvent.click(submit);
  await waitFor(() => expect(publish).toHaveBeenCalledTimes(1));
  fireEvent.click(submit);
  await waitFor(() => expect(publish).toHaveBeenCalledTimes(2));
  expect(publish.mock.calls[1][0]).toBe(publish.mock.calls[0][0]);

  fireEvent.change(screen.getByRole("textbox", { name: "发布皮肤名称" }), {
    target: { value: "另一款设计" },
  });
  fireEvent.click(submit);
  await waitFor(() => expect(publish).toHaveBeenCalledTimes(3));
  expect(publish.mock.calls[2][0]).not.toBe(publish.mock.calls[0][0]);
});

test("my works scope filters owned skins and can unpublish with confirmation", async () => {
  const mine = { ...skin("40000000-0000-4000-8000-000000000001", "我的公开皮肤"), owned: true };
  const publicSkin = skin("40000000-0000-4000-8000-000000000002", "别人的皮肤");
  const list = vi.fn().mockResolvedValue({ skins: [mine, publicSkin], has_more: false });
  const detail = vi.fn().mockResolvedValue(mine);
  const unpublish = vi.fn().mockResolvedValue(undefined);
  render(<CommunitySkinsPage client={client({ list, detail, unpublish })} theme="dark" />);
  fireEvent.click(await screen.findByRole("button", { name: "我的作品" }));
  expect(await screen.findByRole("button", { name: "查看皮肤 我的公开皮肤" })).not.toBeNull();
  expect(screen.queryByRole("button", { name: "查看皮肤 别人的皮肤" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "查看皮肤 我的公开皮肤" }));
  fireEvent.click(await screen.findByRole("button", { name: "下架这款皮肤" }));
  expect(screen.getByRole("alertdialog", { name: "确认下架皮肤" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "确认下架" }));
  await waitFor(() => expect(unpublish).toHaveBeenCalledWith(mine.id));
  await waitFor(() => expect(list).toHaveBeenCalledTimes(3));
});

test("category chips filter the list from its first page and load more keeps the category", async () => {
  const first = skin("10000000-0000-4000-8000-000000000001", "第一款");
  const second = skin("10000000-0000-4000-8000-000000000002", "第二款");
  const third = skin("10000000-0000-4000-8000-000000000003", "第三款");
  const list = vi
    .fn()
    .mockResolvedValueOnce({ skins: [first, second], has_more: true })
    .mockResolvedValueOnce({ skins: [second], has_more: true })
    .mockResolvedValueOnce({ skins: [third], has_more: false })
    .mockResolvedValueOnce({ skins: [first], has_more: false });
  render(<CommunitySkinsPage client={client({ list })} theme="light" />);
  await waitFor(() => expect(list).toHaveBeenCalledWith(0, "", false, null));
  const chips = screen.getByRole("group", { name: "键盘皮肤分类" });
  expect(
    within(chips)
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(["全部", "自然", "国风", "二次元", "可爱", "美食", "科技夜色", "简约", "其他"]);
  expect(within(chips).getByRole("button", { name: "全部" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  await screen.findByRole("button", { name: "查看皮肤 第一款" });

  fireEvent.click(within(chips).getByRole("button", { name: "国风" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", false, "guofeng"));
  expect(within(chips).getByRole("button", { name: "国风" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  await waitFor(() => expect(screen.queryByRole("button", { name: "查看皮肤 第一款" })).toBeNull());
  fireEvent.click(await screen.findByRole("button", { name: "加载更多" }));
  // 换分类后从 0 重新分页，「加载更多」接着新分类第一页的 offset。
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(1, "", false, "guofeng"));
  expect(await screen.findByRole("button", { name: "查看皮肤 第三款" })).not.toBeNull();

  fireEvent.click(within(chips).getByRole("button", { name: "全部" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", false, null));
});

test("a search keeps the selected category", async () => {
  const list = vi.fn().mockResolvedValue({ skins: [], has_more: false });
  render(<CommunitySkinsPage client={client({ list })} theme="light" />);
  await waitFor(() => expect(list).toHaveBeenCalledWith(0, "", false, null));
  const chips = screen.getByRole("group", { name: "键盘皮肤分类" });
  fireEvent.click(within(chips).getByRole("button", { name: "可爱" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", false, "cute"));
  fireEvent.change(screen.getByRole("textbox", { name: "搜索皮肤设计" }), {
    target: { value: "猫" },
  });
  fireEvent.click(screen.getByRole("button", { name: "搜索" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "猫", false, "cute"));
});

test("a failed category switch returns to the category of the displayed list", async () => {
  const first = skin("10000000-0000-4000-8000-000000000001", "第一款");
  const pending = deferred<CommunitySkinPage>();
  const list = vi
    .fn()
    .mockResolvedValueOnce({ skins: [first], has_more: true })
    .mockReturnValueOnce(pending.promise)
    .mockRejectedValueOnce({ code: "community_unavailable" })
    .mockResolvedValueOnce({ skins: [], has_more: false });
  render(<CommunitySkinsPage client={client({ list })} theme="light" />);
  await screen.findByRole("button", { name: "查看皮肤 第一款" });
  const chips = screen.getByRole("group", { name: "键盘皮肤分类" });
  fireEvent.click(within(chips).getByRole("button", { name: "自然" }));
  fireEvent.click(within(chips).getByRole("button", { name: "美食" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", false, "food"));
  await waitFor(() =>
    expect(within(chips).getByRole("button", { name: "全部" }).getAttribute("aria-pressed")).toBe(
      "true",
    ),
  );
  expect(within(chips).getByRole("button", { name: "美食" }).getAttribute("aria-pressed")).toBe(
    "false",
  );
  expect(screen.getByRole("button", { name: "查看皮肤 第一款" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(1, "", false, null));
  await act(async () => pending.resolve({ skins: [], has_more: false }));
});

test("going back to 全部 while another category is loading reads 全部 again", async () => {
  const first = skin("10000000-0000-4000-8000-000000000001", "第一款");
  const pending = deferred<CommunitySkinPage>();
  const list = vi
    .fn()
    .mockResolvedValueOnce({ skins: [first], has_more: false })
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValueOnce({ skins: [first], has_more: false });
  render(<CommunitySkinsPage client={client({ list })} theme="light" />);
  await screen.findByRole("button", { name: "查看皮肤 第一款" });
  const chips = screen.getByRole("group", { name: "键盘皮肤分类" });
  fireEvent.click(within(chips).getByRole("button", { name: "自然" }));
  fireEvent.click(within(chips).getByRole("button", { name: "全部" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", false, null));
  expect(list).toHaveBeenCalledTimes(3);
  await act(async () => pending.resolve({ skins: [], has_more: false }));
  expect(within(chips).getByRole("button", { name: "全部" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
});

test("cards and the detail view show the category label, and a skin without one shows none", async () => {
  const cute = {
    ...skin("10000000-0000-4000-8000-000000000001", "猫爪"),
    category: "cute" as const,
  };
  const plain = skin("10000000-0000-4000-8000-000000000002", "素色");
  render(
    <CommunitySkinsPage
      client={client({
        list: vi.fn().mockResolvedValue({ skins: [cute, plain], has_more: false }),
        detail: vi.fn().mockResolvedValue(cute),
      })}
      theme="light"
    />,
  );
  const card = await screen.findByRole("button", { name: "查看皮肤 猫爪" });
  expect(within(card).getByText("可爱 · 示例作者")).not.toBeNull();
  const other = screen.getByRole("button", { name: "查看皮肤 素色" });
  expect(within(other).getByText("示例作者")).not.toBeNull();
  fireEvent.click(card);
  await screen.findByRole("heading", { name: "猫爪" });
  expect(await screen.findByText("可爱 · 示例作者")).not.toBeNull();
  // 不是自己的作品时不能改分类。
  expect(screen.queryByRole("combobox", { name: "修改分类" })).toBeNull();
});

test("publish dialog: the category defaults to 其他 and the chosen one is sent", async () => {
  const local = { id: "30000000-0000-4000-8000-000000000001", name: "我的森林", design };
  const publish = vi.fn().mockResolvedValue(undefined);
  render(
    <CommunitySkinsPage
      client={client({ publish })}
      theme="light"
      localSkinLibrary={{ load: vi.fn().mockResolvedValue([local]), mutate: vi.fn() }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的设计" }));
  const select = (await screen.findByRole("combobox", { name: "发布分类" })) as HTMLSelectElement;
  expect(select.value).toBe("other");
  expect(Array.from(select.options).map((option) => option.textContent)).toEqual([
    "自然",
    "国风",
    "二次元",
    "可爱",
    "美食",
    "科技夜色",
    "简约",
    "其他",
  ]);
  fireEvent.change(select, { target: { value: "tech" } });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  await waitFor(() => expect(publish).toHaveBeenCalledOnce());
  expect(publish.mock.calls[0].slice(1)).toEqual(["我的森林", "", design, "tech"]);
});

test("owners change the category of their skin from the detail view", async () => {
  const mine = {
    ...skin("10000000-0000-4000-8000-000000000001", "我的皮肤"),
    owned: true,
    category: "nature" as const,
  };
  const communitySkins = client({
    list: vi.fn().mockResolvedValue({ skins: [mine], has_more: false }),
    detail: vi.fn().mockResolvedValue(mine),
  });
  render(<CommunitySkinsPage client={communitySkins} theme="light" />);
  fireEvent.click(await screen.findByRole("button", { name: "查看皮肤 我的皮肤" }));
  const select = (await screen.findByRole("combobox", { name: "修改分类" })) as HTMLSelectElement;
  expect(select.value).toBe("nature");
  fireEvent.change(select, { target: { value: "minimal" } });
  await waitFor(() => expect(communitySkins.setCategory).toHaveBeenCalledWith(mine.id, "minimal"));
  expect(await screen.findByText("已改为「简约」分类。")).not.toBeNull();
  expect((screen.getByRole("combobox", { name: "修改分类" }) as HTMLSelectElement).value).toBe(
    "minimal",
  );
  // 回到列表时卡片也换成新分类。
  fireEvent.click(screen.getByRole("button", { name: "返回社区" }));
  const card = await screen.findByRole("button", { name: "查看皮肤 我的皮肤" });
  expect(within(card).getByText("简约 · 我的作品")).not.toBeNull();
});
