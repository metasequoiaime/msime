// @vitest-environment jsdom
import { testHost } from "../support/host";
import { StrictMode } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  CommunityPage,
  CommunityPluginPublishDialog,
  CommunityPluginsPage,
  SettingsPage,
  type CommunityPlugin,
  type CommunityPluginClient,
  type PluginCatalogResult,
  type PluginPackage,
} from "@msime/ui";
import { createDesktopPluginCommunity } from "../../src/core/desktop-host-services";
import { settingsFormReady } from "../support/settings-form";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function plugin(id: string, name: string, overrides: Partial<CommunityPlugin> = {}) {
  return {
    id,
    kind: "sound",
    plugin_id: "rain",
    name,
    description: `${name} 的说明`,
    author: "示例作者",
    version: "1.0.0",
    license: "CC-BY-4.0",
    size: 2048,
    sha256: "0".repeat(64),
    downloads: 3,
    rating_count: 1,
    rating_average: 4,
    owned: false,
    my_rating: 0,
    created_at: "2026-09-30T00:00:00Z",
    ...overrides,
  } satisfies CommunityPlugin;
}

const first = plugin("20000000-0000-4000-8000-000000000001", "雨声");
const second = plugin("20000000-0000-4000-8000-000000000002", "雷雨", { plugin_id: "storm" });

function pack(kind: PluginPackage["kind"], id: string, builtin = false): PluginPackage {
  return {
    id,
    name: `本地 ${id}`,
    version: "1.0.0",
    license: "MIT",
    author: null,
    description: null,
    builtin,
    kind,
  };
}

