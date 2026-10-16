// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useCommunitySkinPublication } from "../../../../packages/ui/src/community/use-community-skin-publication";
import {
  defaultTouchKeyboardSkinDesign as design,
  type SavedTouchKeyboardSkin,
} from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

afterEach(cleanup);
const first = { id: "synthetic-design-1", name: "示例森林", design };
const second = {
  id: "synthetic-design-2",
  name: "示例湖泊",
  design: { ...design, background: 0x123456 },
};
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
      publish: vi.fn(
        async (
          _id: string,
          _name: string,
          _description: string,
          _design: typeof design,
          _category: string,
        ) => {},
      ),
    },
    library: { load: vi.fn(async (): Promise<SavedTouchKeyboardSkin[]> => [first, second]) },
    onPublished: vi.fn(async () => {}),
  };
}

test("加载首个设计并要求授权，校验后提交规范化字段与所选设计", async () => {
  const props = options();
  const { result } = renderHook(() => useCommunitySkinPublication(props));
  expect(result.current.busy).toBe(true);
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  await waitFor(() => expect(result.current.busy).toBe(false));
  expect(result.current.selected).toEqual(first);
  expect(result.current.name).toBe(first.name);
  expect(result.current.category).toBe("other");
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  expect(result.current.error).toBe("请填写有效名称和说明，并确认拥有公开发布所需的素材权利。");
  act(() => {
    result.current.onNameChange("  合成名称  ");
    result.current.onDescriptionChange("  合成说明  ");
    result.current.onAgreedChange(true);
  });
  await act(async () => result.current.submit());
  expect(props.client.publish).toHaveBeenCalledWith(
    expect.stringMatching(/^[0-9a-f-]{36}$/),
    "合成名称",
    "合成说明",
    design,
    "other",
  );
  expect(props.onPublished).toHaveBeenCalledOnce();
  expect(result.current.error).toBe("");
});

test("重试沿用发布 ID，名称、说明、设计和分类编辑分别更新 ID，授权不更新", async () => {
  const props = options();
  props.client.publish.mockRejectedValue(new Error("synthetic publication failure"));
  const { result } = renderHook(() => useCommunitySkinPublication(props));
  await waitFor(() => expect(result.current.busy).toBe(false));
  act(() => result.current.onAgreedChange(true));
  const submit = async () => {
    await act(async () => result.current.submit());
    return props.client.publish.mock.lastCall![0];
  };
  const original = await submit();
  expect(await submit()).toBe(original);
  act(() => {
    result.current.onAgreedChange(false);
    result.current.onAgreedChange(true);
  });
  expect(await submit()).toBe(original);
  let previous = original;
  for (const edit of [
    () => result.current.onNameChange("新名称"),
    () => result.current.onDescriptionChange("新说明"),
    () => result.current.selectDesign(second.id),
    () => result.current.changeCategory("nature"),
  ]) {
    act(edit);
    const next = await submit();
    expect(next).not.toBe(previous);
    previous = next;
  }
  expect(result.current.name).toBe(second.name);
  expect(props.client.publish.mock.lastCall).toEqual([
    previous,
    second.name,
    "新说明",
    second.design,
    "nature",
  ]);
  expect(props.onPublished).not.toHaveBeenCalled();
});

test.each(["client", "library"] as const)(
  "替换 %s 后旧读取的成功和失败都不覆盖当前表单",
  async (owner) => {
    const props = options();
    const old = deferred<SavedTouchKeyboardSkin[]>();
    const staleFailure = deferred<SavedTouchKeyboardSkin[]>();
    props.library.load
      .mockReturnValueOnce(old.promise)
      .mockReturnValueOnce(staleFailure.promise)
      .mockResolvedValue([second]);
    const { result, rerender } = renderHook((next) => useCommunitySkinPublication(next), {
      initialProps: props,
    });
    const middle = {
      ...props,
      [owner]: owner === "client" ? options().client : { load: vi.fn(() => staleFailure.promise) },
    };
    rerender(middle);
    const latest = {
      ...props,
      [owner]: owner === "client" ? options().client : { load: vi.fn(async () => [second]) },
    };
    rerender(latest);
    await waitFor(() => expect(result.current.busy).toBe(false));
    await act(async () => {
      old.resolve([first]);
      staleFailure.reject({ code: "community_unauthorized" });
    });
    expect(result.current.saved).toEqual([second]);
    expect(result.current.selected).toEqual(second);
    expect(result.current.error).toBe("");
    expect(result.current.signInRequired).toBe(false);
  },
);

