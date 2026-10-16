// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useCandidateSkinPublication } from "../../../../packages/ui/src/community/use-candidate-skin-publication";
import type {
  CandidateSkinPackPreview,
  CommunityCandidateSkin,
} from "../../../../packages/ui/src/community/community-candidate-skin-model";
import type { SkinCatalog } from "@msime/ui";

const renderSkinPreview = vi.hoisted(() => vi.fn());
vi.mock("../../../../packages/ui/src/skin/skin-preview-render", () => ({ renderSkinPreview }));
afterEach(() => {
  cleanup();
  renderSkinPreview.mockReset();
});

const preview: CandidateSkinPackPreview = {
  suggestedName: "合成森林",
  license: { code: "MIT", assets: "CC-BY-4.0", source: null },
  fileCount: 2,
  size: 2048,
};
const published: CommunityCandidateSkin = {
  id: "10000000-0000-4000-8000-000000000001",
  package_id: "forest",
  name: "合成森林",
  description: "合成说明",
  author: "示例作者",
  version: "1.0.0",
  license: { code: "MIT", assets: "CC-BY-4.0", source: "" },
  size: 2048,
  file_count: 2,
  downloads: 0,
  rating_count: 0,
  rating_average: 0,
  owned: true,
  my_rating: 0,
  created_at: "2026-10-16T00:00:00Z",
  updated_at: "2026-10-16T00:00:00Z",
  visibility: "public",
};
function catalog(ids = ["forest", "lake"]): SkinCatalog {
  return {
    directory: "/synthetic/skins",
    issues: [],
    packages: ids.map((id) => ({
      id,
      name: `合成 ${id}`,
      version: "1",
      base: "system",
      author: null,
      description: null,
      layouts: ["horizontal"],
      themes: ["light"],
      minWidthDip: 0,
      decorationTopDip: 0,
      decorationWidthDip: 0,
      toolbarStylesheet: null,
      preview: null,
      candidate: { dark: {}, light: {} },
    })),
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
      packPreview: vi.fn(async (): Promise<CandidateSkinPackPreview> => preview),
      addPreview: vi.fn(async (_id: string, _bytes: number[]): Promise<SkinCatalog> => catalog()),
      addLicense: vi.fn(async (_id: string, _license: string): Promise<SkinCatalog> => catalog()),
      publish: vi.fn(
        async (
          _skinId: string,
          _id: string,
          _name: string,
          _description: string,
          _visibility: string,
          _category: string,
        ): Promise<CommunityCandidateSkin> => published,
      ),
    },
    localSkins: vi.fn(async (): Promise<SkinCatalog> => catalog()),
    initialSkinId: "forest",
    readImage: vi.fn(),
    openSkinDirectory: vi.fn(async () => {}),
    onPublished: vi.fn(async (_skin: CommunityCandidateSkin) => {}),
  };
}

test("只有初始皮肤和最小客户端能力时也能预检，授权后提交规范化字段", async () => {
  const { localSkins: _localSkins, ...props } = options();
  const { result } = renderHook(() => useCandidateSkinPublication(props));
  expect(result.current.optionsLoading).toBe(false);
  expect(result.current.options).toEqual([{ id: "forest", name: "forest" }]);
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  await waitFor(() => expect(result.current.pack).toEqual(preview));
  expect(props.client.packPreview).toHaveBeenCalledWith("forest", "public");
  act(() => {
    result.current.onNameChange("  合成名称  ");
    result.current.onDescriptionChange("  合成说明  ");
    result.current.onAgreedChange(true);
  });
  await act(async () => result.current.submit());
  expect(props.client.publish).toHaveBeenCalledWith(
    "forest",
    expect.stringMatching(/^[0-9a-f-]{36}$/),
    "合成名称",
    "合成说明",
    "public",
    "other",
  );
  expect(props.onPublished).toHaveBeenCalledWith(published);
});