function catalog(packages: PluginPackage[]): () => Promise<PluginCatalogResult> {
  return vi.fn().mockResolvedValue({ packages, issues: [] });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function client(overrides: Partial<CommunityPluginClient> = {}): CommunityPluginClient {
  return {
    list: vi.fn().mockResolvedValue({ plugins: [first], has_more: false }),
    detail: vi.fn().mockImplementation(async (id: string) => (id === first.id ? first : second)),
    packPreview: vi.fn().mockResolvedValue({
      suggestedName: "我的雨声",
      suggestedDescription: "下雨的声音",
      version: "1.0.0",
      license: "MIT",
      fileCount: 3,
      size: 1048576,
    }),
    publish: vi.fn().mockResolvedValue(plugin(first.id, "我的雨声", { owned: true })),
    install: vi.fn().mockResolvedValue(pack("sound", "rain")),
    rate: vi.fn().mockResolvedValue({ stars: 5 }),
    delete: vi.fn().mockResolvedValue({ deleted: true }),
    ...overrides,
  };
}

async function openDetail(name = "雨声") {
  fireEvent.click(await screen.findByRole("button", { name: `查看插件 ${name}` }));
  return screen.findByRole("heading", { name });
}

test("the kind filter is sent with every page, including the ones load more appends", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({ plugins: [first], has_more: false })
    .mockResolvedValueOnce({ plugins: [first], has_more: true })
    .mockResolvedValueOnce({ plugins: [second], has_more: false });
  render(<CommunityPluginsPage client={client({ list })} />);
  await waitFor(() => expect(list).toHaveBeenCalledWith(0, "", null, false));

  fireEvent.click(screen.getByRole("button", { name: "音效包" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", "sound", false));
  expect(screen.getByRole("button", { name: "音效包" }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.queryByRole("button", { name: "特效包" })).toBeNull();

  fireEvent.click(await screen.findByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(1, "", "sound", false));
  expect(await screen.findByRole("button", { name: "查看插件 雷雨" })).not.toBeNull();
});

test("an empty community without a search or filter says nothing is published yet", async () => {
  const list = vi.fn().mockResolvedValue({ plugins: [], has_more: false });
  render(<CommunityPluginsPage client={client({ list })} localPlugins={catalog([])} />);
  expect(await screen.findByText(/社区里还没有插件/)).not.toBeNull();
  expect(screen.getByText(/发布我的插件/, { selector: "p" })).not.toBeNull();
  expect(screen.queryByText(/没有匹配/)).toBeNull();
});

test("without a way to publish, the empty community does not point at the publish button", async () => {
  const list = vi.fn().mockResolvedValue({ plugins: [], has_more: false });
  render(<CommunityPluginsPage client={client({ list })} />);
  const notice = await screen.findByText(/社区里还没有插件/);
  expect(notice.textContent).not.toContain("发布我的插件");
});

test("an empty page under a kind filter or a search says nothing matched", async () => {
  const list = vi.fn().mockResolvedValue({ plugins: [], has_more: false });
  render(<CommunityPluginsPage client={client({ list })} />);
  await screen.findByText(/社区里还没有插件/);

  fireEvent.click(screen.getByRole("button", { name: "音乐包" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", "music", false));
  expect(await screen.findByText("没有匹配的插件。")).not.toBeNull();
  expect(screen.queryByText(/社区里还没有插件/)).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "全部" }));
  await screen.findByText(/社区里还没有插件/);
  fireEvent.change(screen.getByRole("textbox", { name: "搜索插件" }), {
    target: { value: "雨" },
  });
  fireEvent.click(screen.getByRole("button", { name: "搜索" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "雨", null, false));
  expect(await screen.findByText("没有匹配的插件。")).not.toBeNull();
});

test("a list that fails shows the error and no empty-list notice", async () => {
  const list = vi.fn().mockRejectedValue(new Error("offline"));
  render(<CommunityPluginsPage client={client({ list })} />);
  await waitFor(() => expect(list).toHaveBeenCalled());
  expect(await screen.findByRole("alert")).not.toBeNull();
  expect(screen.queryByText(/没有.*插件/)).toBeNull();
});

test("load more resumes after the items the host skipped on the previous page", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({ plugins: [first], has_more: true, skipped: 2 })
    .mockResolvedValueOnce({ plugins: [second], has_more: false, skipped: 0 });
  render(<CommunityPluginsPage client={client({ list })} />);
  fireEvent.click(await screen.findByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(3, "", null, false));
  expect(await screen.findByRole("button", { name: "查看插件 雷雨" })).not.toBeNull();
});

test("a first page the host emptied reads on to the installable plugins after it", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({ plugins: [], has_more: true, skipped: 20 })
    .mockResolvedValueOnce({ plugins: [first], has_more: false, skipped: 1 });
  render(<CommunityPluginsPage client={client({ list })} localPlugins={catalog([])} />);
  expect(await screen.findByRole("button", { name: "查看插件 雨声" })).not.toBeNull();
  expect(list).toHaveBeenNthCalledWith(1, 0, "", null, false);
  expect(list).toHaveBeenNthCalledWith(2, 20, "", null, false);
  expect(screen.queryByText(/社区里还没有插件/)).toBeNull();
  expect(screen.queryByRole("button", { name: "加载更多" })).toBeNull();
});

test("a run of emptied pages stops after a few and points at load more, not at an empty community", async () => {
  const list = vi.fn().mockResolvedValue({ plugins: [], has_more: true, skipped: 20 });
  render(<CommunityPluginsPage client={client({ list })} localPlugins={catalog([])} />);
  expect(await screen.findByText(/这台设备都不能安装/)).not.toBeNull();
  expect(screen.queryByText(/社区里还没有插件/)).toBeNull();
  const calls = list.mock.calls.length;
  expect(calls).toBeGreaterThan(1);
  expect(calls).toBeLessThanOrEqual(6);
  fireEvent.click(screen.getByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenCalledWith(calls * 20, "", null, false));
});

test("a kind filter whose first page fails is rolled back, so load more stays on the listed kind", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({ plugins: [first], has_more: true })
    .mockRejectedValueOnce(new Error("offline"))
    .mockResolvedValueOnce({ plugins: [second], has_more: false });
  render(<CommunityPluginsPage client={client({ list })} />);
  await waitFor(() => expect(list).toHaveBeenCalledWith(0, "", null, false));

  fireEvent.click(await screen.findByRole("button", { name: "音乐包" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", "music", false));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "音乐包" }).getAttribute("aria-pressed")).toBe(
      "false",
    ),
  );

  fireEvent.click(await screen.findByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(1, "", null, false));
});

test("installing over a pack of the same kind and id asks first, then reports the install", async () => {
  const communityClient = client();
  const onInstalled = vi.fn();
  render(
    <CommunityPluginsPage
      client={communityClient}
      localPlugins={catalog([pack("sound", "rain"), pack("music", "storm")])}
      onInstalled={onInstalled}
    />,
  );
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  const confirm = await screen.findByRole("alertdialog", { name: "确认替换插件" });
  expect(communityClient.install).not.toHaveBeenCalled();

  fireEvent.click(within(confirm).getByRole("button", { name: "替换安装" }));
  await waitFor(() =>
    expect(communityClient.install).toHaveBeenCalledWith(first.id, "sound", "rain"),
  );
  expect(await screen.findByText("已安装到插件目录，可在「我的插件」中选用。")).not.toBeNull();
  expect(onInstalled).toHaveBeenCalledWith(pack("sound", "rain"));
});

test("a pack of another kind with the same id installs without asking", async () => {
  const communityClient = client();
  render(
    <CommunityPluginsPage
      client={communityClient}
      localPlugins={catalog([pack("music", "rain")])}
    />,
  );
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  await waitFor(() =>
    expect(communityClient.install).toHaveBeenCalledWith(first.id, "sound", "rain"),
  );
  expect(screen.queryByRole("alertdialog", { name: "确认替换插件" })).toBeNull();
});

test("a download that fails its checksum is named", async () => {
  const install = vi.fn().mockRejectedValue({ code: "plugin_community_checksum" });
  render(<CommunityPluginsPage client={client({ install })} />);
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  expect((await screen.findByRole("alert")).textContent).toContain("下载的插件已损坏");
});

test("rates and takes down through the gallery", async () => {
  const owned = plugin(second.id, "雷雨", { owned: true });
  const communityClient = client({
    list: vi.fn().mockResolvedValue({ plugins: [first, owned], has_more: false }),
    detail: vi.fn().mockImplementation(async (id: string) => (id === first.id ? first : owned)),
  });
  render(<CommunityPluginsPage client={communityClient} />);
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "评 5 星" }));
  await waitFor(() => expect(communityClient.rate).toHaveBeenCalledWith(first.id, 5));
  expect(await screen.findByText("已评分：5 星。")).not.toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "返回社区" }));
  await openDetail("雷雨");
  fireEvent.click(screen.getByRole("button", { name: "下架这个插件" }));
  const confirm = await screen.findByRole("alertdialog", { name: "确认下架插件" });
  fireEvent.click(within(confirm).getByRole("button", { name: "确认下架" }));
  await waitFor(() => expect(communityClient.delete).toHaveBeenCalledWith(owned.id));
});

