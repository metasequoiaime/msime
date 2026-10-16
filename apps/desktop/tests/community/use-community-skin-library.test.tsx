// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useCommunitySkinLibrary } from "../../../../packages/ui/src/community/use-community-skin-library";
import type {
  CommunitySkin,
  CommunitySkinDownload,
} from "../../../../packages/ui/src/community/community-skin-model";
import { defaultTouchKeyboardSkinDesign as design } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";
import type { SavedTouchKeyboardSkin } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";

afterEach(cleanup);

const skin: CommunitySkin = {
  id: "AB000000-0000-4000-8000-000000000001",
  name: "示例皮肤",
  description: "示例说明",
  author: "示例作者",
  design,
  downloads: 0,
  rating_count: 0,
  rating_average: 0,
  owned: false,
  my_rating: 0,
};
const saved: SavedTouchKeyboardSkin = { id: skin.id.toLowerCase(), name: skin.name, design };
const preferences: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function options() {
  return {
    client: {
      download: vi.fn(async (): Promise<CommunitySkinDownload> => ({
        skin: saved,
        trial: { id: "trial-example", name: skin.name },
      })),
      finishTrial: vi.fn(async () => {}),
    },
    localSkinLibrary: { load: vi.fn(async (): Promise<SavedTouchKeyboardSkin[]> => [saved]) },
    harmony: true,
    browsing: true,
    preferences,
    onApplyPreferences: vi.fn(async (_next: Preferences) => {}),
    onSkinApplied: vi.fn(),
    toast: vi.fn(),
  };
}

test("按大小写无关的 id 获取库设计，使用完成后报告社区原始 id 和完整偏好", async () => {
  const props = options();
  const applying = deferred<void>();
  props.onApplyPreferences.mockImplementation(() => applying.promise);
  const { result, rerender } = renderHook((next) => useCommunitySkinLibrary(next), {
    initialProps: props,
  });
  await waitFor(() => expect(result.current.pillState(skin)).toBe("use"));
  let action!: Promise<void>;
  act(() => {
    action = result.current.apply(skin);
    void result.current.apply(skin);
    void result.current.take(skin);
  });
  expect(result.current.pillState(skin)).toBe("using");
  expect(props.onApplyPreferences).toHaveBeenCalledOnce();
  expect(props.client.download).not.toHaveBeenCalled();
  expect(props.onSkinApplied).not.toHaveBeenCalled();
  const next = props.onApplyPreferences.mock.calls[0][0];
  expect(next).toEqual({
    ...preferences,
    global_theme: "custom",
    custom_theme: {
      base: "system",
      candidate_skin: null,
      candidate_skin_dark: null,
      keyboard: design,
    },
  });
  await act(async () => {
    applying.resolve();
    await action;
  });
  expect(props.onSkinApplied).toHaveBeenCalledWith(skin.id);
  expect(props.toast).toHaveBeenCalledWith(`已换上「${skin.name}」`);
  rerender({ ...props, preferences: next });
  expect(result.current.pillState(skin)).toBe("in-use");
});

test("获取同步防重复，恢复试用后入库，早先的读取不能覆盖获取结果", async () => {
  const props = options();
  const loading = deferred<SavedTouchKeyboardSkin[]>();
  const restoring = deferred<void>();
  props.localSkinLibrary.load.mockImplementation(() => loading.promise);
  props.client.finishTrial.mockImplementation(() => restoring.promise);
  const first = renderHook(() => useCommunitySkinLibrary(props));
  const second = renderHook(() => useCommunitySkinLibrary(props));
  let action!: Promise<void>;
  act(() => {
    action = first.result.current.take(skin);
    void first.result.current.take({ ...skin, id: skin.id.toLowerCase() });
  });
  expect(first.result.current.pillState(skin)).toBe("getting");
  expect(second.result.current.pillState(skin)).toBe("get");
  await waitFor(() =>
    expect(props.client.finishTrial).toHaveBeenCalledWith("trial-example", false),
  );
  expect(props.client.download).toHaveBeenCalledOnce();
  expect(props.toast).not.toHaveBeenCalled();
  await act(async () => {
    restoring.resolve();
    await action;
  });
  expect(first.result.current.pillState(skin)).toBe("use");
  await act(async () => loading.resolve([]));
  expect(first.result.current.pillState(skin)).toBe("use");
  expect(second.result.current.pillState(skin)).toBe("get");
  expect(props.onApplyPreferences).not.toHaveBeenCalled();
});

