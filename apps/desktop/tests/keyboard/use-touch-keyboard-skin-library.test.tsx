// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useTouchKeyboardSkinLibrary,
  type CustomSkinLibraryClient,
  type SavedTouchKeyboardSkin,
} from "@msime/ui";
import { defaultTouchKeyboardSkinDesign } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

afterEach(cleanup);

const design = { ...defaultTouchKeyboardSkinDesign, pattern: 3 as const };
const item = { id: "synthetic-1", name: "合成设计", design };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function library(initial: SavedTouchKeyboardSkin[] = []) {
  let items = initial;
  return {
    load: vi.fn(async () => items),
    mutate: vi.fn<CustomSkinLibraryClient["mutate"]>(async (action) => {
      if (action.operation === "create")
        items = [
          ...items,
          { id: `synthetic-${items.length + 1}`, name: action.name, design: action.design },
        ];
      else if (action.operation === "rename")
        items = items.map((entry) =>
          entry.id === action.id ? { ...entry, name: action.name } : entry,
        );
      else if (action.operation === "update")
        items = items.map((entry) =>
          entry.id === action.id ? { ...entry, design: action.design } : entry,
        );
      else items = items.filter((entry) => entry.id !== action.id);
      return items;
    }),
  };
}

async function ready(client?: CustomSkinLibraryClient) {
  const hook = renderHook(() => useTouchKeyboardSkinLibrary(client));
  await act(async () => {});
  return hook;
}

test("公共 hook 加载库，无客户端时保持空闲", async () => {
  const client = library([item]);
  const first = await ready(client);
  expect(first.result.current.saved).toEqual([item]);
  expect(first.result.current.busy).toBe(false);
  const empty = await ready();
  act(() => empty.result.current.beginCreate());
  await act(async () => expect(await empty.result.current.submitName(design)).toBeUndefined());
  expect(empty.result.current.saved).toEqual([]);
  expect(empty.result.current.busy).toBe(false);
});

test("新建使用去空白名称与当前设计，成功关闭名称表单并返回 created", async () => {
  const client = library([item]);
  const { result } = await ready(client);
  act(() => result.current.beginCreate());
  expect(result.current.skinName).toBe("我的设计 2");
  act(() => result.current.setSkinName("  合成新设计  "));
  await act(async () => expect(await result.current.submitName(design)).toBe("created"));
  expect(client.mutate).toHaveBeenCalledWith({ operation: "create", name: "合成新设计", design });
  expect(result.current.saved.at(-1)?.name).toBe("合成新设计");
  expect(result.current.nameEditor).toBeNull();
  expect(result.current.notice).toContain("设计已保存");
});

test("重命名只发送目标 id 和名称，不替换已保存设计", async () => {
  const client = library([item]);
  const { result } = await ready(client);
  act(() => result.current.beginRename(item));
  expect(result.current.skinName).toBe(item.name);
  act(() => result.current.setSkinName("新合成名称"));
  await act(async () =>
    expect(await result.current.submitName(defaultTouchKeyboardSkinDesign)).toBe("renamed"),
  );
  expect(client.mutate).toHaveBeenCalledWith({
    operation: "rename",
    id: item.id,
    name: "新合成名称",
  });
  expect(result.current.saved[0].design).toEqual(design);
  expect(result.current.notice).toBe("皮肤名称已更新。");
});

test("名称限制按字素，空白名称保留表单并阻止服务调用", async () => {
  const client = library();
  const { result } = await ready(client);
  act(() => result.current.beginCreate());
  act(() => result.current.setSkinName("👩‍💻".repeat(33)));
  expect(result.current.skinName).toBe("👩‍💻".repeat(32));
  act(() => result.current.setSkinName("  "));
  await act(async () => expect(await result.current.submitName(design)).toBeUndefined());
  expect(client.mutate).not.toHaveBeenCalled();
  expect(result.current.nameEditor).toEqual({ operation: "create" });
  expect(result.current.notice).toBe("请输入皮肤名称。");
});

test.each(["update", "delete"] as const)(
  "%s 先保存确认目标，确认时发送正确操作",
  async (operation) => {
    const client = library([item]);
    const { result } = await ready(client);
    act(() => result.current.requestMutation(operation, item));
    expect(client.mutate).not.toHaveBeenCalled();
    expect(result.current.confirmation).toEqual({ operation, item });
    await act(async () =>
      expect(await result.current.confirmMutation(defaultTouchKeyboardSkinDesign)).toBe(true),
    );
    expect(client.mutate).toHaveBeenCalledWith(
      operation === "update"
        ? { operation, id: item.id, design: defaultTouchKeyboardSkinDesign }
        : { operation, id: item.id },
    );
    expect(result.current.confirmation).toBeNull();
    expect(result.current.saved).toEqual(
      operation === "update" ? [{ ...item, design: defaultTouchKeyboardSkinDesign }] : [],
    );
  },
);

test("取消名称和确认不发送变更，重新打开名称清掉旧提示", async () => {
  const client = library([item]);
  const { result } = await ready(client);
  act(() => {
    result.current.setNotice("合成提示");
    result.current.beginCreate();
  });
  expect(result.current.notice).toBe("");
  act(() => result.current.cancelName());
  await act(async () => expect(await result.current.submitName(design)).toBeUndefined());
  act(() => result.current.requestMutation("delete", item));
  act(() => result.current.cancelConfirmation());
  await act(async () => expect(await result.current.confirmMutation(design)).toBe(false));
  expect(client.mutate).not.toHaveBeenCalled();
});