test("publishing offers only installed, shareable packs and retries under the same id", async () => {
  const publish = vi
    .fn()
    .mockRejectedValueOnce({ code: "community_unavailable" })
    .mockResolvedValueOnce(plugin(first.id, "我的雨声", { owned: true }));
  const communityClient = client({ publish });
  render(
    <CommunityPluginsPage
      client={communityClient}
      localPlugins={catalog([
        pack("sound", "default", true),
        pack("effect", "sparkle"),
        pack("command_table", "dates"),
        pack("sound", "rain"),
      ])}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的插件" }));
  const dialog = await screen.findByRole("dialog", { name: "发布插件" });
  const select = await within(dialog).findByRole("combobox", { name: "发布插件" });
  const options = within(select)
    .getAllByRole("option")
    .map((option) => option.getAttribute("value"));
  expect(options).toEqual(["command_table/dates", "sound/rain"]);

  fireEvent.change(select, { target: { value: "sound/rain" } });
  await waitFor(() =>
    expect(communityClient.packPreview).toHaveBeenLastCalledWith("sound", "rain"),
  );
  const name = (await within(dialog).findByRole("textbox", {
    name: "发布插件名称",
  })) as HTMLInputElement;
  await waitFor(() => expect(name.value).toBe("我的雨声"));
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "公开发布" }));
  expect((await within(dialog).findByRole("alert")).textContent).toContain("社区暂时不可用");

  fireEvent.click(within(dialog).getByRole("button", { name: "公开发布" }));
  await waitFor(() => expect(publish).toHaveBeenCalledTimes(2));
  const [firstCall, retry] = publish.mock.calls;
  expect(firstCall).toEqual(["sound", "rain", firstCall[2], "我的雨声", "下雨的声音"]);
  expect(retry[2]).toBe(firstCall[2]);
  expect(await screen.findByText("已发布到社区。")).not.toBeNull();
});