test("同步防重复，旧发布完成不通知新 owner，也不解除新发布的忙状态", async () => {
  const props = options();
  const old = deferred<void>();
  props.client.publish.mockReturnValue(old.promise);
  const { result, rerender } = renderHook((next) => useCommunitySkinPublication(next), {
    initialProps: props,
  });
  await waitFor(() => expect(result.current.busy).toBe(false));
  act(() => result.current.onAgreedChange(true));
  let oldAction!: Promise<void>;
  act(() => {
    oldAction = result.current.submit();
    void result.current.submit();
  });
  expect(props.client.publish).toHaveBeenCalledOnce();
  const next = options();
  const current = deferred<void>();
  next.client.publish.mockReturnValue(current.promise);
  rerender(next);
  await waitFor(() => expect(result.current.busy).toBe(false));
  let currentAction!: Promise<void>;
  act(() => {
    currentAction = result.current.submit();
  });
  expect(next.client.publish).toHaveBeenCalledOnce();
  await act(async () => {
    old.resolve();
    await oldAction;
  });
  expect(props.onPublished).not.toHaveBeenCalled();
  expect(next.onPublished).not.toHaveBeenCalled();
  expect(result.current.busy).toBe(true);
  await act(async () => {
    current.resolve();
    await currentAction;
  });
  expect(next.onPublished).toHaveBeenCalledOnce();
  expect(result.current.busy).toBe(false);
});

test("读取失败提供登录状态，空库不允许发布", async () => {
  const props = options();
  props.library.load.mockRejectedValue({ code: "community_unauthorized" });
  const { result, rerender } = renderHook((next) => useCommunitySkinPublication(next), {
    initialProps: props,
  });
  await waitFor(() => expect(result.current.busy).toBe(false));
  expect(result.current.signInRequired).toBe(true);
  expect(result.current.error).not.toBe("");
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  const next = options();
  next.library.load.mockResolvedValue([]);
  rerender(next);
  await waitFor(() => expect(result.current.busy).toBe(false));
  expect(result.current.selected).toBeNull();
  act(() => result.current.onAgreedChange(true));
  await act(async () => result.current.submit());
  expect(next.client.publish).not.toHaveBeenCalled();
});

test("发布登录失败后可重试，实例之间不共享字段与授权", async () => {
  const props = options();
  props.client.publish.mockRejectedValueOnce({ code: "community_unauthorized" });
  const left = renderHook(() => useCommunitySkinPublication(props));
  const right = renderHook(() => useCommunitySkinPublication(props));
  await waitFor(() => expect(left.result.current.busy || right.result.current.busy).toBe(false));
  act(() => {
    left.result.current.onAgreedChange(true);
    left.result.current.onNameChange("😀".repeat(40));
  });
  expect(left.result.current.name).toBe("😀".repeat(32));
  expect(right.result.current.name).toBe(first.name);
  expect(right.result.current.agreed).toBe(false);
  await act(async () => left.result.current.submit());
  expect(left.result.current.signInRequired).toBe(true);
  expect(props.onPublished).not.toHaveBeenCalled();
  await act(async () => left.result.current.submit());
  expect(left.result.current.signInRequired).toBe(false);
  expect(props.onPublished).toHaveBeenCalledOnce();
});

test("卸载后发布回包不通知调用方", async () => {
  const props = options();
  const pending = deferred<void>();
  props.client.publish.mockReturnValue(pending.promise);
  const { result, unmount } = renderHook(() => useCommunitySkinPublication(props));
  await waitFor(() => expect(result.current.busy).toBe(false));
  act(() => result.current.onAgreedChange(true));
  let action!: Promise<void>;
  act(() => {
    action = result.current.submit();
  });
  unmount();
  await act(async () => {
    pending.resolve();
    await action;
  });
  expect(props.onPublished).not.toHaveBeenCalled();
});