test("同一轮重复提交只发一次，加载期间也阻止变更", async () => {
  const client = library();
  const loading = deferred<SavedTouchKeyboardSkin[]>();
  client.load.mockReturnValue(loading.promise);
  const { result } = await ready(client);
  act(() => result.current.beginCreate());
  await act(async () => expect(await result.current.submitName(design)).toBeUndefined());
  expect(client.mutate).not.toHaveBeenCalled();
  await act(async () => loading.resolve([]));
  const saving = deferred<SavedTouchKeyboardSkin[]>();
  client.mutate.mockReturnValue(saving.promise);
  let first!: Promise<unknown>;
  await act(async () => {
    first = result.current.submitName(design);
    expect(await result.current.submitName(design)).toBeUndefined();
  });
  expect(client.mutate).toHaveBeenCalledOnce();
  expect(result.current.busy).toBe(true);
  await act(async () => {
    saving.resolve([item]);
    expect(await first).toBe("created");
  });
  expect(result.current.busy).toBe(false);
});

test("变更错误保留表单并可重试，错误转换复用领域文案", async () => {
  const client = library();
  client.mutate.mockRejectedValueOnce({ code: "custom_skin_duplicate_name" });
  const { result } = await ready(client);
  act(() => result.current.beginCreate());
  await act(async () => expect(await result.current.submitName(design)).toBeUndefined());
  expect(result.current.notice).toBe("已经有同名皮肤，请换一个名称。");
  expect(result.current.nameEditor).toEqual({ operation: "create" });
  await act(async () => expect(await result.current.submitName(design)).toBe("created"));
  expect(client.mutate).toHaveBeenCalledTimes(2);
});

test("加载错误通过相同领域转换显示", async () => {
  const client = library();
  client.load.mockRejectedValueOnce({ code: "custom_skin_format" });
  const { result } = await ready(client);
  expect(result.current.saved).toEqual([]);
  expect(result.current.notice).toBe("皮肤图库无法读取，原文件已保留。");
  expect(result.current.busy).toBe(false);
});

test("更换客户端后忽略旧加载成功与失败", async () => {
  const oldLoad = deferred<SavedTouchKeyboardSkin[]>();
  const old = library();
  old.load.mockReturnValue(oldLoad.promise);
  const next = library([item]);
  const { result, rerender } = renderHook(({ client }) => useTouchKeyboardSkinLibrary(client), {
    initialProps: { client: old },
  });
  rerender({ client: next });
  await act(async () => {});
  await act(async () => oldLoad.resolve([{ ...item, id: "old" }]));
  expect(result.current.saved).toEqual([item]);
  const late = deferred<SavedTouchKeyboardSkin[]>();
  old.load.mockReturnValue(late.promise);
  rerender({ client: old });
  rerender({ client: next });
  await act(async () => late.reject({ code: "custom_skin_format" }));
  expect(result.current.notice).toBe("");
  expect(result.current.saved).toEqual([item]);
});

test("旧保存不能关闭表单、发布成功或释放新库加载的忙碌状态", async () => {
  const old = library();
  const saving = deferred<SavedTouchKeyboardSkin[]>();
  old.mutate.mockReturnValue(saving.promise);
  const loading = deferred<SavedTouchKeyboardSkin[]>();
  const next = library();
  next.load.mockReturnValue(loading.promise);
  const { result, rerender } = renderHook(({ client }) => useTouchKeyboardSkinLibrary(client), {
    initialProps: { client: old },
  });
  await act(async () => {});
  act(() => result.current.beginCreate());
  let pending!: Promise<unknown>;
  act(() => {
    pending = result.current.submitName(design);
  });
  rerender({ client: next });
  await act(async () => {
    saving.resolve([item]);
    expect(await pending).toBeUndefined();
  });
  expect(result.current.busy).toBe(true);
  expect(result.current.nameEditor).toEqual({ operation: "create" });
  expect(result.current.notice).toBe("");
  expect(result.current.saved).toEqual([]);
  await act(async () => loading.resolve([]));
  expect(result.current.busy).toBe(false);
});

test("卸载后的保存不返回成功，独立 hook 不共享表单状态", async () => {
  const client = library();
  const first = await ready(client);
  const second = await ready(client);
  act(() => first.result.current.beginCreate());
  expect(second.result.current.nameEditor).toBeNull();
  const saving = deferred<SavedTouchKeyboardSkin[]>();
  client.mutate.mockReturnValue(saving.promise);
  let pending!: Promise<unknown>;
  act(() => {
    pending = first.result.current.submitName(design);
  });
  first.unmount();
  await act(async () => {
    saving.resolve([item]);
    expect(await pending).toBeUndefined();
  });
  expect(second.result.current.saved).toEqual([]);
});

test("StrictMode 只接收当前挂载代次的加载结果", async () => {
  const old = deferred<SavedTouchKeyboardSkin[]>();
  const client = library([item]);
  client.load.mockReturnValueOnce(old.promise);
  const { result } = renderHook(() => useTouchKeyboardSkinLibrary(client), { wrapper: StrictMode });
  await act(async () => {});
  await act(async () => old.resolve([{ ...item, id: "stale" }]));
  expect(result.current.saved).toEqual([item]);
  expect(result.current.busy).toBe(false);
});
