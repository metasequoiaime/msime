// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  CandidateSkinPublishDialog,
  CommunityCandidateSkinsPage,
  CommunityPage,
  type CandidateSkinCommunityClient,
  type CandidateSkinSyncReport,
  type CommunityCandidateSkin,
  type CommunityCandidateSkinPage,
  type SkinCatalog,
} from "@msime/ui";

const renderSkinPreview = vi.hoisted(() => vi.fn());
vi.mock("../../../../packages/ui/src/skin/skin-preview-render", () => ({
  renderSkinPreview,
}));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function skin(id: string, name: string, overrides: Partial<CommunityCandidateSkin> = {}) {
  return {
    id,
    package_id: "ink-wash",
    name,
    description: `${name} 的说明`,
    author: "示例作者",
    version: "1.0.0",
    license: { code: "MIT", assets: "CC-BY-4.0", source: "" },
    size: 1024,
    file_count: 1,
    downloads: 7,
    rating_count: 2,
    rating_average: 4.5,
    owned: false,
    my_rating: 0,
    created_at: "2026-09-30T00:00:00Z",
    visibility: "public",
    updated_at: "2026-09-30T00:00:00Z",
    ...overrides,
  } satisfies CommunityCandidateSkin;
}

const first = skin("10000000-0000-4000-8000-000000000001", "水墨");
const second = skin("10000000-0000-4000-8000-000000000002", "青绿");
const third = skin("10000000-0000-4000-8000-000000000003", "朱砂");

function catalog(ids: string[]): SkinCatalog {
  return {
    directory: "/synthetic/skins",
    issues: [],
    packages: ids.map((id) => ({
      id,
      name: `本地 ${id}`,
      version: "1",
      base: "system",
      author: null,
      description: null,
      layouts: ["horizontal", "vertical"],
      themes: ["dark", "light"],
      minWidthDip: 0,
      decorationTopDip: 0,
      decorationWidthDip: 0,
      toolbarStylesheet: null,
      preview: "preview.png",
      candidate: { dark: {}, light: {} },
    })),
  };
}

function report(overrides: Partial<CandidateSkinSyncReport> = {}): CandidateSkinSyncReport {
  return {
    uploaded: [],
    downloaded: [],
    deleted_local: [],
    deleted_cloud: [],
    skipped: [],
    stopped: null,
    ...overrides,
  };
}

function client(
  overrides: Partial<CandidateSkinCommunityClient> = {},
): CandidateSkinCommunityClient {
  return {
    list: vi.fn().mockResolvedValue({ skins: [first], has_more: false }),
    detail: vi.fn().mockImplementation(async (id: string) => (id === first.id ? first : second)),
    preview: vi.fn().mockResolvedValue({ dataUrl: "data:image/png;base64,iVBORw0KGgo=" }),
    install: vi.fn().mockResolvedValue(catalog(["ink-wash"])),
    packPreview: vi.fn().mockResolvedValue({
      suggestedName: "水墨",
      license: { code: "MIT", assets: "CC-BY-4.0", source: null },
      fileCount: 2,
      size: 1572864,
    }),
    addPreview: vi.fn().mockResolvedValue(catalog(["ink-wash"])),
    addLicense: vi.fn().mockResolvedValue(catalog(["ink-wash"])),
    publish: vi.fn().mockResolvedValue(first),
    rate: vi.fn().mockResolvedValue({ stars: 4 }),
    unpublish: vi.fn().mockResolvedValue({ deleted: true }),
    setVisibility: vi.fn().mockImplementation(async (id: string, visibility) => ({
      ...(id === first.id ? first : second),
      owned: true,
      visibility,
    })),
    setCategory: vi.fn().mockImplementation(async (id: string, category) => ({
      ...(id === first.id ? first : second),
      owned: true,
      category,
    })),
    sync: vi.fn().mockResolvedValue(report()),
    ...overrides,
  };
}

