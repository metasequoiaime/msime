// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { usePluginsSection } from "../../../../packages/ui/src/settings/use-plugins-section";
import { defaultPluginPreferences } from "../../../../packages/ui/src/settings/plugin-preferences";
import type {
  MentionEntry,
  PluginCatalogResult,
  PluginClient,
  PluginPackage,
  ScriptGrantsResult,
} from "../../../../packages/ui/src/settings/plugin-types";

afterEach(cleanup);

const pack: PluginPackage = {
  id: "synthetic-sound",
  name: "合成音效",
  kind: "sound",
  version: "1.0.0",
  license: "CC0-1.0",
  author: null,
  description: null,
  builtin: false,
};
const catalog: PluginCatalogResult = { packages: [pack], issues: [] };
const mentions: MentionEntry[] = [{ text: "合成地点", key: "he'cheng" }];
const grants: ScriptGrantsResult = { revision: 2, plugins: [], health: [], commands: [] };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

function client(overrides: Partial<PluginClient> = {}): PluginClient {
  return {
    catalog: vi.fn(async () => catalog),
    importPack: vi.fn(async () => null),
    remove: vi.fn(async () => {}),
    loadMentions: vi.fn(async () => mentions),
    saveMentions: vi.fn(async () => {}),
    scriptGrants: vi.fn(async () => grants),
    ...overrides,
  };
}

function options(host: PluginClient): Parameters<typeof usePluginsSection>[0] {
  return {
    client: host,
    preferences: defaultPluginPreferences,
    keySound: true,
    music: false,
    triggers: true,
    typingEffects: false,
    effectStyles: false,
    effectPacks: false,
    mentionMode: true,
    helpcode: false,
    scriptPlugins: true,
    active: true,
    onChange: vi.fn(),
    onError: vi.fn(),
    confirm: vi.fn(async () => true),
  };
}

test("重访只关闭详情，保留筛选与名单编辑，并以最新宿主名单判断是否已保存", async () => {
  const nextMentions = [{ text: "合成新地点", key: "xin" }];
  const host = client({
    loadMentions: vi.fn().mockResolvedValueOnce(mentions).mockResolvedValueOnce(nextMentions),
  });
  const initial = options(host);
  const { result, rerender } = renderHook(usePluginsSection, { initialProps: initial });
  await waitFor(() => expect(result.current.mentions).toEqual(mentions));
  const edited = [{ text: "合成编辑", key: "bian'ji" }];
  const filter = { kind: "sound" as const, query: "合成" };
  act(() => {
    result.current.setMentions(edited);
    result.current.setLibraryFilter(filter);
    result.current.openView({ kind: "mentions" }, "mentions");
  });
  rerender({ ...initial, active: false });
  expect(result.current.view).toEqual({ kind: "list" });
  expect(result.current.libraryFilter).toEqual(filter);
  rerender(initial);
  await waitFor(() => expect(host.loadMentions).toHaveBeenCalledTimes(2));
  expect(result.current.mentions).toEqual(edited);
  expect(result.current.mentionsDirty).toBe(true);
  act(() => result.current.setMentions(nextMentions));
  expect(result.current.mentionsDirty).toBe(false);

  rerender({ ...initial, onError: vi.fn() });
  expect(host.catalog).toHaveBeenCalledTimes(2);
  expect(host.loadMentions).toHaveBeenCalledTimes(2);
  expect(host.scriptGrants).toHaveBeenCalledTimes(2);
});

test.each([
  [false, "success"],
  [false, "failure"],
  [true, "success"],
  [true, "failure"],
] as const)("替换客户端隔离旧目录、授权和名单的 %s / %s 结果", async (strict, outcome) => {
  const oldCatalog = deferred<PluginCatalogResult>();
  const oldMentions = deferred<MentionEntry[]>();
  const oldGrants = deferred<ScriptGrantsResult>();
  const initial = options(
    client({
      catalog: vi.fn(() => oldCatalog.promise),
      loadMentions: vi.fn(() => oldMentions.promise),
      scriptGrants: vi.fn(() => oldGrants.promise),
    }),
  );
  const { result, rerender } = renderHook(usePluginsSection, {
    initialProps: initial,
    wrapper: strict ? StrictMode : undefined,
  });
  rerender({ ...initial, client: client() });
  await waitFor(() => expect(result.current.catalogState).toBe("loaded"));
  await act(async () => {
    if (outcome === "success") {
      oldCatalog.resolve({ packages: [], issues: [] });
      oldMentions.resolve([{ text: "合成旧地点", key: "jiu" }]);
      oldGrants.resolve({ ...grants, revision: 1 });
    } else {
      oldCatalog.reject(new Error("synthetic offline"));
      oldMentions.reject(new Error("synthetic offline"));
      oldGrants.reject(new Error("synthetic offline"));
    }
  });
  expect(result.current.catalog).toEqual(catalog);
  expect(result.current.mentions).toEqual(mentions);
  expect(result.current.scriptGrants).toEqual(grants);
  expect(result.current.catalogState).toBe("loaded");
  expect(initial.onError).not.toHaveBeenCalled();
});

test("导入成功后的重读失败保留已加载目录，并单独报告读取失败", async () => {
  const initial = options(
    client({
      catalog: vi.fn().mockResolvedValueOnce(catalog).mockRejectedValueOnce(new Error("offline")),
      importPack: vi.fn(async () => pack),
    }),
  );
  const { result } = renderHook(usePluginsSection, { initialProps: initial });
  await waitFor(() => expect(result.current.catalogState).toBe("loaded"));
  await act(async () => result.current.importPack("archive"));
  expect(result.current.catalog).toEqual(catalog);
  expect(result.current.packsListed).toBe(true);
  expect(result.current.notice).toContain("已导入");
  expect(result.current.working).toBe(false);
  expect(vi.mocked(initial.onError).mock.calls).toEqual([[""], ["无法读取插件列表，请重试。"]]);
});