test("关闭网格和切换皮肤库会忽略旧读取，返回网格重新读取详情保存的设计", async () => {
  const props = options();
  const old = deferred<SavedTouchKeyboardSkin[]>();
  props.localSkinLibrary.load.mockImplementation(() => old.promise);
  const { result, rerender } = renderHook((next) => useCommunitySkinLibrary(next), {
    initialProps: props,
  });
  rerender({ ...props, browsing: false });
  await act(async () => old.resolve([saved]));
  expect(result.current.pillState(skin)).toBe("get");
  const stale = deferred<SavedTouchKeyboardSkin[]>();
  props.localSkinLibrary.load.mockImplementation(() => stale.promise);
  rerender(props);
  const replacement = { load: vi.fn(async (): Promise<SavedTouchKeyboardSkin[]> => []) };
  rerender({ ...props, localSkinLibrary: replacement });
  await act(async () => stale.resolve([saved]));
  expect(result.current.pillState(skin)).toBe("get");
  rerender({ ...props, localSkinLibrary: replacement, browsing: false });
  replacement.load.mockResolvedValue([saved]);
  rerender({ ...props, localSkinLibrary: replacement });
  await waitFor(() => expect(result.current.pillState(skin)).toBe("use"));
  expect(replacement.load).toHaveBeenCalledTimes(2);
});

test("库读取仅在鸿蒙网格可见时进行，失败保持获取操作", async () => {
  const props = options();
  props.localSkinLibrary.load.mockRejectedValue(new Error("synthetic load failure"));
  const { result, rerender } = renderHook((next) => useCommunitySkinLibrary(next), {
    initialProps: { ...props, harmony: false },
  });
  expect(props.localSkinLibrary.load).not.toHaveBeenCalled();
  rerender({ ...props, browsing: false });
  expect(props.localSkinLibrary.load).not.toHaveBeenCalled();
  await act(async () => rerender(props));
  expect(result.current.pillState(skin)).toBe("get");
  expect(props.toast).not.toHaveBeenCalled();
});

test("获取恢复失败时保留待清理试用，换客户端时用原客户端重试且不污染新客户端", async () => {
  const props = options();
  props.localSkinLibrary.load.mockResolvedValue([]);
  props.client.finishTrial.mockRejectedValueOnce(new Error("synthetic restore failure"));
  const { result, rerender, unmount } = renderHook((next) => useCommunitySkinLibrary(next), {
    initialProps: props,
  });
  await act(async () => result.current.take(skin));
  expect(result.current.pillState(skin)).toBe("get");
  expect(props.toast).toHaveBeenCalledOnce();
  const nextClient = options().client;
  rerender({ ...props, client: nextClient });
  expect(props.client.finishTrial).toHaveBeenCalledTimes(2);
  expect(props.client.finishTrial).toHaveBeenLastCalledWith("trial-example", false);
  unmount();
  expect(nextClient.finishTrial).not.toHaveBeenCalled();
});

test("获取失败会报告可处理的错误并允许重试，成功试用无需卸载清理", async () => {
  const props = options();
  props.localSkinLibrary.load.mockResolvedValue([]);
  props.client.download.mockRejectedValueOnce({ code: "community_skin_library_full" });
  const { result, unmount } = renderHook(() => useCommunitySkinLibrary(props));
  await act(async () => result.current.take(skin));
  expect(result.current.pillState(skin)).toBe("get");
  expect(props.toast).toHaveBeenCalledWith("最多保存 12 套皮肤，请先删除不需要的设计。");
  await act(async () => result.current.take(skin));
  expect(result.current.pillState(skin)).toBe("use");
  unmount();
  expect(props.client.finishTrial).toHaveBeenCalledOnce();
});

test("应用失败保留当前偏好、不上报换装，清除忙状态后可重试", async () => {
  const props = options();
  props.onApplyPreferences.mockRejectedValueOnce(new Error("synthetic apply failure"));
  const { result } = renderHook(() => useCommunitySkinLibrary(props));
  await waitFor(() => expect(result.current.pillState(skin)).toBe("use"));
  await act(async () => result.current.apply(skin));
  expect(props.onSkinApplied).not.toHaveBeenCalled();
  expect(props.toast).toHaveBeenCalledWith("切换失败，保留当前皮肤");
  expect(result.current.pillState(skin)).toBe("use");
  await act(async () => result.current.apply(skin));
  expect(props.onSkinApplied).toHaveBeenCalledOnce();
});