test.each(["success", "failure"])(
  "late catalog %s cannot replace the new publish reader under StrictMode",
  async (outcome) => {
    let resolveOld!: (value: PluginCatalogResult) => void;
    let rejectOld!: (reason: unknown) => void;
    const oldCatalog = new Promise<PluginCatalogResult>((resolve, reject) => {
      resolveOld = resolve;
      rejectOld = reject;
    });
    const nextCatalog = deferred<PluginCatalogResult>();
    const communityClient = client();
    const onClose = vi.fn();
    const onPublished = vi.fn();
    const view = (localPlugins: () => Promise<PluginCatalogResult>) => (
      <StrictMode>
        <CommunityPluginPublishDialog
          client={communityClient}
          localPlugins={localPlugins}
          onClose={onClose}
          onPublished={onPublished}
        />
      </StrictMode>
    );
    const { rerender } = render(view(() => oldCatalog));
    rerender(view(() => nextCatalog.promise));

    await act(async () => {
      if (outcome === "success") resolveOld({ packages: [pack("sound", "old")], issues: [] });
      else rejectOld(new Error("synthetic catalog failure"));
    });
    expect(screen.getByText("正在读取本地插件…")).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "发布插件" })).toBeNull();
    expect(screen.queryByText("读取本地插件失败，请重试。")).toBeNull();

    await act(async () => {
      nextCatalog.resolve({ packages: [pack("sound", "replacement")], issues: [] });
    });
    const select = await screen.findByRole("combobox", { name: "发布插件" });
    expect(
      within(select)
        .getAllByRole("option")
        .map((option) => option.getAttribute("value")),
    ).toEqual(["sound/replacement"]);
    expect(screen.queryByText("正在读取本地插件…")).toBeNull();
    expect(screen.queryByText("读取本地插件失败，请重试。")).toBeNull();
  },
);

test("a successful publish releases the dialog busy state after the callback", async () => {
  const onPublished = vi.fn().mockResolvedValue(undefined);
  render(
    <CommunityPluginPublishDialog
      client={client()}
      localPlugins={catalog([pack("sound", "rain")])}
      onClose={vi.fn()}
      onPublished={onPublished}
    />,
  );
  const dialog = await screen.findByRole("dialog", { name: "发布插件" });
  await within(dialog).findByRole("textbox", { name: "发布插件名称" });
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  const submit = within(dialog).getByRole("button", { name: "公开发布" }) as HTMLButtonElement;
  fireEvent.click(submit);
  await waitFor(() => expect(onPublished).toHaveBeenCalledOnce());
  await waitFor(() => expect(submit.disabled).toBe(false));
});