/** Let a settled request's continuation run and React commit what it scheduled, so an assertion that something did not render is not simply early. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

async function openDetail(name = "水墨") {
  fireEvent.click(await screen.findByRole("button", { name: `查看候选窗口皮肤 ${name}` }));
  return screen.findByRole("heading", { name });
}

test("lists with the exact offset, search and scope", async () => {
  const communityClient = client();
  render(<CommunityCandidateSkinsPage client={communityClient} />);
  await waitFor(() => expect(communityClient.list).toHaveBeenCalledWith(0, "", false, null));
  expect(screen.getByRole("heading", { name: "社区皮肤" })).not.toBeNull();
  expect(screen.getByText("为输入候选窗口换一身新装，安装后在「我的皮肤」中启用")).not.toBeNull();

  fireEvent.change(screen.getByRole("textbox", { name: "搜索候选窗口皮肤" }), {
    target: { value: " 水墨 " },
  });
  fireEvent.click(screen.getByRole("button", { name: "搜索" }));
  await waitFor(() =>
    expect(communityClient.list).toHaveBeenLastCalledWith(0, " 水墨 ", false, null),
  );

  fireEvent.click(screen.getByRole("button", { name: "我的作品" }));
  await waitFor(() =>
    expect(communityClient.list).toHaveBeenLastCalledWith(0, " 水墨 ", true, null),
  );
  expect(screen.getByRole("button", { name: "我的作品" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  fireEvent.click(
    within(screen.getByRole("group", { name: "候选窗口皮肤范围" })).getByRole("button", {
      name: "全部",
    }),
  );
  await waitFor(() =>
    expect(communityClient.list).toHaveBeenLastCalledWith(0, " 水墨 ", false, null),
  );
});

test("load more advances the offset and drops duplicate ids", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({ skins: [first, second], has_more: true })
    .mockResolvedValueOnce({ skins: [second, third], has_more: false });
  render(<CommunityCandidateSkinsPage client={client({ list })} />);
  fireEvent.click(await screen.findByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(2, "", false, null));
  expect(await screen.findByRole("button", { name: "查看候选窗口皮肤 朱砂" })).not.toBeNull();
  expect(screen.getAllByRole("button", { name: /查看候选窗口皮肤/ })).toHaveLength(3);
});

test("an older response cannot replace a newer search", async () => {
  const older = deferred<CommunityCandidateSkinPage>();
  const list = vi
    .fn()
    .mockReturnValueOnce(older.promise)
    .mockResolvedValueOnce({ skins: [second], has_more: false });
  render(<CommunityCandidateSkinsPage client={client({ list })} />);
  await waitFor(() => expect(list).toHaveBeenCalledTimes(1));
  fireEvent.change(screen.getByRole("textbox", { name: "搜索候选窗口皮肤" }), {
    target: { value: "青" },
  });
  fireEvent.click(screen.getByRole("button", { name: "搜索" }));
  expect(await screen.findByRole("button", { name: "查看候选窗口皮肤 青绿" })).not.toBeNull();
  older.resolve({ skins: [first], has_more: false });
  await settle();
  expect(screen.getByRole("button", { name: "查看候选窗口皮肤 青绿" })).not.toBeNull();
  expect(screen.queryByRole("button", { name: "查看候选窗口皮肤 水墨" })).toBeNull();
});

test("a replaced client cannot deliver the previous client's page", async () => {
  const older = deferred<CommunityCandidateSkinPage>();
  const oldClient = client({ list: vi.fn().mockReturnValue(older.promise) });
  const newClient = client({
    list: vi.fn().mockResolvedValue({ skins: [second], has_more: false }),
  });
  const view = render(<CommunityCandidateSkinsPage client={oldClient} />);
  view.rerender(<CommunityCandidateSkinsPage client={newClient} />);
  expect(await screen.findByRole("button", { name: "查看候选窗口皮肤 青绿" })).not.toBeNull();
  older.resolve({ skins: [first], has_more: false });
  await settle();
  expect(screen.getByRole("button", { name: "查看候选窗口皮肤 青绿" })).not.toBeNull();
  expect(screen.queryByRole("button", { name: "查看候选窗口皮肤 水墨" })).toBeNull();
});

test("a failed scope switch keeps the list and the toggle on the scope still shown", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({ skins: [first, second], has_more: true })
    .mockRejectedValueOnce({ code: "community_unauthorized" })
    .mockResolvedValueOnce({ skins: [third], has_more: false });
  render(<CommunityCandidateSkinsPage client={client({ list })} onLogin={vi.fn()} />);
  expect(await screen.findByRole("button", { name: "查看候选窗口皮肤 青绿" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "我的作品" }));
  expect(await screen.findByRole("button", { name: "去登录" })).not.toBeNull();
  await settle();
  expect(list).toHaveBeenLastCalledWith(0, "", true, null);
  expect(screen.getByRole("button", { name: "我的作品" }).getAttribute("aria-pressed")).toBe(
    "false",
  );
  expect(
    within(screen.getByRole("group", { name: "候选窗口皮肤范围" }))
      .getByRole("button", { name: "全部" })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(2, "", false, null));
  expect(await screen.findByRole("button", { name: "查看候选窗口皮肤 朱砂" })).not.toBeNull();
});

test("cards load their preview lazily and the detail reuses it", async () => {
  const communityClient = client();
  render(<CommunityCandidateSkinsPage client={communityClient} />);
  const image = await screen.findByRole("img", { name: "水墨 预览" });
  expect(image.getAttribute("src")).toBe("data:image/png;base64,iVBORw0KGgo=");
  expect(communityClient.preview).toHaveBeenCalledWith(first.id);
  await openDetail();
  expect(await screen.findByRole("img", { name: "水墨 预览" })).not.toBeNull();
  expect(communityClient.preview).toHaveBeenCalledTimes(1);
  expect(screen.getByText("素材授权 CC-BY-4.0 / 代码授权 MIT")).not.toBeNull();
  expect(screen.getByText(/7 人下载 · 4.5 分 · 2 人评分/)).not.toBeNull();
});

test("一键安装 installs, reports it, and 去启用 opens the theme page", async () => {
  const communityClient = client();
  const onOpenSkinPage = vi.fn();
  render(
    <CommunityCandidateSkinsPage
      client={communityClient}
      localSkins={vi.fn().mockResolvedValue(catalog(["other"]))}
      onOpenSkinPage={onOpenSkinPage}
    />,
  );
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  expect(await screen.findByText("已安装到外部皮肤。")).not.toBeNull();
  expect(communityClient.install).toHaveBeenCalledWith(first.id, false);
  fireEvent.click(screen.getByRole("button", { name: "去启用" }));
  expect(onOpenSkinPage).toHaveBeenCalledOnce();
});

test("an install from the community page leaves the directory row to 主题", async () => {
  render(
    <CommunityPage
      theme="light"
      candidateSkins={client()}
      localSkins={vi.fn().mockResolvedValue(catalog(["other"]))}
      openSkinDirectory={vi.fn().mockResolvedValue(undefined)}
    />,
  );
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  expect(await screen.findByText("已安装到外部皮肤。")).not.toBeNull();
  expect(screen.queryByText("/synthetic/skins")).toBeNull();
  expect(screen.queryByRole("button", { name: "打开目录" })).toBeNull();
  expect(screen.queryByRole("button", { name: "导入皮肤" })).toBeNull();
});

test("an installed package id is confirmed before any download", async () => {
  const communityClient = client();
  render(
    <CommunityCandidateSkinsPage
      client={communityClient}
      localSkins={vi.fn().mockResolvedValue(catalog(["ink-wash"]))}
    />,
  );
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  const confirm = await screen.findByRole("alertdialog", {
    name: "确认替换皮肤",
  });
  expect(confirm.textContent).toContain("已存在同名皮肤“ink-wash”，安装会整体替换它。");
  expect(communityClient.install).not.toHaveBeenCalled();
  fireEvent.click(within(confirm).getByRole("button", { name: "替换安装" }));
  await waitFor(() => expect(communityClient.install).toHaveBeenCalledWith(first.id, true));
  expect(await screen.findByText("已安装到外部皮肤。")).not.toBeNull();
});

test("a folder that appears during install falls back to the same confirmation", async () => {
  const install = vi
    .fn()
    .mockRejectedValueOnce({ code: "candidate_skin_exists" })
    .mockResolvedValueOnce(catalog(["ink-wash"]));
  const communityClient = client({ install });
  render(
    <CommunityCandidateSkinsPage
      client={communityClient}
      localSkins={vi.fn().mockResolvedValue(catalog([]))}
    />,
  );
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  const confirm = await screen.findByRole("alertdialog", {
    name: "确认替换皮肤",
  });
  expect(install).toHaveBeenCalledWith(first.id, false);
  fireEvent.click(within(confirm).getByRole("button", { name: "替换安装" }));
  await waitFor(() => expect(install).toHaveBeenLastCalledWith(first.id, true));
  expect(await screen.findByText("已安装到外部皮肤。")).not.toBeNull();
});

test("failures show fixed sentences only, never backend text", async () => {
  const communityClient = client({
    install: vi.fn().mockRejectedValue({
      code: "candidate_skin_image_invalid",
      message: "backend says <b>boom</b>",
    }),
  });
  render(<CommunityCandidateSkinsPage client={communityClient} />);
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toBe("图片已损坏或格式与扩展名不符。");
  expect(document.body.textContent).not.toContain("boom");
});

test("去登录 appears only for community_unauthorized", async () => {
  const onLogin = vi.fn();
  const install = vi
    .fn()
    .mockRejectedValueOnce({ code: "community_forbidden" })
    .mockRejectedValueOnce({ code: "community_unauthorized" });
  render(<CommunityCandidateSkinsPage client={client({ install })} onLogin={onLogin} />);
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  expect((await screen.findByRole("alert")).textContent).toBe(
    "下载后才能评分，且不能给自己的作品评分。",
  );
  expect(screen.queryByRole("button", { name: "去登录" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  fireEvent.click(await screen.findByRole("button", { name: "去登录" }));
  expect(onLogin).toHaveBeenCalledOnce();
});

test("non-owners rate and owners unpublish after a confirmation", async () => {
  const rated = { ...first, my_rating: 4 };
  const detail = vi.fn().mockResolvedValueOnce(first).mockResolvedValueOnce(rated);
  const communityClient = client({ detail });
  const view = render(<CommunityCandidateSkinsPage client={communityClient} />);
  await openDetail();
  expect(screen.queryByRole("button", { name: "下架这款皮肤" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "评 4 星" }));
  await waitFor(() => expect(communityClient.rate).toHaveBeenCalledWith(first.id, 4));
  expect(await screen.findByText("我的评分：4 星")).not.toBeNull();
  view.unmount();

  const owned = { ...first, owned: true };
  const ownerClient = client({
    list: vi
      .fn()
      .mockResolvedValueOnce({ skins: [owned], has_more: false })
      .mockResolvedValueOnce({ skins: [], has_more: false }),
    detail: vi.fn().mockResolvedValue(owned),
  });
  render(<CommunityCandidateSkinsPage client={ownerClient} />);
  await openDetail();
  expect(screen.queryByRole("button", { name: "评 4 星" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "下架这款皮肤" }));
  const confirm = screen.getByRole("alertdialog", { name: "确认下架皮肤" });
  expect(ownerClient.unpublish).not.toHaveBeenCalled();
  fireEvent.click(within(confirm).getByRole("button", { name: "确认下架" }));
  await waitFor(() => expect(ownerClient.unpublish).toHaveBeenCalledWith(first.id));
  expect(await screen.findByText(/已下架这款皮肤/)).not.toBeNull();
  await waitFor(() => expect(ownerClient.list).toHaveBeenCalledTimes(2));
});

test("publish dialog: a missing license is chosen in the dialog and written for the author", async () => {
  const openSkinDirectory = vi.fn().mockResolvedValue(undefined);
  const packPreview = vi
    .fn()
    .mockRejectedValueOnce({ code: "candidate_skin_license_required" })
    .mockResolvedValue({
      suggestedName: "水墨",
      license: { code: null, assets: "CC-BY-4.0", source: null },
      fileCount: 2,
      size: 2048,
    });
  const communityClient = client({ packPreview });
  render(
    <CandidateSkinPublishDialog
      client={communityClient}
      initialSkinId="ink-wash"
      openSkinDirectory={openSkinDirectory}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  expect(await screen.findByText(/公开发布需要注明别人可以怎样使用/)).not.toBeNull();
  expect(packPreview).toHaveBeenCalledWith("ink-wash", "public");
  expect(screen.queryByRole("textbox", { name: "发布皮肤名称" })).toBeNull();
  expect(screen.queryByRole("button", { name: "公开发布" })).toBeNull();
  expect(screen.queryByRole("button", { name: "打开目录" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "使用此授权并继续" }));
  await waitFor(() =>
    expect(communityClient.addLicense).toHaveBeenCalledWith("ink-wash", "CC-BY-4.0"),
  );
  expect(await screen.findByRole("textbox", { name: "发布皮肤名称" })).not.toBeNull();
  expect(screen.getByLabelText("皮肤授权").textContent).toBe("素材授权 CC-BY-4.0");
  expect(packPreview).toHaveBeenCalledTimes(2);
});

test("publish dialog: a license of the author's own is trimmed, bounded, and a failed write says so", async () => {
  const communityClient = client({
    packPreview: vi.fn().mockRejectedValue({ code: "candidate_skin_license_required" }),
    addLicense: vi.fn().mockRejectedValue({ code: "candidate_skin_package_invalid" }),
  });
  render(
    <CandidateSkinPublishDialog
      client={communityClient}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  fireEvent.click(await screen.findByRole("radio", { name: "其他" }));
  const write = screen.getByRole("button", {
    name: "使用此授权并继续",
  }) as HTMLButtonElement;
  expect(write.disabled).toBe(true);
  const custom = screen.getByRole("textbox", { name: "其他素材授权" });
  fireEvent.change(custom, { target: { value: "猫".repeat(41) } });
  expect(screen.getByText(/授权说明太长/)).not.toBeNull();
  expect(write.disabled).toBe(true);
  fireEvent.change(custom, { target: { value: "  仅限个人使用  " } });
  expect(write.disabled).toBe(false);
  fireEvent.click(write);
  await waitFor(() =>
    expect(communityClient.addLicense).toHaveBeenCalledWith("ink-wash", "仅限个人使用"),
  );
  expect(await screen.findByText(/写入授权失败/)).not.toBeNull();
  expect(screen.queryByRole("textbox", { name: "发布皮肤名称" })).toBeNull();
});

test("license writing ignores a same-tick duplicate submission", async () => {
  const pending = deferred<SkinCatalog>();
  const addLicense = vi.fn().mockReturnValue(pending.promise);
  const communityClient = client({
    packPreview: vi.fn().mockRejectedValue({ code: "candidate_skin_license_required" }),
    addLicense,
  });
  render(
    <CandidateSkinPublishDialog
      client={communityClient}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  const write = await screen.findByRole("button", { name: "使用此授权并继续" });
  act(() => {
    fireEvent.click(write);
    fireEvent.click(write);
  });
  expect(addLicense).toHaveBeenCalledOnce();
  pending.resolve(catalog(["ink-wash"]));
  await settle();
});

test("publish dialog: a package without a preview can have one drawn and saved", async () => {
  const missing = catalog(["ink-wash"]);
  missing.packages[0].preview = null;
  const packPreview = vi
    .fn()
    .mockRejectedValueOnce({ code: "candidate_skin_preview_required" })
    .mockResolvedValue({
      suggestedName: "水墨",
      license: { code: "MIT", assets: "CC-BY-4.0", source: null },
      fileCount: 1,
      size: 2048,
    });
  const communityClient = client({ packPreview });
  const readImage = vi.fn();
  renderSkinPreview.mockResolvedValueOnce([137, 80, 78, 71]);
  render(
    <CandidateSkinPublishDialog
      client={communityClient}
      localSkins={vi.fn().mockResolvedValue(missing)}
      initialSkinId="ink-wash"
      openSkinDirectory={vi.fn()}
      readImage={readImage}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  expect(await screen.findByText(/这款皮肤还没有预览图/)).not.toBeNull();
  expect(screen.queryByText(/skin\.toml/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "生成预览图" }));
  await waitFor(() =>
    expect(communityClient.addPreview).toHaveBeenCalledWith("ink-wash", [137, 80, 78, 71]),
  );
  expect(renderSkinPreview).toHaveBeenCalledWith(missing.packages[0], readImage);
  expect(await screen.findByRole("textbox", { name: "发布皮肤名称" })).not.toBeNull();
  expect(packPreview).toHaveBeenCalledTimes(2);
});

test("publish dialog: a preview that cannot be drawn says so and keeps the button", async () => {
  const missing = catalog(["ink-wash"]);
  missing.packages[0].preview = null;
  const communityClient = client({
    packPreview: vi.fn().mockRejectedValue({ code: "candidate_skin_preview_required" }),
  });
  renderSkinPreview.mockRejectedValueOnce(new Error("canvas unavailable"));
  render(
    <CandidateSkinPublishDialog
      client={communityClient}
      localSkins={vi.fn().mockResolvedValue(missing)}
      initialSkinId="ink-wash"
      readImage={vi.fn()}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "生成预览图" }));
  expect(await screen.findByText(/生成预览图失败/)).not.toBeNull();
  expect(communityClient.addPreview).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "生成预览图" })).not.toBeNull();
});

test("publish dialog: a preview write from a replaced client is ignored", async () => {
  const missing = catalog(["ink-wash"]);
  missing.packages[0].preview = null;
  const pending = deferred<SkinCatalog>();
  const oldClient = client({
    packPreview: vi.fn().mockRejectedValue({ code: "candidate_skin_preview_required" }),
    addPreview: vi.fn().mockReturnValue(pending.promise),
  });
  const nextClient = client({
    packPreview: vi.fn().mockRejectedValue({ code: "candidate_skin_preview_required" }),
  });
  renderSkinPreview.mockResolvedValue([137, 80, 78, 71]);
  const view = render(
    <CandidateSkinPublishDialog
      client={oldClient}
      localSkins={vi.fn().mockResolvedValue(missing)}
      initialSkinId="ink-wash"
      readImage={vi.fn()}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "生成预览图" }));
  await waitFor(() => expect(oldClient.addPreview).toHaveBeenCalled());

  view.rerender(
    <CandidateSkinPublishDialog
      client={nextClient}
      localSkins={vi.fn().mockResolvedValue(missing)}
      initialSkinId="ink-wash"
      readImage={vi.fn()}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  await screen.findByRole("alert");
  expect((screen.getByRole("button", { name: "生成预览图" }) as HTMLButtonElement).disabled).toBe(
    false,
  );
  pending.resolve(catalog(["ink-wash"]));
  await settle();
  expect(nextClient.packPreview).toHaveBeenCalledTimes(1);
});

test("preview generation ignores a same-tick duplicate submission", async () => {
  const missing = catalog(["ink-wash"]);
  missing.packages[0].preview = null;
  const pending = deferred<SkinCatalog>();
  const communityClient = client({
    packPreview: vi.fn().mockRejectedValue({ code: "candidate_skin_preview_required" }),
    addPreview: vi.fn().mockReturnValue(pending.promise),
  });
  renderSkinPreview.mockResolvedValue([137, 80, 78, 71]);
  render(
    <CandidateSkinPublishDialog
      client={communityClient}
      localSkins={vi.fn().mockResolvedValue(missing)}
      initialSkinId="ink-wash"
      readImage={vi.fn()}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  const draw = await screen.findByRole("button", { name: "生成预览图" });
  act(() => {
    fireEvent.click(draw);
    fireEvent.click(draw);
  });
  await waitFor(() => expect(communityClient.addPreview).toHaveBeenCalled());
  expect(communityClient.addPreview).toHaveBeenCalledOnce();
  pending.resolve(catalog(["ink-wash"]));
  await settle();
});

test("publish dialog: without an image reader a missing preview is explained", async () => {
  const communityClient = client({
    packPreview: vi.fn().mockRejectedValue({ code: "candidate_skin_preview_required" }),
  });
  render(
    <CandidateSkinPublishDialog
      client={communityClient}
      localSkins={vi.fn().mockResolvedValue(catalog(["ink-wash"]))}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  expect(await screen.findByText(/用 preview 指定/)).not.toBeNull();
  expect(screen.queryByRole("button", { name: "生成预览图" })).toBeNull();
});

test("publish dialog: 公开发布 waits for the rights box and a valid name", async () => {
  const communityClient = client();
  render(
    <CandidateSkinPublishDialog
      client={communityClient}
      localSkins={vi.fn().mockResolvedValue(catalog(["ink-wash", "paper-cut"]))}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  const name = (await screen.findByRole("textbox", {
    name: "发布皮肤名称",
  })) as HTMLInputElement;
  expect(name.value).toBe("水墨");
  expect(screen.getByText("2 个文件 · 1.5 MB / 2 MB")).not.toBeNull();
  expect(screen.getByText("素材授权 CC-BY-4.0 / 代码授权 MIT")).not.toBeNull();
  const publish = screen.getByRole("button", {
    name: "公开发布",
  }) as HTMLButtonElement;
  expect(publish.disabled).toBe(true);
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));
  expect(publish.disabled).toBe(false);
  fireEvent.change(name, { target: { value: "   " } });
  expect(publish.disabled).toBe(true);
  fireEvent.change(name, { target: { value: "水墨二" } });
  expect(publish.disabled).toBe(false);
  expect((screen.getByRole("combobox", { name: "发布皮肤" }) as HTMLSelectElement).value).toBe(
    "ink-wash",
  );
});

test("publish dialog ignores a same-tick duplicate submission", async () => {
  const pending = deferred<CommunityCandidateSkin>();
  const publish = vi.fn().mockReturnValue(pending.promise);
  render(
    <CandidateSkinPublishDialog
      client={client({ publish })}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  await screen.findByRole("textbox", { name: "发布皮肤名称" });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));

  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
    fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  });
  expect(publish).toHaveBeenCalledOnce();
  pending.resolve(first);
  await settle();
});

test("replacing the publish client releases a pending dialog action", async () => {
  const pending = deferred<CommunityCandidateSkin>();
  const localSkins = vi.fn().mockResolvedValue(catalog(["ink-wash"]));
  const initial = client({ publish: vi.fn().mockReturnValue(pending.promise) });
  const replacement = client();
  const view = render(
    <CandidateSkinPublishDialog
      client={initial}
      localSkins={localSkins}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  await screen.findByRole("textbox", { name: "发布皮肤名称" });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));
  const publish = screen.getByRole("button", {
    name: "公开发布",
  }) as HTMLButtonElement;
  fireEvent.click(publish);
  await waitFor(() => expect(publish.disabled).toBe(true));

  view.rerender(
    <CandidateSkinPublishDialog
      client={replacement}
      localSkins={localSkins}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
  );
  await waitFor(() =>
    expect(
      (
        screen.getByRole("checkbox", {
          name: "确认拥有发布素材权利",
        }) as HTMLInputElement
      ).disabled,
    ).toBe(false),
  );
  pending.resolve(first);
});

test("publish dialog keeps the publication id on retry and renews it on edit", async () => {
  const publish = vi
    .fn()
    .mockRejectedValueOnce({ code: "community_invalid" })
    .mockRejectedValueOnce({ code: "community_rate_limited" })
    .mockResolvedValueOnce(first);
  const onPublished = vi.fn();
  render(
    <CandidateSkinPublishDialog
      client={client({ publish })}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={onPublished}
    />,
  );
  await screen.findByRole("textbox", { name: "发布皮肤名称" });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  expect((await screen.findByRole("alert")).textContent).toBe(
    "服务器未接受这款皮肤：请确认图片每边不超过 2048 像素、能被正常解码，且皮肤 ID 不与内置主题重名。",
  );
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  expect((await screen.findByText("发布太频繁，请稍后再试。")).textContent).toBeTruthy();
  const firstId = publish.mock.calls[0][1];
  expect(publish.mock.calls[1][1]).toBe(firstId);
  expect(publish.mock.calls[0]).toEqual(["ink-wash", firstId, "水墨", "", "public", "other"]);

  fireEvent.change(screen.getByRole("textbox", { name: "发布设计说明" }), {
    target: { value: " 淡墨 " },
  });
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  await waitFor(() => expect(onPublished).toHaveBeenCalledWith(first));
  expect(publish.mock.calls[2][1]).not.toBe(firstId);
  expect(publish.mock.calls[2][3]).toBe("淡墨");
});

test("the gallery publish button opens the dialog with local packages", async () => {
  const communityClient = client();
  render(
    <CommunityCandidateSkinsPage
      client={communityClient}
      localSkins={vi.fn().mockResolvedValue(catalog(["ink-wash"]))}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "发布我的皮肤" }));
  expect(await screen.findByRole("dialog", { name: "发布候选窗口皮肤" })).not.toBeNull();
  await waitFor(() =>
    expect(communityClient.packPreview).toHaveBeenCalledWith("ink-wash", "public"),
  );
});

test("publish dialog: 仅自己可见 checks the package as private and keeps the typed name", async () => {
  const privateSkin = { ...first, owned: true, visibility: "private" as const };
  const publish = vi.fn().mockResolvedValue(privateSkin);
  const communityClient = client({ publish });
  const onPublished = vi.fn();
  render(
    <CandidateSkinPublishDialog
      client={communityClient}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={onPublished}
    />,
  );
  const name = (await screen.findByRole("textbox", {
    name: "发布皮肤名称",
  })) as HTMLInputElement;
  expect((screen.getByRole("radio", { name: /公开/ }) as HTMLInputElement).checked).toBe(true);
  fireEvent.change(name, { target: { value: "我的水墨" } });
  fireEvent.click(screen.getByRole("radio", { name: "仅自己可见" }));
  await waitFor(() =>
    expect(communityClient.packPreview).toHaveBeenLastCalledWith("ink-wash", "private"),
  );
  await screen.findByRole("checkbox", { name: "确认拥有发布素材权利" });
  expect((screen.getByRole("textbox", { name: "发布皮肤名称" }) as HTMLInputElement).value).toBe(
    "我的水墨",
  );
  expect(screen.queryByRole("button", { name: "公开发布" })).toBeNull();
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));
  fireEvent.click(screen.getByRole("button", { name: "保存到我的皮肤库" }));
  await waitFor(() => expect(onPublished).toHaveBeenCalledWith(privateSkin));
  expect(publish.mock.calls[0].slice(2)).toEqual(["我的水墨", "", "private", "other"]);
});

test("owners switch a package between public and private, and only public ones are taken down", async () => {
  const hidden = { ...first, owned: true, visibility: "private" as const };
  const communityClient = client({
    list: vi.fn().mockResolvedValue({ skins: [hidden], has_more: false }),
    detail: vi.fn().mockResolvedValue(hidden),
  });
  render(<CommunityCandidateSkinsPage client={communityClient} />);
  expect(await screen.findByText("我的作品 · 私有")).not.toBeNull();
  await openDetail();
  expect(screen.getByText("私有")).not.toBeNull();
  expect(screen.queryByRole("button", { name: "下架这款皮肤" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "公开" }));
  await waitFor(() =>
    expect(communityClient.setVisibility).toHaveBeenCalledWith(first.id, "public"),
  );
  expect(await screen.findByText("已公开，其他用户现在可以下载这款皮肤。")).not.toBeNull();
  expect(screen.getByRole("button", { name: "下架这款皮肤" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "设为私有" }));
  await waitFor(() =>
    expect(communityClient.setVisibility).toHaveBeenLastCalledWith(first.id, "private"),
  );
  expect(await screen.findByText("已设为私有，只有你能看到这款皮肤。")).not.toBeNull();
});

test("a refused switch to public shows the fixed sentence and keeps the package private", async () => {
  const hidden = { ...first, owned: true, visibility: "private" as const };
  render(
    <CommunityCandidateSkinsPage
      client={client({
        list: vi.fn().mockResolvedValue({ skins: [hidden], has_more: false }),
        detail: vi.fn().mockResolvedValue(hidden),
        setVisibility: vi.fn().mockRejectedValue({ code: "community_invalid", message: "raw" }),
      })}
    />,
  );
  await openDetail();
  fireEvent.click(screen.getByRole("button", { name: "公开" }));
  expect((await screen.findByRole("alert")).textContent).toBe("内容无效，请修改后重试。");
  expect(screen.getByText("私有")).not.toBeNull();
});

function communityPage(communityClient: CandidateSkinCommunityClient, localSkins = vi.fn()) {
  localSkins.mockResolvedValue(catalog(["ink-wash"]));
  render(
    <CommunityPage
      theme="light"
      candidateSkins={communityClient}
      localSkins={localSkins}
      openSkinDirectory={vi.fn().mockResolvedValue(undefined)}
    />,
  );
  return localSkins;
}

test("the community page syncs as it opens without listing the skin directory", async () => {
  const sync = vi
    .fn()
    .mockResolvedValue(report({ uploaded: ["ink-wash"], downloaded: ["paper-cut"] }));
  communityPage(client({ sync }));
  await waitFor(() => expect(sync).toHaveBeenCalledOnce());
  await settle();
  // The directory row and the sync outcome live on 主题 and nowhere, respectively.
  expect(screen.queryByText("本地皮肤")).toBeNull();
  expect(screen.queryByText("外部皮肤")).toBeNull();
  expect(screen.queryByText(/已同步/)).toBeNull();
  expect(screen.queryByRole("button", { name: "打开目录" })).toBeNull();
});

test("an install from the gallery syncs again, queued behind a run in progress", async () => {
  const running = deferred<CandidateSkinSyncReport>();
  const sync = vi
    .fn()
    .mockReturnValueOnce(running.promise)
    .mockResolvedValue(report({ uploaded: ["ink-wash"] }));
  communityPage(client({ sync }));
  await openDetail();
  // The pre-install scan finds ink-wash, so the install asks first.
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  fireEvent.click(await screen.findByRole("button", { name: "替换安装" }));
  expect(await screen.findByText("已安装到外部皮肤。")).not.toBeNull();
  expect(sync).toHaveBeenCalledOnce();
  await act(async () => running.resolve(report()));
  await waitFor(() => expect(sync).toHaveBeenCalledTimes(2));
});

test("category chips filter the list from its first page and load more keeps the category", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({ skins: [first, second], has_more: true })
    .mockResolvedValueOnce({ skins: [second], has_more: true })
    .mockResolvedValueOnce({ skins: [third], has_more: false })
    .mockResolvedValueOnce({ skins: [first], has_more: false });
  render(<CommunityCandidateSkinsPage client={client({ list })} />);
  await waitFor(() => expect(list).toHaveBeenCalledWith(0, "", false, null));
  const chips = screen.getByRole("group", { name: "候选窗口皮肤分类" });
  const labels = within(chips)
    .getAllByRole("button")
    .map((button) => button.textContent);
  expect(labels).toEqual([
    "全部",
    "自然",
    "国风",
    "二次元",
    "可爱",
    "美食",
    "科技夜色",
    "简约",
    "其他",
  ]);
  expect(within(chips).getByRole("button", { name: "全部" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  await screen.findByRole("button", { name: "查看候选窗口皮肤 水墨" });

  fireEvent.click(within(chips).getByRole("button", { name: "国风" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", false, "guofeng"));
  expect(within(chips).getByRole("button", { name: "国风" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: "查看候选窗口皮肤 水墨" })).toBeNull(),
  );
  fireEvent.click(await screen.findByRole("button", { name: "加载更多" }));
  // 换分类后从 0 重新分页，「加载更多」接着新分类第一页的 offset。
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(1, "", false, "guofeng"));
  expect(await screen.findByRole("button", { name: "查看候选窗口皮肤 朱砂" })).not.toBeNull();

  fireEvent.click(within(chips).getByRole("button", { name: "全部" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", false, null));
});

test("a failed category switch keeps the previous category selected", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({ skins: [first], has_more: false })
    .mockRejectedValueOnce({ code: "community_unavailable" });
  render(<CommunityCandidateSkinsPage client={client({ list })} />);
  await screen.findByRole("button", { name: "查看候选窗口皮肤 水墨" });
  const chips = screen.getByRole("group", { name: "候选窗口皮肤分类" });
  fireEvent.click(within(chips).getByRole("button", { name: "美食" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(0, "", false, "food"));
  await settle();
  expect(within(chips).getByRole("button", { name: "美食" }).getAttribute("aria-pressed")).toBe(
    "false",
  );
  expect(within(chips).getByRole("button", { name: "全部" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  expect(screen.getByRole("button", { name: "查看候选窗口皮肤 水墨" })).not.toBeNull();
});

test("a failed rapid category switch returns to the category of the displayed list", async () => {
  const pending = deferred<CommunityCandidateSkinPage>();
  const failed = deferred<CommunityCandidateSkinPage>();
  const list = vi
    .fn()
    .mockResolvedValueOnce({ skins: [first, second], has_more: true })
    .mockReturnValueOnce(pending.promise)
    .mockReturnValueOnce(failed.promise)
    .mockResolvedValueOnce({ skins: [third], has_more: false });
  render(<CommunityCandidateSkinsPage client={client({ list })} />);
  await screen.findByRole("button", { name: "查看候选窗口皮肤 青绿" });
  const chips = screen.getByRole("group", { name: "候选窗口皮肤分类" });

  fireEvent.click(within(chips).getByRole("button", { name: "自然" }));
  fireEvent.click(within(chips).getByRole("button", { name: "美食" }));
  await act(async () => failed.reject({ code: "community_unavailable" }));

  expect(within(chips).getByRole("button", { name: "全部" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  fireEvent.click(screen.getByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith(2, "", false, null));
  await act(async () => pending.resolve({ skins: [first], has_more: false }));
});

test("cards and the detail view show the category label, and an item without one shows none", async () => {
  const cute = skin(first.id, "水墨", { category: "cute" });
  const plain = skin(second.id, "青绿");
  render(
    <CommunityCandidateSkinsPage
      client={client({
        list: vi.fn().mockResolvedValue({ skins: [cute, plain], has_more: false }),
        detail: vi.fn().mockResolvedValue(cute),
      })}
    />,
  );
  const card = await screen.findByRole("button", {
    name: "查看候选窗口皮肤 水墨",
  });
  expect(within(card).getByText("可爱 · 示例作者")).not.toBeNull();
  const other = screen.getByRole("button", { name: "查看候选窗口皮肤 青绿" });
  expect(within(other).getByText("示例作者")).not.toBeNull();
  await openDetail();
  expect(screen.getByText("可爱 · 示例作者 · v1.0.0")).not.toBeNull();
  // 不是自己的作品时不能改分类。
  expect(screen.queryByRole("combobox", { name: "修改分类" })).toBeNull();
});

test("publish dialog: the category defaults to 其他 and the chosen one is sent", async () => {
  const publish = vi.fn().mockResolvedValue(first);
  const onPublished = vi.fn();
  render(
    <CandidateSkinPublishDialog
      client={client({ publish })}
      initialSkinId="ink-wash"
      onClose={vi.fn()}
      onPublished={onPublished}
    />,
  );
  await screen.findByRole("textbox", { name: "发布皮肤名称" });
  const select = screen.getByRole("combobox", {
    name: "发布分类",
  }) as HTMLSelectElement;
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
  await waitFor(() => expect(onPublished).toHaveBeenCalledWith(first));
  expect(publish.mock.calls[0].slice(2)).toEqual(["水墨", "", "public", "tech"]);
});

test("owners change the category of their package from the detail view", async () => {
  const mine = skin(first.id, "水墨", { owned: true, category: "nature" });
  const communityClient = client({
    list: vi.fn().mockResolvedValue({ skins: [mine], has_more: false }),
    detail: vi.fn().mockResolvedValue(mine),
  });
  render(<CommunityCandidateSkinsPage client={communityClient} />);
  await openDetail();
  const select = (await screen.findByRole("combobox", {
    name: "修改分类",
  })) as HTMLSelectElement;
  expect(select.value).toBe("nature");
  fireEvent.change(select, { target: { value: "minimal" } });
  await waitFor(() =>
    expect(communityClient.setCategory).toHaveBeenCalledWith(first.id, "minimal"),
  );
  expect(await screen.findByText("已改为「简约」分类。")).not.toBeNull();
  expect((screen.getByRole("combobox", { name: "修改分类" }) as HTMLSelectElement).value).toBe(
    "minimal",
  );
});
