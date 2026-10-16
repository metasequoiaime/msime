// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useAiSkinGeneration,
  type AiSkinProposal,
  type AiSkinProgress,
  type CustomSkinLibraryClient,
  type SavedTouchKeyboardSkin,
  type CommunitySkinClient,
} from "@msime/ui";
import { defaultTouchKeyboardSkinDesign } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function proposal(name = "合成皮肤"): AiSkinProposal {
  return {
    name,
    description: "合成设计说明",
    artworkPrompt: "合成插画",
    design: { ...defaultTouchKeyboardSkinDesign, pattern: 3 },
    artwork: { b64_json: "YQ==", mime_type: "image/png", width: 1, height: 1 },
  };
}

function services() {
  let items: SavedTouchKeyboardSkin[] = [];
  const client = {
    generate: vi.fn(async (_id: string, _prompt: string) => [proposal()]),
    cancel: vi.fn(async (_id: string) => undefined),
  };
  const library: CustomSkinLibraryClient = {
    load: vi.fn(async () => items),
    mutate: vi.fn(async (action) => {
      if (action.operation === "create")
        items = [
          ...items,
          { id: `synthetic-${items.length}`, name: action.name, design: action.design },
        ];
      return items;
    }),
  };
  const publish = vi.fn<CommunitySkinClient["publish"]>(async () => undefined);
  const communitySkins = { publish } as unknown as CommunitySkinClient;
  return { client, library, communitySkins, publish };
}

test("公共 hook 可等待生成与插画准备，同批请求只启动一次并跟踪当前进度", async () => {
  const options = services();
  const pending = deferred<AiSkinProposal[]>();
  options.client.generate.mockReturnValue(pending.promise);
  const listeners: ((progress: AiSkinProgress) => void)[] = [];
  const stops: ReturnType<typeof vi.fn>[] = [];
  const client = {
    ...options.client,
    onProgress: vi.fn(async (listener: (progress: AiSkinProgress) => void) => {
      listeners.push(listener);
      const stop = vi.fn();
      stops.push(stop);
      return stop;
    }),
  };
  const { result, unmount } = renderHook(() => useAiSkinGeneration({ ...options, client }), {
    wrapper: StrictMode,
  });
  let task: Promise<void> | undefined;
  act(() => {
    task = result.current.generate();
    result.current.generate();
  });
  expect(client.generate).toHaveBeenCalledOnce();
  expect(result.current.busy).toBe(true);
  const id = client.generate.mock.calls[0][0];
  const listener = listeners.at(-1)!;
  act(() => {
    listener({ requestId: "synthetic-other", completed: 3 });
  });
  expect(result.current.completed).toBe(0);
  act(() => {
    listener({ requestId: id, completed: 2 });
  });
  expect(result.current.completed).toBe(2);
  await act(async () => {
    pending.resolve([proposal()]);
    await task;
  });
  expect(result.current.busy).toBe(false);
  expect(result.current.proposals[0].design).toEqual(
    expect.objectContaining({
      photo: "YQ==",
      photoShade: 0.08,
      photoPosition: 0.5,
      keyOpacity: 0.92,
      pattern: 0,
    }),
  );
  expect(result.current.message).toBe("");
  unmount();
  await act(async () => {});
  expect(stops.every((stop) => stop.mock.calls.length === 1)).toBe(true);
  expect(client.cancel).not.toHaveBeenCalled();
});

test.each(["success", "failure"])("取消后立即重抽，旧请求 %s 不改新请求状态", async (outcome) => {
  const options = services();
  const old = deferred<AiSkinProposal[]>();
  const current = deferred<AiSkinProposal[]>();
  options.client.generate.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
  const { result } = renderHook(() => useAiSkinGeneration(options));
  let oldTask: Promise<void> | undefined;
  act(() => {
    oldTask = result.current.generate();
  });
  const id = options.client.generate.mock.calls[0][0];
  act(() => {
    result.current.cancelGeneration();
    result.current.generate();
  });
  expect(options.client.cancel).toHaveBeenCalledWith(id);
  expect(options.client.generate).toHaveBeenCalledTimes(2);
  await act(async () => {
    if (outcome === "success") old.resolve([proposal("合成旧提案")]);
    else old.reject({ code: "account_unauthorized" });
    await oldTask;
  });
  expect(result.current.proposals).toEqual([]);
  expect(result.current.message).toBe("");
  expect(result.current.busy).toBe(true);
  await act(async () => {
    current.resolve([proposal("合成新提案")]);
  });
  expect(result.current.proposals[0].name).toBe("合成新提案");
  expect(result.current.busy).toBe(false);
});

test("生成失败映射原文案，取消错误不报失败，均可重试", async () => {
  const options = services();
  options.client.generate
    .mockRejectedValueOnce({ code: "account_unauthorized" })
    .mockRejectedValueOnce({ code: "ai_skin_cancelled" });
  const { result } = renderHook(() => useAiSkinGeneration(options));
  await act(async () => {
    await result.current.generate();
  });
  expect(result.current.message).toBe("请先登录，再来抽取皮肤。");
  expect(result.current.busy).toBe(false);
  await act(async () => {
    await result.current.generate();
  });
  expect(result.current.message).toBe("");
  await act(async () => {
    await result.current.generate();
  });
  expect(result.current.proposals).toHaveLength(1);
});