test("publish dialog ignores a same-tick duplicate submission", async () => {
  const pending = deferred<CommunityPlugin>();
  const publish = vi.fn().mockReturnValue(pending.promise);
  render(
    <CommunityPluginPublishDialog
      client={client({ publish })}
      localPlugins={catalog([pack("sound", "rain")])}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  const dialog = await screen.findByRole("dialog", { name: "发布插件" });
  await within(dialog).findByRole("textbox", { name: "发布插件名称" });
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "确认拥有发布内容权利" }));

  await act(async () => {
    fireEvent.click(within(dialog).getByRole("button", { name: "公开发布" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "公开发布" }));
  });
  expect(publish).toHaveBeenCalledOnce();
  pending.resolve(first);
  await waitFor(() => expect(publish).toHaveBeenCalledOnce());
});

test("replacing the publish client releases a pending dialog action", async () => {
  const pending = deferred<CommunityPlugin>();
  const publish = vi.fn().mockReturnValue(pending.promise);
  const initial = client({ publish });
  const replacement = client();
  const view = render(
    <CommunityPluginPublishDialog
      client={initial}
      localPlugins={catalog([pack("sound", "rain")])}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  const dialog = await screen.findByRole("dialog", { name: "发布插件" });
  await within(dialog).findByRole("textbox", { name: "发布插件名称" });
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  const submit = within(dialog).getByRole("button", { name: "公开发布" }) as HTMLButtonElement;
  fireEvent.click(submit);
  await waitFor(() => expect(submit.disabled).toBe(true));

  view.rerender(
    <CommunityPluginPublishDialog
      client={replacement}
      localPlugins={catalog([pack("sound", "rain")])}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  await waitFor(() => {
    const currentDialog = screen.getByRole("dialog", { name: "发布插件" });
    expect(
      (
        within(currentDialog).getByRole("checkbox", {
          name: "确认拥有发布内容权利",
        }) as HTMLInputElement
      ).disabled,
    ).toBe(false);
  });
  pending.resolve(first);
});

test("editing the name after a failed publish draws a new publication id", async () => {
  const publish = vi.fn().mockRejectedValue({ code: "community_unavailable" });
  render(
    <CommunityPluginsPage
      client={client({ publish })}
      localPlugins={catalog([pack("sound", "rain")])}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的插件" }));
  const dialog = await screen.findByRole("dialog", { name: "发布插件" });
  const name = (await within(dialog).findByRole("textbox", {
    name: "发布插件名称",
  })) as HTMLInputElement;
  await waitFor(() => expect(name.value).toBe("我的雨声"));
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "公开发布" }));
  await within(dialog).findByRole("alert");
  fireEvent.change(name, { target: { value: "新的雨声" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "公开发布" }));
  await waitFor(() => expect(publish).toHaveBeenCalledTimes(2));
  expect(publish.mock.calls[1][3]).toBe("新的雨声");
  expect(publish.mock.calls[1][2]).not.toBe(publish.mock.calls[0][2]);
});