test("重试沿用 ID，编辑和分类更改更新 ID，切换可见性重检但保留手工名称", async () => {
  const props = options();
  props.client.publish.mockRejectedValue({ code: "community_unavailable" });
  const { result } = renderHook(() => useCandidateSkinPublication(props));
  await waitFor(() => expect(result.current.pack).toEqual(preview));
  act(() => result.current.onAgreedChange(true));
  const submit = async () => {
    await act(async () => result.current.submit());
    return props.client.publish.mock.lastCall![1];
  };
  let previous = await submit();
  expect(await submit()).toBe(previous);
  act(() => {
    result.current.onAgreedChange(false);
    result.current.onAgreedChange(true);
  });
  expect(await submit()).toBe(previous);
  for (const edit of [
    () => result.current.onNameChange("手工名称"),
    () => result.current.onDescriptionChange("手工说明"),
    () => result.current.changeCategory("nature"),
    () => result.current.setVisibility("private"),
    () => result.current.setSkinId("lake"),
  ]) {
    act(edit);
    await waitFor(() => expect(result.current.packLoading).toBe(false));
    act(() => result.current.onAgreedChange(true));
    const current = await submit();
    expect(current).not.toBe(previous);
    previous = current;
    if (result.current.skinId === "forest") expect(result.current.name).toBe("手工名称");
  }
  expect(result.current.name).toBe(preview.suggestedName);
  expect(props.client.publish.mock.lastCall!.slice(4)).toEqual(["private", "nature"]);
});

test.each(["client", "localSkins"] as const)(
  "替换 %s 后旧目录成功和失败不覆盖新读取",
  async (owner) => {
    const props = options();
    const old = deferred<SkinCatalog>();
    const middle = deferred<SkinCatalog>();
    const current = deferred<SkinCatalog>();
    props.localSkins.mockReturnValueOnce(old.promise).mockReturnValueOnce(middle.promise);
    const { result, rerender } = renderHook((next) => useCandidateSkinPublication(next), {
      initialProps: props,
    });
    rerender({
      ...props,
      [owner]: owner === "client" ? options().client : vi.fn(() => middle.promise),
    });
    props.localSkins.mockReturnValue(current.promise);
    rerender({
      ...props,
      [owner]: owner === "client" ? options().client : vi.fn(() => current.promise),
    });
    await act(async () => {
      old.resolve(catalog(["old"]));
      middle.reject(new Error("synthetic catalog failure"));
    });
    expect(result.current.optionsLoading).toBe(true);
    expect(result.current.options).toEqual([{ id: "forest", name: "forest" }]);
    expect(result.current.error).toBe("");
    await act(async () => current.resolve(catalog(["new"])));
    expect(result.current.optionsLoading).toBe(false);
    expect(result.current.skinId).toBe("new");
  },
);

test.each(["client", "skinId", "visibility"] as const)(
  "预检 %s owner 改变后忽略旧成功与失败",
  async (owner) => {
    const props = options();
    const old = deferred<CandidateSkinPackPreview>();
    const middle = deferred<CandidateSkinPackPreview>();
    const current = deferred<CandidateSkinPackPreview>();
    props.client.packPreview
      .mockReturnValueOnce(old.promise)
      .mockReturnValueOnce(middle.promise)
      .mockReturnValue(current.promise);
    const { result, rerender } = renderHook((next) => useCandidateSkinPublication(next), {
      initialProps: props,
    });
    if (owner === "client") {
      const next = options();
      next.client.packPreview.mockReturnValue(middle.promise);
      rerender({ ...props, client: next.client });
      const latest = options();
      latest.client.packPreview.mockReturnValue(current.promise);
      rerender({ ...props, client: latest.client });
    } else if (owner === "skinId") {
      act(() => result.current.setSkinId("lake"));
      act(() => result.current.setSkinId("forest"));
    } else {
      act(() => result.current.setVisibility("private"));
      act(() => result.current.setVisibility("public"));
    }
    await act(async () => {
      old.resolve(preview);
      middle.reject({ code: "candidate_skin_package_invalid" });
    });
    expect(result.current.pack).toBeNull();
    expect(result.current.packError).toBe("");
    expect(result.current.packLoading).toBe(true);
    await act(async () => current.resolve({ ...preview, suggestedName: "当前名称" }));
    expect(result.current.name).toBe("当前名称");
    expect(result.current.packLoading).toBe(false);
  },
);

