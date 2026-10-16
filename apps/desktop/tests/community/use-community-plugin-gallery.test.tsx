// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useCommunityPluginGallery,
  type CommunityPlugin,
  type CommunityPluginClient,
  type PluginCatalogResult,
  type PluginPackage,
} from "@msime/ui";

afterEach(cleanup);

const plugin: CommunityPlugin = {
  id: "synthetic-publication",
  plugin_id: "synthetic-pack",
  kind: "sound",
  name: "合成插件",
  description: "合成说明",
  author: "合成作者",
  version: "1.0.0",
  license: "MIT",
  size: 1024,
  sha256: "0".repeat(64),
  downloads: 0,
  rating_count: 0,
  rating_average: 0,
  owned: false,
  my_rating: 0,
  created_at: "2026-10-17T00:00:00Z",
};
const pack: PluginPackage = {
  id: plugin.plugin_id,
  kind: plugin.kind,
  name: plugin.name,
  version: plugin.version,
  license: plugin.license,
  author: null,
  description: null,
  builtin: false,
};

function client(): CommunityPluginClient {
  return {
    list: vi.fn().mockImplementation(async () => ({ plugins: [plugin], has_more: false })),
    detail: vi.fn().mockResolvedValue(plugin),
    install: vi.fn().mockResolvedValue(pack),
    rate: vi.fn().mockResolvedValue({ stars: 5 }),
    delete: vi.fn().mockResolvedValue({ deleted: true }),
    packPreview: vi.fn(),
    publish: vi.fn(),
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("插件图库独立组合搜索、类型、作品范围与发布刷新", async () => {
  const community = client();
  const { result } = renderHook(() => useCommunityPluginGallery({ client: community }));
  await waitFor(() => expect(result.current.plugins).toHaveLength(1));
  await act(async () => {
    await result.current.requestList("合成搜索", false);
  });
  await act(async () => {
    await result.current.changeKind("music");
  });
  await act(async () => {
    await result.current.changeMineOnly(true);
  });
  expect(community.list).toHaveBeenLastCalledWith(0, "合成搜索", "music", true);
  act(() => result.current.setPublishOpen(true));
  await act(async () => {
    await result.current.publishDone();
  });
  expect(result.current.publishOpen).toBe(false);
  expect(result.current.actionNotice).toBe("已发布到社区。");
  expect(community.list).toHaveBeenLastCalledWith(0, "合成搜索", "music", true);
});

test("插件类型首屏失败恢复旧类型，并保留跳过行的分页偏移", async () => {
  const community = client();
  vi.mocked(community.list)
    .mockResolvedValueOnce({ plugins: [plugin], has_more: true, skipped: 3 })
    .mockRejectedValueOnce(new Error("synthetic failure"))
    .mockResolvedValueOnce({ plugins: [], has_more: false });
  const { result } = renderHook(() => useCommunityPluginGallery({ client: community }));
  await waitFor(() => expect(result.current.plugins).toHaveLength(1));
  await act(async () => {
    await result.current.changeKind("music");
  });
  expect(result.current.kind).toBe(null);
  await act(async () => {
    await result.current.requestList("", true);
  });
  expect(community.list).toHaveBeenLastCalledWith(4, "", null, false);
});

test("同类型同标识安装先确认，成功后通知并关闭详情重置状态", async () => {
  const community = client();
  const onInstalled = vi.fn();
  const { result } = renderHook(() =>
    useCommunityPluginGallery({
      client: community,
      localPlugins: async () => ({ packages: [pack], issues: [] }),
      onInstalled,
    }),
  );
  await waitFor(() => expect(result.current.plugins).toHaveLength(1));
  await act(async () => {
    await result.current.open(plugin);
  });
  await act(async () => {
    await result.current.install(false);
  });
  expect(result.current.confirmReplace).toBe(true);
  expect(community.install).not.toHaveBeenCalled();
  await act(async () => {
    await result.current.install(true);
  });
  expect(community.install).toHaveBeenCalledExactlyOnceWith(
    plugin.id,
    plugin.kind,
    plugin.plugin_id,
  );
  expect(onInstalled).toHaveBeenCalledExactlyOnceWith(pack);
  expect(result.current.installed).toBe(true);
  expect(result.current.confirmReplace).toBe(false);
  act(() => result.current.closeDetail());
  expect(result.current.selected).toBe(null);
  expect(result.current.installed).toBe(false);
});

test("切换客户端后旧安装预检不能安装或覆盖当前界面", async () => {
  const pending = deferred<PluginCatalogResult>();
  const initial = client();
  const replacement = client();
  const onInstalled = vi.fn();
  const { result, rerender } = renderHook(
    ({ community }) =>
      useCommunityPluginGallery({
        client: community,
        localPlugins: () => pending.promise,
        onInstalled,
      }),
    { initialProps: { community: initial } },
  );
  await waitFor(() => expect(result.current.plugins).toHaveLength(1));
  await act(async () => {
    await result.current.open(plugin);
  });
  let installing!: Promise<void>;
  act(() => {
    installing = result.current.install(false);
  });
  rerender({ community: replacement });
  await waitFor(() => expect(replacement.list).toHaveBeenCalled());
  await act(async () => {
    pending.resolve({ packages: [], issues: [] });
    await installing;
  });
  expect(initial.install).not.toHaveBeenCalled();
  expect(replacement.install).not.toHaveBeenCalled();
  expect(onInstalled).not.toHaveBeenCalled();
  expect(result.current.installed).toBe(false);
  expect(result.current.confirmReplace).toBe(false);
});