test("保存按提案关联，同批互斥且等待后重复调用复用同一记录", async () => {
  const options = services();
  const pending = deferred<SavedTouchKeyboardSkin[]>();
  options.library.load = vi.fn(() => pending.promise);
  const { result } = renderHook(() => useAiSkinGeneration(options));
  const item = proposal();
  const save = result.current.save;
  let first!: Promise<SavedTouchKeyboardSkin | null>;
  let duplicate!: Promise<SavedTouchKeyboardSkin | null>;
  act(() => {
    first = save(item);
    duplicate = save(item);
  });
  await expect(duplicate).resolves.toBeNull();
  expect(options.library.load).toHaveBeenCalledOnce();
  await act(async () => {
    pending.resolve([]);
    await first;
  });
  const saved = await first;
  expect(saved?.id).toBe("synthetic-0");
  await expect(save(item)).resolves.toBe(saved);
  expect(options.library.mutate).toHaveBeenCalledOnce();
  expect(result.current.saved.get(item)).toBe(saved);
  expect(result.current.saved.get(proposal())).toBeUndefined();
});

test("图库满时不写入并保留原错误文案", async () => {
  const options = services();
  options.library.load = vi.fn(async () =>
    Array.from({ length: 12 }, (_, index) => ({
      id: `synthetic-${index}`,
      name: `合成 ${index}`,
      design: proposal().design,
    })),
  );
  const { result } = renderHook(() => useAiSkinGeneration(options));
  let value: SavedTouchKeyboardSkin | null = null;
  await act(async () => {
    value = await result.current.save(proposal());
  });
  expect(value).toBeNull();
  expect(result.current.message).toBe("最多保存 12 套皮肤，请先删除不需要的设计。");
  expect(options.library.mutate).not.toHaveBeenCalled();
});

test("发布表单保留保存标识与设计，校验、编辑与同批提交独立复用", async () => {
  const options = services();
  const pending = deferred<void>();
  options.publish.mockReturnValue(pending.promise);
  const { result } = renderHook(() => useAiSkinGeneration(options));
  const item = proposal();
  await act(async () => {
    await result.current.beginPublish(item);
  });
  expect(result.current.publishing?.id).toBe("synthetic-0");
  expect(result.current.publishDescription).toBe(item.description);
  await act(async () => {
    await result.current.publish();
  });
  expect(options.publish).not.toHaveBeenCalled();
  act(() => {
    result.current.setPublishAgreed(true);
    result.current.changePublishName("   ");
  });
  await act(async () => {
    await result.current.publish();
  });
  expect(result.current.message).toBe("请填写有效的名称和设计说明。");
  act(() => {
    result.current.changePublishName(" 合成新名称 ");
    result.current.setPublishDescription(" 合成新说明 ");
    result.current.setPublishCategory("minimal");
  });
  let task: Promise<void> | undefined;
  act(() => {
    task = result.current.publish();
    result.current.publish();
  });
  expect(options.publish).toHaveBeenCalledExactlyOnceWith(
    "synthetic-0",
    "合成新名称",
    "合成新说明",
    item.design,
    "minimal",
  );
  expect(result.current.publishBusy).toBe(true);
  await act(async () => {
    pending.resolve();
    await task;
  });
  expect(result.current.publishBusy).toBe(false);
  expect(result.current.publishing).toBeNull();
  expect(result.current.publishAgreed).toBe(false);
  expect(result.current.publishDescription).toBe("");
  expect(result.current.publishCategory).toBe("other");
  expect(result.current.message).toBe("已发布到社区。");
});

test.each(["success", "failure"])("更换社区服务后旧发布 %s 不污染新会话", async (outcome) => {
  const initial = services();
  const pending = deferred<void>();
  initial.publish.mockReturnValue(pending.promise);
  const { result, rerender } = renderHook((options) => useAiSkinGeneration(options), {
    initialProps: initial,
    wrapper: StrictMode,
  });
  await act(async () => {
    await result.current.beginPublish(proposal());
  });
  act(() => result.current.setPublishAgreed(true));
  let task: Promise<void> | undefined;
  act(() => {
    task = result.current.publish();
  });
  const current = services();
  rerender({ ...initial, communitySkins: current.communitySkins });
  expect(result.current.publishBusy).toBe(false);
  expect(result.current.publishing).toBeNull();
  await act(async () => {
    await result.current.beginPublish(proposal("合成新提案"));
  });
  await act(async () => {
    if (outcome === "success") pending.resolve();
    else pending.reject({ code: "synthetic_publish_failure" });
    await task;
  });
  expect(result.current.publishing?.name).toBe("合成新提案");
  expect(result.current.message).toBe("已保存到“我的皮肤”。");
  expect(current.publish).not.toHaveBeenCalled();
});

test("卸载会取消在途生成，并阻止保存加载后继续写入", async () => {
  const options = services();
  const generated = deferred<AiSkinProposal[]>();
  const loaded = deferred<SavedTouchKeyboardSkin[]>();
  options.client.generate.mockReturnValue(generated.promise);
  options.library.load = vi.fn(() => loaded.promise);
  const { result, unmount } = renderHook(() => useAiSkinGeneration(options));
  let generation: Promise<void> | undefined;
  let saving!: Promise<SavedTouchKeyboardSkin | null>;
  act(() => {
    generation = result.current.generate();
    saving = result.current.save(proposal());
  });
  unmount();
  expect(options.client.cancel).toHaveBeenCalledOnce();
  await act(async () => {
    loaded.resolve([]);
    generated.resolve([proposal()]);
    await generation;
    await saving;
  });
  expect(options.library.mutate).not.toHaveBeenCalled();
  await expect(saving).resolves.toBeNull();
});