test("自定义素材授权按 UTF-8 字节校验，写入失败可重试，成功后重新预检", async () => {
  const props = options();
  props.client.packPreview.mockRejectedValue({ code: "candidate_skin_license_required" });
  props.client.addLicense.mockRejectedValueOnce(new Error("synthetic write failure"));
  const { result } = renderHook(() => useCandidateSkinPublication(props));
  await waitFor(() => expect(result.current.licenseless).toBe(true));
  act(() => {
    result.current.setLicenseChoice("other");
    result.current.setCustomLicense("猫".repeat(41));
  });
  expect(result.current.licenseValid).toBe(false);
  await act(async () => result.current.writeLicense());
  expect(props.client.addLicense).not.toHaveBeenCalled();
  act(() => result.current.setCustomLicense(`  ${"猫".repeat(40)}  `));
  expect(result.current.licenseValid).toBe(true);
  await act(async () => result.current.writeLicense());
  expect(result.current.licenseFailed).toBe(true);
  props.client.packPreview.mockResolvedValue(preview);
  await act(async () => result.current.writeLicense());
  expect(props.client.addLicense).toHaveBeenLastCalledWith("forest", "猫".repeat(40));
  expect(result.current.pack).toEqual(preview);
  expect(result.current.licenseFailed).toBe(false);
});

test.each([
  { action: "license", outcome: "success" },
  { action: "license", outcome: "failure" },
  { action: "preview", outcome: "success" },
  { action: "preview", outcome: "failure" },
])("$action 同步防重复，旧 $outcome 不解除当前补写状态", async ({ action, outcome }) => {
  const props = options();
  const old = deferred<SkinCatalog>();
  const current = deferred<SkinCatalog>();
  const code =
    action === "license" ? "candidate_skin_license_required" : "candidate_skin_preview_required";
  props.client.packPreview.mockRejectedValue({ code });
  props.client.addLicense.mockReturnValue(old.promise);
  props.client.addPreview.mockReturnValue(old.promise);
  renderSkinPreview.mockResolvedValue([137, 80, 78, 71]);
  const { result, rerender } = renderHook((next) => useCandidateSkinPublication(next), {
    initialProps: props,
  });
  await waitFor(() =>
    expect(
      action === "license" ? result.current.licenseless : result.current.previewless,
    ).toBeTruthy(),
  );
  const run = () =>
    action === "license" ? result.current.writeLicense() : result.current.drawPreview();
  let oldAction!: Promise<void>;
  await act(async () => {
    oldAction = run();
    void run();
  });
  expect(
    action === "license" ? props.client.addLicense : props.client.addPreview,
  ).toHaveBeenCalledOnce();
  const next = options();
  next.client.packPreview.mockRejectedValue({ code });
  next.client.addLicense.mockReturnValue(current.promise);
  next.client.addPreview.mockReturnValue(current.promise);
  rerender({ ...props, client: next.client });
  await waitFor(() => expect(result.current.packLoading).toBe(false));
  let currentAction!: Promise<void>;
  await act(async () => {
    currentAction = run();
  });
  await act(async () => {
    if (outcome === "success") old.resolve(catalog(["old"]));
    else old.reject(new Error("synthetic stale failure"));
    await oldAction;
  });
  expect(action === "license" ? result.current.writingLicense : result.current.drawing).toBe(true);
  expect(action === "license" ? result.current.licenseFailed : result.current.drawFailed).toBe(
    false,
  );
  expect(next.client.packPreview).toHaveBeenCalledOnce();
  next.client.packPreview.mockResolvedValue(preview);
  await act(async () => {
    current.resolve(catalog());
    await currentAction;
  });
  expect(result.current.pack).toEqual(preview);
  expect(action === "license" ? result.current.writingLicense : result.current.drawing).toBe(false);
  if (action === "preview") {
    expect(next.client.addPreview).toHaveBeenCalledWith("forest", [137, 80, 78, 71]);
    expect(renderSkinPreview).toHaveBeenCalledWith(catalog().packages[0], props.readImage);
  }
});

