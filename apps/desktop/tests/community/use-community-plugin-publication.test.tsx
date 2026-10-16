// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useCommunityPluginPublication } from "../../../../packages/ui/src/community/use-community-plugin-publication";
import type {
  CommunityPlugin,
  CommunityPluginPackPreview,
} from "../../../../packages/ui/src/community/community-plugin-model";
import type { PluginCatalogResult, PluginPackage } from "@msime/ui";

afterEach(cleanup);

const preview: CommunityPluginPackPreview = {
  suggestedName: "示例雨声",
  suggestedDescription: "合成发布说明",
  version: "1.0.0",
  license: "MIT",
  fileCount: 3,
  size: 2048,
};
const published: CommunityPlugin = {
  id: "10000000-0000-4000-8000-000000000001",
  kind: "sound",
  plugin_id: "rain",
  name: "示例雨声",
  description: "合成发布说明",
  author: "示例作者",
  version: "1.0.0",
  license: "MIT",
  size: 2048,
  sha256: "0".repeat(64),
  downloads: 0,
  rating_count: 0,
  rating_average: 0,
  owned: true,
  my_rating: 0,
  created_at: "2026-10-16T00:00:00Z",
};
function pack(kind: PluginPackage["kind"], id: string, builtin = false): PluginPackage {
  return {
    kind,
    id,
    builtin,
    name: `示例 ${id}`,
    version: "1.0.0",
    license: "MIT",
    author: null,
    description: null,
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}
function options() {
  return {
    client: {
      packPreview: vi.fn(async (): Promise<CommunityPluginPackPreview> => preview),
      publish: vi.fn(
        async (
          _kind: string,
          _pluginId: string,
          _id: string,
          _name: string,
          _description: string,
        ): Promise<CommunityPlugin> => published,
      ),
    },
    localPlugins: vi.fn(async (): Promise<PluginCatalogResult> => ({
      packages: [pack("sound", "rain"), pack("music", "rain")],
      issues: [],
    })),
    onPublished: vi.fn(async (_plugin: CommunityPlugin) => {}),
  };
}

test("过滤内置与脚本包，同标识不同类型可独立选择并提交规范化字段", async () => {
  const props = options();
  props.localPlugins.mockResolvedValue({
    packages: [
      pack("sound", "builtin", true),
      pack("script", "sample-script"),
      pack("effect", "sparkle"),
      pack("sound", "rain"),
      pack("music", "rain"),
    ],
    issues: [],
  });
  const { result } = renderHook(() => useCommunityPluginPublication(props));
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  await waitFor(() => expect(result.current.pack).toEqual(preview));
  expect(result.current.options.map((item) => item.key)).toEqual([
    "effect/sparkle",
    "sound/rain",
    "music/rain",
  ]);
  expect(result.current.selection).toBe("effect/sparkle");
  act(() => result.current.selectPlugin("music/rain"));
  await waitFor(() => expect(result.current.pack).toEqual(preview));
  expect(props.client.packPreview).toHaveBeenLastCalledWith("music", "rain");
  expect(result.current.ready).toBe(false);
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  act(() => {
    result.current.onNameChange("  合成音乐  ");
    result.current.onDescriptionChange("  合成说明  ");
    result.current.onAgreedChange(true);
  });
  expect(result.current.ready).toBe(true);
  await act(async () => result.current.submit());
  expect(props.client.publish).toHaveBeenCalledWith(
    "music",
    "rain",
    expect.stringMatching(/^[0-9a-f-]{36}$/),
    "合成音乐",
    "合成说明",
  );
  expect(props.onPublished).toHaveBeenCalledWith(published);
});

test("重试沿用 ID，授权不改 ID，名称、说明与所选包改变时重建 ID", async () => {
  const props = options();
  props.client.publish.mockRejectedValue({ code: "community_unavailable" });
  const { result } = renderHook(() => useCommunityPluginPublication(props));
  await waitFor(() => expect(result.current.pack).toEqual(preview));
  act(() => result.current.onAgreedChange(true));
  const submit = async () => {
    await act(async () => result.current.submit());
    return props.client.publish.mock.lastCall![2];
  };
  const initial = await submit();
  expect(await submit()).toBe(initial);
  act(() => {
    result.current.onAgreedChange(false);
    result.current.onAgreedChange(true);
  });
  expect(await submit()).toBe(initial);
  let previous = initial;
  for (const edit of [
    () => result.current.onNameChange("新名称"),
    () => result.current.onDescriptionChange("新说明"),
    () => result.current.selectPlugin("music/rain"),
  ]) {
    act(edit);
    await waitFor(() => expect(result.current.packLoading).toBe(false));
    if (result.current.selection === "music/rain") expect(result.current.agreed).toBe(false);
    act(() => result.current.onAgreedChange(true));
    const next = await submit();
    expect(next).not.toBe(previous);
    previous = next;
  }
  expect(props.client.publish.mock.lastCall!.slice(0, 2)).toEqual(["music", "rain"]);
  expect(props.onPublished).not.toHaveBeenCalled();
});

test("名称按字素截断，空名称与超长说明不能发布，实例不共享授权", async () => {
  const props = options();
  props.client.packPreview.mockResolvedValue({ ...preview, suggestedName: "😀".repeat(40) });
  const left = renderHook(() => useCommunityPluginPublication(props));
  const right = renderHook(() => useCommunityPluginPublication(props));
  await waitFor(() => expect(left.result.current.pack && right.result.current.pack).toBeTruthy());
  expect(left.result.current.name).toBe("😀".repeat(32));
  act(() => {
    left.result.current.onAgreedChange(true);
    left.result.current.onNameChange("  ");
  });
  await act(async () => left.result.current.submit());
  act(() => {
    left.result.current.onNameChange("有效名称");
    left.result.current.onDescriptionChange("字".repeat(281));
  });
  expect(left.result.current.ready).toBe(false);
  await act(async () => left.result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  expect(right.result.current.name).toBe("😀".repeat(32));
  expect(right.result.current.agreed).toBe(false);
});

test.each(["success", "failure"])("切换包后忽略旧预检的 %s 与 finally", async (outcome) => {
  const props = options();
  const old = deferred<CommunityPluginPackPreview>();
  const next = deferred<CommunityPluginPackPreview>();
  props.client.packPreview.mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
  const { result } = renderHook(() => useCommunityPluginPublication(props));
  await waitFor(() => expect(props.client.packPreview).toHaveBeenCalledOnce());
  act(() => result.current.selectPlugin("music/rain"));
  await act(async () => {
    if (outcome === "success") old.resolve(preview);
    else old.reject({ code: "plugin_community_too_large" });
  });
  expect(result.current.pack).toBeNull();
  expect(result.current.packError).toBe("");
  expect(result.current.packLoading).toBe(true);
  await act(async () => next.resolve({ ...preview, suggestedName: "当前音乐" }));
  expect(result.current.name).toBe("当前音乐");
  expect(result.current.packLoading).toBe(false);
});

test.each(["client", "localPlugins"] as const)(
  "替换 %s 后旧目录的成功、失败与 finally 不覆盖当前读取",
  async (owner) => {
    const props = options();
    const old = deferred<PluginCatalogResult>();
    const middleRead = deferred<PluginCatalogResult>();
    const nextRead = deferred<PluginCatalogResult>();
    props.localPlugins.mockReturnValueOnce(old.promise).mockReturnValueOnce(middleRead.promise);
    const { result, rerender } = renderHook((next) => useCommunityPluginPublication(next), {
      initialProps: props,
    });
    rerender({
      ...props,
      [owner]: owner === "client" ? options().client : vi.fn(() => middleRead.promise),
    });
    props.localPlugins.mockReturnValue(nextRead.promise);
    rerender({
      ...props,
      [owner]: owner === "client" ? options().client : vi.fn(() => nextRead.promise),
    });
    await act(async () => {
      old.resolve({ packages: [pack("sound", "old")], issues: [] });
      middleRead.reject(new Error("synthetic catalog failure"));
    });
    expect(result.current.optionsLoading).toBe(true);
    expect(result.current.options).toEqual([]);
    expect(result.current.error).toBe("");
    await act(async () => nextRead.resolve({ packages: [pack("music", "new")], issues: [] }));
    expect(result.current.selection).toBe("music/new");
    expect(result.current.optionsLoading).toBe(false);
  },
);

test.each(["client", "localPlugins"] as const)(
  "同步防重复，替换 %s 后旧发布不通知回调或解除新发布忙状态",
  async (owner) => {
    const props = options();
    const old = deferred<CommunityPlugin>();
    const current = deferred<CommunityPlugin>();
    props.client.publish.mockReturnValue(old.promise);
    const { result, rerender } = renderHook((next) => useCommunityPluginPublication(next), {
      initialProps: props,
    });
    await waitFor(() => expect(result.current.pack).toEqual(preview));
    act(() => result.current.onAgreedChange(true));
    let oldAction!: Promise<void>;
    act(() => {
      oldAction = result.current.submit();
      void result.current.submit();
    });
    expect(props.client.publish).toHaveBeenCalledOnce();
    const next = options();
    const nextProps = {
      ...props,
      [owner]: owner === "client" ? next.client : next.localPlugins,
      onPublished: next.onPublished,
    };
    nextProps.client.publish.mockReturnValue(current.promise);
    rerender(nextProps);
    await waitFor(() =>
      expect(result.current.optionsLoading || result.current.packLoading).toBe(false),
    );
    act(() => result.current.onAgreedChange(true));
    let action!: Promise<void>;
    act(() => {
      action = result.current.submit();
    });
    expect(result.current.busy).toBe(true);
    await act(async () => {
      old.resolve(published);
      await oldAction;
    });
    expect(result.current.busy).toBe(true);
    expect(props.onPublished).not.toHaveBeenCalled();
    expect(next.onPublished).not.toHaveBeenCalled();
    await act(async () => {
      current.resolve(published);
      await action;
    });
    expect(result.current.busy).toBe(false);
    expect(next.onPublished).toHaveBeenCalledWith(published);
  },
);

test("发布登录失败可以重试，成功回调完成前保持忙状态", async () => {
  const props = options();
  const callback = deferred<void>();
  props.client.publish.mockRejectedValueOnce({ code: "community_unauthorized" });
  props.onPublished.mockReturnValue(callback.promise);
  const { result } = renderHook(() => useCommunityPluginPublication(props));
  await waitFor(() => expect(result.current.pack).toEqual(preview));
  act(() => result.current.onAgreedChange(true));
  await act(async () => result.current.submit());
  expect(result.current.signInRequired).toBe(true);
  expect(props.onPublished).not.toHaveBeenCalled();
  let action!: Promise<void>;
  await act(async () => {
    action = result.current.submit();
  });
  expect(result.current.signInRequired).toBe(false);
  expect(result.current.busy).toBe(true);
  expect(props.onPublished).toHaveBeenCalledWith(published);
  await act(async () => {
    callback.resolve();
    await action;
  });
  expect(result.current.busy).toBe(false);
});

test("卸载后发布成功不通知调用方", async () => {
  const props = options();
  const pending = deferred<CommunityPlugin>();
  props.client.publish.mockReturnValue(pending.promise);
  const { result, unmount } = renderHook(() => useCommunityPluginPublication(props));
  await waitFor(() => expect(result.current.pack).toEqual(preview));
  act(() => result.current.onAgreedChange(true));
  let action!: Promise<void>;
  act(() => {
    action = result.current.submit();
  });
  unmount();
  await act(async () => {
    pending.resolve(published);
    await action;
  });
  expect(props.onPublished).not.toHaveBeenCalled();
});

test.each(["catalog", "preview", "empty"])("%s 无可发布材料时不调用发布", async (failure) => {
  const props = options();
  if (failure === "catalog") props.localPlugins.mockRejectedValue(new Error("synthetic failure"));
  if (failure === "preview")
    props.client.packPreview.mockRejectedValue({ code: "plugin_community_too_large" });
  if (failure === "empty") props.localPlugins.mockResolvedValue({ packages: [], issues: [] });
  const { result } = renderHook(() => useCommunityPluginPublication(props));
  await waitFor(() =>
    expect(result.current.optionsLoading || result.current.packLoading).toBe(false),
  );
  act(() => result.current.onAgreedChange(true));
  expect(result.current.ready).toBe(false);
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  if (failure === "catalog") expect(result.current.error).toBe("读取本地插件失败，请重试。");
  if (failure === "preview") expect(result.current.packError).toContain("8 MB");
});
