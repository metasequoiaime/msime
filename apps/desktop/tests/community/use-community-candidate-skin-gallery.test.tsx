// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useCommunityCandidateSkinGallery,
  type CandidateSkinCommunityClient,
  type CommunityCandidateSkin,
  type SkinCatalog,
} from "@msime/ui";
import { subscribeSkinCatalogChanges } from "../../../../packages/ui/src/skin/skin-catalog-changes";

afterEach(cleanup);

const skin: CommunityCandidateSkin = {
  id: "synthetic-candidate",
  package_id: "synthetic-package",
  name: "合成皮肤",
  description: "合成说明",
  author: "合成作者",
  version: "1.0.0",
  license: { code: "MIT", assets: "CC0", source: "" },
  size: 1024,
  file_count: 1,
  downloads: 0,
  rating_count: 0,
  rating_average: 0,
  owned: true,
  my_rating: 0,
  created_at: "2026-10-17T00:00:00Z",
  updated_at: "2026-10-17T00:00:00Z",
  visibility: "public",
};
const catalog: SkinCatalog = { directory: "/synthetic/skins", packages: [], issues: [] };

function client(): CandidateSkinCommunityClient {
  return {
    list: vi.fn().mockImplementation(async () => ({ skins: [skin], has_more: false })),
    detail: vi.fn().mockResolvedValue(skin),
    preview: vi.fn().mockResolvedValue({ dataUrl: "data:image/png;base64,AA==" }),
    install: vi.fn().mockResolvedValue(catalog),
    packPreview: vi.fn(),
    addPreview: vi.fn(),
    addLicense: vi.fn(),
    publish: vi.fn(),
    setVisibility: vi.fn(),
    setCategory: vi.fn(),
    sync: vi.fn().mockResolvedValue({
      uploaded: [],
      downloaded: [],
      deleted_local: [],
      deleted_cloud: [],
      skipped: [],
      stopped: null,
    }),
    rate: vi.fn(),
    unpublish: vi.fn(),
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("候选图库独立提供分类搜索与发布完成刷新，打开和浏览不自动同步", async () => {
  const community = client();
  const { result } = renderHook(() => useCommunityCandidateSkinGallery({ client: community }));
  await waitFor(() => expect(result.current.skins).toHaveLength(1));
  expect(community.list).toHaveBeenCalledExactlyOnceWith(0, "", false, null);
  await act(async () => {
    await result.current.requestList("合成检索", false);
  });
  await act(async () => {
    await result.current.changeCategory("nature");
  });
  expect(community.list).toHaveBeenLastCalledWith(0, "合成检索", false, "nature");
  act(() => result.current.setPublishOpen(true));
  await act(async () => {
    await result.current.publishDone({ ...skin, visibility: "private" });
  });
  expect(result.current.publishOpen).toBe(false);
  expect(result.current.actionNotice).toBe("已保存到你的皮肤库，仅自己可见。");
  expect(community.list).toHaveBeenLastCalledWith(0, "合成检索", false, "nature");
  expect(community.sync).not.toHaveBeenCalled();
  expect(community.install).not.toHaveBeenCalled();
});

test("预览在同一客户端共用 Promise，失败后可重新读取", async () => {
  const community = client();
  vi.mocked(community.preview).mockRejectedValueOnce(new Error("synthetic preview failure"));
  const { result } = renderHook(() => useCommunityCandidateSkinGallery({ client: community }));
  const failed = result.current.loadPreview(skin.id, skin.updated_at);
  expect(result.current.loadPreview(skin.id, skin.updated_at)).toBe(failed);
  await expect(failed).rejects.toThrow("synthetic preview failure");
  await expect(result.current.loadPreview(skin.id, skin.updated_at)).resolves.toBe(
    "data:image/png;base64,AA==",
  );
  expect(community.preview).toHaveBeenCalledTimes(2);
});

test("作者原地替换作品后 updated_at 变化，预览重新读取且不再给出旧图", async () => {
  const community = client();
  vi.mocked(community.preview)
    .mockResolvedValueOnce({ dataUrl: "data:image/png;base64,AA==" })
    .mockResolvedValueOnce({ dataUrl: "data:image/png;base64,AQ==" });
  const { result } = renderHook(() => useCommunityCandidateSkinGallery({ client: community }));
  await expect(result.current.loadPreview(skin.id, skin.updated_at)).resolves.toBe(
    "data:image/png;base64,AA==",
  );
  const replaced = "2026-10-20T00:26:00Z";
  await expect(result.current.loadPreview(skin.id, replaced)).resolves.toBe(
    "data:image/png;base64,AQ==",
  );
  await expect(result.current.loadPreview(skin.id, replaced)).resolves.toBe(
    "data:image/png;base64,AQ==",
  );
  expect(community.preview).toHaveBeenCalledTimes(2);
  expect(community.preview).toHaveBeenLastCalledWith(skin.id);
});

test("更换客户端后预览缓存隔离，旧 Promise 仍交给旧消费者收尾", async () => {
  const pending = deferred<{ dataUrl: string }>();
  const previous = client();
  vi.mocked(previous.preview).mockReturnValue(pending.promise);
  const next = client();
  const { result, rerender } = renderHook(
    ({ community }) => useCommunityCandidateSkinGallery({ client: community }),
    { initialProps: { community: previous } },
  );
  const oldPreview = result.current.loadPreview(skin.id, skin.updated_at);
  rerender({ community: next });
  await expect(result.current.loadPreview(skin.id, skin.updated_at)).resolves.toBe(
    "data:image/png;base64,AA==",
  );
  pending.resolve({ dataUrl: "data:image/png;base64,AQ==" });
  await expect(oldPreview).resolves.toBe("data:image/png;base64,AQ==");
  await expect(result.current.loadPreview(skin.id, skin.updated_at)).resolves.toBe(
    "data:image/png;base64,AA==",
  );
  expect(next.preview).toHaveBeenCalledOnce();
});

test("安装预检期间更换客户端后不再调用旧安装", async () => {
  const pending = deferred<SkinCatalog>();
  const previous = client();
  const next = client();
  const localSkins = vi.fn().mockReturnValue(pending.promise);
  const { result, rerender } = renderHook(
    ({ community }) => useCommunityCandidateSkinGallery({ client: community, localSkins }),
    { initialProps: { community: previous } },
  );
  await waitFor(() => expect(result.current.skins).toHaveLength(1));
  await act(async () => result.current.open(skin));
  await waitFor(() => expect(result.current.detailBusy).toBe(false));
  let installing!: Promise<void>;
  act(() => {
    installing = result.current.install(false);
  });
  expect(localSkins).toHaveBeenCalledOnce();
  rerender({ community: next });
  await act(async () => {
    pending.resolve(catalog);
    await installing;
  });
  expect(previous.install).not.toHaveBeenCalled();
  expect(next.install).not.toHaveBeenCalled();
  expect(result.current.installed).toBe(false);
  expect(result.current.confirmReplace).toBe(false);
});

test("旧客户端已完成的安装仍通知目录重扫，但不写入新客户端状态", async () => {
  const pending = deferred<SkinCatalog>();
  const previous = client();
  vi.mocked(previous.install).mockReturnValue(pending.promise);
  const next = client();
  const changed = vi.fn();
  const unsubscribe = subscribeSkinCatalogChanges(changed);
  try {
    const { result, rerender } = renderHook(
      ({ community }) => useCommunityCandidateSkinGallery({ client: community }),
      { initialProps: { community: previous } },
    );
    await waitFor(() => expect(result.current.skins).toHaveLength(1));
    await act(async () => result.current.open(skin));
    await waitFor(() => expect(result.current.detailBusy).toBe(false));
    let installing!: Promise<void>;
    act(() => {
      installing = result.current.install(false);
    });
    expect(previous.install).toHaveBeenCalledExactlyOnceWith(skin.id, false);
    rerender({ community: next });
    await act(async () => {
      pending.resolve(catalog);
      await installing;
    });
    expect(changed).toHaveBeenCalledOnce();
    expect(result.current.installed).toBe(false);
    expect(next.install).not.toHaveBeenCalled();
  } finally {
    unsubscribe();
  }
});