test("以预览替代装饰图的皮肤不自动生成，渲染失败可以重试", async () => {
  const props = options();
  const decorated = catalog();
  decorated.packages[0].decorationTopDip = 60;
  decorated.packages[0].decorationWidthDip = 100;
  props.localSkins.mockResolvedValue(decorated);
  props.client.packPreview.mockRejectedValue({ code: "candidate_skin_preview_required" });
  const { result, rerender } = renderHook((next) => useCandidateSkinPublication(next), {
    initialProps: props,
  });
  await waitFor(() =>
    expect(result.current.optionsLoading || result.current.packLoading).toBe(false),
  );
  expect(result.current.previewless).toBeUndefined();
  await act(async () => result.current.drawPreview());
  expect(renderSkinPreview).not.toHaveBeenCalled();
  rerender({ ...props, localSkins: vi.fn(async () => catalog()) });
  await waitFor(() => expect(result.current.previewless).toBeTruthy());
  renderSkinPreview.mockRejectedValueOnce(new Error("synthetic canvas failure"));
  await act(async () => result.current.drawPreview());
  expect(result.current.drawFailed).toBe(true);
  expect(props.client.addPreview).not.toHaveBeenCalled();
  renderSkinPreview.mockResolvedValue([137, 80, 78, 71]);
  props.client.packPreview.mockResolvedValue(preview);
  await act(async () => result.current.drawPreview());
  expect(result.current.drawFailed).toBe(false);
  expect(result.current.pack).toEqual(preview);
});

test.each(["client", "localSkins", "unmount"] as const)(
  "发布同步防重复，%s 后旧成功不通知调用方",
  async (owner) => {
    const props = options();
    const pending = deferred<CommunityCandidateSkin>();
    props.client.publish.mockReturnValue(pending.promise);
    const { result, rerender, unmount } = renderHook((next) => useCandidateSkinPublication(next), {
      initialProps: props,
    });
    await waitFor(() => expect(result.current.pack).toEqual(preview));
    act(() => result.current.onAgreedChange(true));
    let action!: Promise<void>;
    act(() => {
      action = result.current.submit();
      void result.current.submit();
    });
    expect(props.client.publish).toHaveBeenCalledOnce();
    if (owner === "unmount") unmount();
    else
      rerender({ ...props, [owner]: owner === "client" ? options().client : options().localSkins });
    await act(async () => {
      pending.resolve(published);
      await action;
    });
    expect(props.onPublished).not.toHaveBeenCalled();
  },
);

test("发布登录失败后可重试，成功回调完成前保持忙状态，实例不共享授权", async () => {
  const props = options();
  const callback = deferred<void>();
  props.client.publish.mockRejectedValueOnce({ code: "community_unauthorized" });
  props.onPublished.mockReturnValue(callback.promise);
  const left = renderHook(() => useCandidateSkinPublication(props));
  const right = renderHook(() => useCandidateSkinPublication(props));
  await waitFor(() => expect(left.result.current.pack && right.result.current.pack).toBeTruthy());
  act(() => left.result.current.onAgreedChange(true));
  expect(right.result.current.agreed).toBe(false);
  await act(async () => left.result.current.submit());
  expect(left.result.current.signInRequired).toBe(true);
  let action!: Promise<void>;
  await act(async () => {
    action = left.result.current.submit();
  });
  expect(left.result.current.signInRequired).toBe(false);
  expect(left.result.current.busy).toBe(true);
  await act(async () => {
    callback.resolve();
    await action;
  });
  expect(left.result.current.busy).toBe(false);
});

test("打开目录失败后可以重试", async () => {
  const props = options();
  props.openSkinDirectory.mockRejectedValueOnce(new Error("synthetic folder failure"));
  const { result } = renderHook(() => useCandidateSkinPublication(props));
  await act(async () => result.current.openFolder());
  expect(result.current.openFailed).toBe(true);
  await act(async () => result.current.openFolder());
  expect(result.current.openFailed).toBe(false);
});