test("a pack the packer refuses says why before the form is shown", async () => {
  const packPreview = vi.fn().mockRejectedValue({ code: "plugin_community_too_large" });
  render(
    <CommunityPluginsPage
      client={client({ packPreview })}
      localPlugins={catalog([pack("music", "lofi")])}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的插件" }));
  const dialog = await screen.findByRole("dialog", { name: "发布插件" });
  expect((await within(dialog).findByRole("alert")).textContent).toContain("8 MB");
  expect(within(dialog).queryByRole("button", { name: "公开发布" })).toBeNull();
});

test("the community page no longer carries a plugin tab", async () => {
  render(
    <CommunityPage
      theme="dark"
      candidateSkins={
        {
          list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
          sync: vi.fn(),
        } as never
      }
    />,
  );
  await waitFor(() => expect(screen.queryByRole("tablist", { name: "社区分类" })).toBeNull());
  expect(screen.queryByRole("tab", { name: "插件" })).toBeNull();
});

test("the 插件 page switches between the installed packs and the community gallery", async () => {
  const pluginClient = client();
  const installed = catalog([pack("sound", "rain")]);
  render(
    <SettingsPage
      client={{
        load: async () => ({
          format_version: 1,
          revision: 1,
          preferences: {
            scheme: "quanpin",
            shuangpin_profile: "xiaohe",
            candidate_page_size: 5,
            learning: true,
            chinese_punctuation: true,
          },
        }),
        save: vi.fn(),
        loadDefaultPreferences: vi.fn(),
        host: testHost({ platform: "macos" }),
        plugins: {
          catalog: installed,
          importPack: vi.fn(async () => null),
          remove: vi.fn(async () => undefined),
          loadMentions: vi.fn(async () => []),
          saveMentions: vi.fn(async () => undefined),
        },
        communityPlugins: pluginClient,
      }}
    />,
  );
  const form = await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "插件" }));
  const tabs = screen.getByRole("tablist", { name: "插件来源" });
  expect(within(tabs).getByRole("tab", { name: "我的插件" }).getAttribute("aria-selected")).toBe(
    "true",
  );
  expect(await screen.findByLabelText("已安装的插件")).not.toBeNull();
  expect(screen.getByRole("button", { name: "恢复默认设置" })).not.toBeNull();
  expect(pluginClient.list).not.toHaveBeenCalled();
  // A pack's detail left open while visiting the gallery is closed on the way back.
  fireEvent.click(await screen.findByRole("button", { name: "本地 rain" }));
  expect(screen.getByRole("heading", { name: "本地 rain" })).not.toBeNull();

  fireEvent.click(within(tabs).getByRole("tab", { name: "社区插件" }));
  await waitFor(() => expect(pluginClient.list).toHaveBeenCalledWith(0, "", null, false));
  const gallery = await screen.findByRole("heading", { name: "社区插件" });
  // The gallery has its own search form, so it must sit outside the settings form rather than nest inside it.
  expect(form.contains(gallery)).toBe(false);
  const installedPage = form.querySelector<HTMLFieldSetElement>('fieldset[aria-label="插件"]')!;
  expect(installedPage.hidden).toBe(true);
  // The gallery stands in for the page's own settings, so there is nothing for 恢复默认设置 to restore.
  expect(screen.queryByRole("button", { name: "恢复默认设置" })).toBeNull();

  const reads = vi.mocked(installed).mock.calls.length;
  fireEvent.click(within(tabs).getByRole("tab", { name: "我的插件" }));
  expect(screen.queryByRole("heading", { name: "社区插件" })).toBeNull();
  expect(installedPage.hidden).toBe(false);
  // Coming back re-reads the directory, so a pack installed from the gallery shows up.
  await waitFor(() => expect(vi.mocked(installed).mock.calls.length).toBeGreaterThan(reads));
  expect(screen.queryByRole("heading", { name: "本地 rain" })).toBeNull();
  expect(await screen.findByRole("button", { name: "本地 rain" })).not.toBeNull();
});

test("the desktop bridge names each command and its camelCase arguments", async () => {
  const invoke = vi.fn().mockResolvedValue({});
  const bridge = createDesktopPluginCommunity(invoke);
  await bridge.list(20, "雨", "music");
  await bridge.packPreview("sound", "rain");
  await bridge.publish("sound", "rain", first.id, "雨声", "");
  await bridge.install(first.id, "sound", "rain");
  await bridge.rate(first.id, 4);
  await bridge.delete(first.id);
  expect(invoke.mock.calls).toEqual([
    ["plugin_community_list", { offset: 20, search: "雨", kind: "music" }],
    ["plugin_community_pack_preview", { kind: "sound", pluginId: "rain" }],
    [
      "plugin_community_publish",
      { kind: "sound", pluginId: "rain", id: first.id, name: "雨声", description: "" },
    ],
    ["plugin_community_install", { id: first.id, kind: "sound", pluginId: "rain" }],
    ["plugin_community_rate", { id: first.id, stars: 4 }],
    ["plugin_community_delete", { id: first.id }],
  ]);
});
